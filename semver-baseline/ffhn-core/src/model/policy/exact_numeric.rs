use std::cmp::Ordering;
use std::fmt;

use rust_decimal::Decimal;

/// A private, lossless decomposition of a decimal policy operand.
///
/// Decimal policy comparisons operate on this form rather than on `Decimal` arithmetic. The
/// latter has a bounded precision representation and can round an intermediate result before the
/// policy decision is made.
#[derive(Clone, Copy, Debug)]
pub(super) struct DecimalParts {
    negative: bool,
    coefficient: u128,
    scale: u32,
}

impl DecimalParts {
    pub(super) fn from_decimal(value: Decimal) -> Self {
        let mantissa = value.mantissa();
        let coefficient = mantissa.unsigned_abs();
        Self {
            negative: mantissa.is_negative() && coefficient != 0,
            coefficient,
            scale: value.scale(),
        }
    }

    fn is_zero(self) -> bool {
        self.coefficient == 0
    }
}

/// Result of an exact decimal percentage comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DecimalPercentageResult {
    Decision(bool),
    Unavailable,
    ZeroReference,
}

/// A defensive failure of the fixed-width proof that makes decimal comparisons exact.
///
/// `DecimalParts` can only be constructed from `Decimal`, so accepted decimal values keep these
/// paths unreachable. They remain checked in production because an implementation regression
/// must become one target-scoped integration fault, never a process-aborting panic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExactNumericInvariantError {
    /// A caller attempted to align an operand to a smaller scale.
    AlignmentScaleRegression,
    /// A value exceeded the documented fixed-width proof budget.
    WidthProofViolation,
}

impl fmt::Display for ExactNumericInvariantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlignmentScaleRegression => formatter
                .write_str("decimal comparison attempted to align an operand to a smaller scale"),
            Self::WidthProofViolation => formatter
                .write_str("decimal comparison exceeded the documented Unsigned320 width proof"),
        }
    }
}

/// Compares an absolute decimal delta with its threshold without rounding either operand.
pub(super) fn absolute_delta_at_least(
    current: DecimalParts,
    reference: DecimalParts,
    threshold: DecimalParts,
) -> Result<bool, ExactNumericInvariantError> {
    if threshold.negative {
        return Ok(true);
    }
    let scale = current.scale.max(reference.scale).max(threshold.scale);
    let delta = absolute_delta(current, reference, scale)?;
    let threshold = align(threshold, scale)?;
    Ok(delta >= threshold)
}

/// Compares a decimal percentage delta by cross multiplication, without rounded division.
pub(super) fn percentage_delta_at_least(
    current: DecimalParts,
    reference: DecimalParts,
    percentage: DecimalParts,
) -> Result<DecimalPercentageResult, ExactNumericInvariantError> {
    if reference.is_zero() {
        return Ok(DecimalPercentageResult::ZeroReference);
    }
    if percentage.negative {
        return Ok(DecimalPercentageResult::Unavailable);
    }

    let value_scale = current.scale.max(reference.scale);
    let delta = absolute_delta(current, reference, value_scale)?;
    let reference = align(reference, value_scale)?;
    let left = decimal_width_value(multiply_by_power_of_ten(
        Some(decimal_width_value(
            delta.checked_mul_u128(u128::from(100_u8)),
        )?),
        percentage.scale,
    ))?;
    let right = decimal_width_value(reference.checked_mul_u128(percentage.coefficient))?;

    Ok(DecimalPercentageResult::Decision(left >= right))
}

fn absolute_delta(
    current: DecimalParts,
    reference: DecimalParts,
    scale: u32,
) -> Result<Unsigned320, ExactNumericInvariantError> {
    let current_coefficient = align(current, scale)?;
    let reference_coefficient = align(reference, scale)?;
    if current.negative == reference.negative {
        match current_coefficient.cmp(&reference_coefficient) {
            Ordering::Less => {
                decimal_width_value(reference_coefficient.checked_sub(current_coefficient))
            }
            Ordering::Equal => Ok(Unsigned320::ZERO),
            Ordering::Greater => {
                decimal_width_value(current_coefficient.checked_sub(reference_coefficient))
            }
        }
    } else {
        decimal_width_value(current_coefficient.checked_add(reference_coefficient))
    }
}

fn align(parts: DecimalParts, scale: u32) -> Result<Unsigned320, ExactNumericInvariantError> {
    let exponent = scale
        .checked_sub(parts.scale)
        .ok_or(ExactNumericInvariantError::AlignmentScaleRegression)?;
    decimal_width_value(multiply_by_power_of_ten(
        Some(decimal_width_value(Unsigned320::try_from_u128(
            parts.coefficient,
        ))?),
        exponent,
    ))
}

/// Converts a fixed-width result into a checked decimal-proof result.
fn decimal_width_value<T>(value: Option<T>) -> Result<T, ExactNumericInvariantError> {
    value.ok_or(ExactNumericInvariantError::WidthProofViolation)
}

/// Multiplies a fixed-width value by an exact power of ten.
pub(super) fn multiply_by_power_of_ten<const LIMBS: usize>(
    mut value: Option<Unsigned<LIMBS>>,
    scale: u32,
) -> Option<Unsigned<LIMBS>> {
    for _ in 0..scale {
        value = value?.checked_mul_u128(u128::from(10_u8));
    }
    value
}

/// Fixed-width unsigned arithmetic used solely for policy cross products.
///
/// Integer percentage comparisons require at most 229 bits, so `Unsigned256` covers their valid
/// inputs. A decimal coefficient is below 2^96 and a scale is at most 28: alignment needs fewer
/// than 190 bits, an absolute delta fewer than 191 bits, the percentage left side fewer than 292
/// bits, and the right side fewer than 286 bits. `Unsigned320` therefore represents every valid
/// decimal or money comparison exactly without bringing a general big-integer dependency into the
/// policy boundary. Every conversion and arithmetic operation checks this proof in production;
/// proof failure is reported to the target lifecycle rather than panicking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Unsigned<const LIMBS: usize>([u64; LIMBS]);

pub(super) type Unsigned256 = Unsigned<4>;
type Unsigned320 = Unsigned<5>;

impl<const LIMBS: usize> Unsigned<LIMBS> {
    pub(super) const ZERO: Self = Self([0; LIMBS]);

    /// Converts `value` only when this fixed-width representation can retain every bit.
    pub(super) fn try_from_u128(value: u128) -> Option<Self> {
        let mut limbs = [0; LIMBS];
        let bytes = value.to_le_bytes();
        let low_limb = u64::from_le_bytes(bytes[..8].try_into().ok()?);
        let high_limb = u64::from_le_bytes(bytes[8..].try_into().ok()?);
        if LIMBS == 0 {
            return (value == 0).then_some(Self(limbs));
        }
        limbs[0] = low_limb;
        if high_limb != 0 {
            let high = limbs.get_mut(1)?;
            *high = high_limb;
        }
        Some(Self(limbs))
    }

    pub(super) fn checked_mul_u128(mut self, mut multiplier: u128) -> Option<Self> {
        let mut product = Self::ZERO;
        // A `u128` multiplier has exactly this many possible set-bit positions. Keeping the loop
        // structurally bounded makes the proof executable even under defensive fault injection.
        for _ in 0..u128::BITS {
            if multiplier == 0 {
                return Some(product);
            }
            if multiplier & 1 != 0 {
                product = product.checked_add(self)?;
            }
            multiplier >>= 1;
            if multiplier != 0 {
                self = self.checked_double()?;
            }
        }
        debug_assert_eq!(multiplier, 0);
        Some(product)
    }

    pub(super) fn checked_add(self, other: Self) -> Option<Self> {
        let mut values = [0; LIMBS];
        let mut carry = false;
        for (index, value) in values.iter_mut().enumerate() {
            let (sum, left_carry) = self.0[index].overflowing_add(other.0[index]);
            let (sum, carry_carry) = sum.overflowing_add(u64::from(carry));
            *value = sum;
            carry = left_carry || carry_carry;
        }
        (!carry).then_some(Self(values))
    }

    pub(super) fn checked_sub(self, other: Self) -> Option<Self> {
        let mut values = [0; LIMBS];
        let mut borrow = false;
        for (index, value) in values.iter_mut().enumerate() {
            let (difference, left_borrow) = self.0[index].overflowing_sub(other.0[index]);
            let (difference, borrow_borrow) = difference.overflowing_sub(u64::from(borrow));
            *value = difference;
            borrow = left_borrow || borrow_borrow;
        }
        (!borrow).then_some(Self(values))
    }

    fn checked_double(self) -> Option<Self> {
        let mut values = [0; LIMBS];
        let mut carry = 0;
        for (index, value) in values.iter_mut().enumerate() {
            let (doubled, overflowed) = self.0[index].overflowing_mul(2);
            *value = doubled + carry;
            carry = u64::from(overflowed);
        }
        (carry == 0).then_some(Self(values))
    }
}

impl<const LIMBS: usize> Ord for Unsigned<LIMBS> {
    fn cmp(&self, other: &Self) -> Ordering {
        for index in (0..LIMBS).rev() {
            let ordering = self.0[index].cmp(&other.0[index]);
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        Ordering::Equal
    }
}

impl<const LIMBS: usize> PartialOrd for Unsigned<LIMBS> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_width_overflow_stays_detectable_for_defensive_paths() {
        let maximum = Unsigned::<4>([u64::MAX; 4]);
        assert_eq!(Unsigned::<0>::try_from_u128(u128::MAX), None);
        assert_eq!(Unsigned::<0>::try_from_u128(0), Some(Unsigned([])));
        assert_eq!(multiply_by_power_of_ten(Some(maximum), 1), None);
        assert_eq!(maximum.checked_mul_u128(2), None);
        assert_eq!(
            Unsigned::<1>::try_from_u128(u128::from(u64::MAX)),
            Some(Unsigned([u64::MAX]))
        );
        assert_eq!(Unsigned::<1>::try_from_u128(u128::from(u64::MAX) + 1), None);
        assert_eq!(
            Unsigned::<2>::try_from_u128(u128::MAX),
            Some(Unsigned([u64::MAX; 2]))
        );
        assert_eq!(
            Unsigned::<2>::try_from_u128(3)
                .expect("three")
                .checked_mul_u128(5),
            Some(Unsigned([15, 0]))
        );
        assert_eq!(
            Unsigned::<2>::try_from_u128(3)
                .expect("three")
                .checked_mul_u128(0),
            Some(Unsigned::ZERO)
        );
        assert_eq!(
            Unsigned::<2>([1_u64 << 63, 0]).checked_mul_u128(2),
            Some(Unsigned([0, 1]))
        );
        assert_eq!(
            Unsigned::<2>::try_from_u128(1)
                .expect("one fits")
                .checked_mul_u128(1_u128 << 127),
            Some(Unsigned([0, 1_u64 << 63]))
        );
        assert_eq!(
            Unsigned::<2>([1, 0]).checked_double(),
            Some(Unsigned([2, 0]))
        );
        assert_eq!(
            Unsigned::<2>([u64::MAX, 0]).checked_double(),
            Some(Unsigned([u64::MAX - 1, 1]))
        );
        assert_eq!(Unsigned::<2>([0, 1_u64 << 63]).checked_double(), None);
        assert_eq!(
            Unsigned256::ZERO.checked_sub(Unsigned256::try_from_u128(1).expect("fits")),
            None
        );
    }

    #[test]
    fn decimal_percentage_cross_products_fit_at_the_documented_five_limb_bound() {
        let maximum_coefficient_at_scale_28 =
            Decimal::from_i128_with_scale(Decimal::MAX.mantissa(), 28);
        let current = maximum_coefficient_at_scale_28;
        let reference = Decimal::MAX;
        let percentage = maximum_coefficient_at_scale_28;

        assert_eq!(
            percentage_delta_at_least(
                DecimalParts::from_decimal(current),
                DecimalParts::from_decimal(reference),
                DecimalParts::from_decimal(percentage),
            ),
            Ok(DecimalPercentageResult::Decision(true))
        );
    }

    #[test]
    fn decimal_comparators_preserve_their_non_arithmetic_special_cases() {
        let one = DecimalParts::from_decimal(Decimal::ONE);
        let zero = DecimalParts::from_decimal(Decimal::ZERO);
        let negative_one = DecimalParts::from_decimal(Decimal::NEGATIVE_ONE);

        assert_eq!(absolute_delta_at_least(one, zero, negative_one), Ok(true));
        assert_eq!(
            percentage_delta_at_least(one, zero, one),
            Ok(DecimalPercentageResult::ZeroReference)
        );
        assert_eq!(
            percentage_delta_at_least(one, one, negative_one),
            Ok(DecimalPercentageResult::Unavailable)
        );
    }

    #[test]
    fn defensive_decimal_proof_failures_are_typed_never_panics() {
        let invalid_alignment = DecimalParts {
            negative: false,
            coefficient: 1,
            scale: 2,
        };
        assert_eq!(
            align(invalid_alignment, 1),
            Err(ExactNumericInvariantError::AlignmentScaleRegression)
        );
        assert_eq!(
            ExactNumericInvariantError::AlignmentScaleRegression.to_string(),
            "decimal comparison attempted to align an operand to a smaller scale"
        );
        assert_eq!(
            ExactNumericInvariantError::WidthProofViolation.to_string(),
            "decimal comparison exceeded the documented Unsigned320 width proof"
        );
    }
}

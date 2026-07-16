use std::cmp::Ordering;
use std::str::FromStr;

use rust_decimal::Decimal;
use semver::Version;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::super::observation::parse_canonical_value;
use super::super::{DeclaredType, Observation, TypeParams};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PolicyValue {
    Integer(i128),
    Decimal(Decimal),
    Money { amount: Decimal, currency: String },
    Semver(Version),
    Datetime(OffsetDateTime),
}

impl PolicyValue {
    pub(super) fn compare(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Integer(left), Self::Integer(right)) => Some(left.cmp(right)),
            (Self::Decimal(left), Self::Decimal(right)) => Some(left.cmp(right)),
            (
                Self::Money {
                    amount: left,
                    currency: left_currency,
                },
                Self::Money {
                    amount: right,
                    currency: right_currency,
                },
            ) if left_currency == right_currency => Some(left.cmp(right)),
            (Self::Semver(left), Self::Semver(right)) => Some(left.cmp(right)),
            (Self::Datetime(left), Self::Datetime(right)) => Some(left.cmp(right)),
            _ => None,
        }
    }

    pub(super) fn is_negative_numeric(&self) -> bool {
        match self {
            Self::Integer(value) => *value < 0,
            Self::Decimal(value) | Self::Money { amount: value, .. } => *value < Decimal::ZERO,
            Self::Semver(_) | Self::Datetime(_) => false,
        }
    }

    pub(super) fn canonical_identity_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Integer(left), Self::Integer(right)) => left == right,
            (Self::Decimal(left), Self::Decimal(right)) => left == right,
            (
                Self::Money {
                    amount: left,
                    currency: left_currency,
                },
                Self::Money {
                    amount: right,
                    currency: right_currency,
                },
            ) => left == right && left_currency == right_currency,
            (Self::Semver(left), Self::Semver(right)) => left.to_string() == right.to_string(),
            (Self::Datetime(left), Self::Datetime(right)) => left == right,
            _ => false,
        }
    }

    pub(super) fn exact_abs_delta_at_least(
        &self,
        reference: &Self,
        threshold: &Self,
    ) -> ArithmeticResult {
        match (self, reference, threshold) {
            (Self::Integer(current), Self::Integer(previous), Self::Integer(threshold)) => {
                let Some(delta) = current.checked_sub(*previous).and_then(i128::checked_abs) else {
                    return ArithmeticResult::Overflow;
                };
                ArithmeticResult::Decision(delta >= *threshold)
            }
            (Self::Decimal(current), Self::Decimal(previous), Self::Decimal(threshold)) => {
                let Some(delta) = Decimal::checked_sub(*current, *previous) else {
                    return ArithmeticResult::Overflow;
                };
                ArithmeticResult::Decision(delta.abs() >= *threshold)
            }
            (
                Self::Money {
                    amount: current,
                    currency: current_currency,
                },
                Self::Money {
                    amount: previous,
                    currency: previous_currency,
                },
                Self::Money {
                    amount: threshold,
                    currency: threshold_currency,
                },
            ) if current_currency == previous_currency
                && current_currency == threshold_currency =>
            {
                let Some(delta) = Decimal::checked_sub(*current, *previous) else {
                    return ArithmeticResult::Overflow;
                };
                ArithmeticResult::Decision(delta.abs() >= *threshold)
            }
            _ => ArithmeticResult::Unavailable,
        }
    }

    pub(super) fn exact_percentage_delta_at_least(
        &self,
        reference: &Self,
        percentage: Decimal,
    ) -> ArithmeticResult {
        match (self, reference) {
            (Self::Integer(current), Self::Integer(previous)) => {
                integer_percentage_delta_at_least(*current, *previous, percentage)
            }
            (Self::Decimal(current), Self::Decimal(previous)) => {
                decimal_percentage_delta_at_least(*current, *previous, percentage)
            }
            (
                Self::Money {
                    amount: current,
                    currency: current_currency,
                },
                Self::Money {
                    amount: previous,
                    currency: reference_currency,
                },
            ) if current_currency == reference_currency => {
                decimal_percentage_delta_at_least(*current, *previous, percentage)
            }
            _ => ArithmeticResult::Unavailable,
        }
    }
}

fn decimal_percentage_delta_at_least(
    current: Decimal,
    previous: Decimal,
    percentage: Decimal,
) -> ArithmeticResult {
    if previous.is_zero() {
        return ArithmeticResult::ZeroReference;
    }
    let Some(delta) = Decimal::checked_sub(current, previous) else {
        return ArithmeticResult::Overflow;
    };
    let Some(left) = Decimal::checked_mul(delta.abs(), Decimal::ONE_HUNDRED) else {
        return ArithmeticResult::Overflow;
    };
    let Some(right) = Decimal::checked_mul(percentage, previous.abs()) else {
        return ArithmeticResult::Overflow;
    };
    ArithmeticResult::Decision(left >= right)
}

fn integer_percentage_delta_at_least(
    current: i128,
    previous: i128,
    percentage: Decimal,
) -> ArithmeticResult {
    if previous == 0 {
        return ArithmeticResult::ZeroReference;
    }
    let Ok(percentage_mantissa) = u128::try_from(percentage.mantissa()) else {
        return ArithmeticResult::Unavailable;
    };
    let left = multiply_by_power_of_ten(
        Unsigned256::from_u128(absolute_difference(current, previous))
            .checked_mul_u128(u128::from(100_u8)),
        percentage.scale(),
    );
    let right =
        Unsigned256::from_u128(previous.unsigned_abs()).checked_mul_u128(percentage_mantissa);
    compare_percentage_cross_products(left, right)
}

fn compare_percentage_cross_products(
    left: Option<Unsigned256>,
    right: Option<Unsigned256>,
) -> ArithmeticResult {
    let Some(left) = left else {
        return ArithmeticResult::Overflow;
    };
    let Some(right) = right else {
        return ArithmeticResult::Overflow;
    };
    ArithmeticResult::Decision(left >= right)
}

fn absolute_difference(left: i128, right: i128) -> u128 {
    if left.is_negative() == right.is_negative() {
        left.abs_diff(right)
    } else {
        left.unsigned_abs() + right.unsigned_abs()
    }
}

fn multiply_by_power_of_ten(mut value: Option<Unsigned256>, scale: u32) -> Option<Unsigned256> {
    for _ in 0..scale {
        value = value?.checked_mul_u128(u128::from(10_u8));
    }
    value
}

/// Sufficient exact unsigned width for the largest v2 integer percentage comparison.
///
/// `i128` operands can differ by almost 2^128−1; a decimal percentage has a 96-bit mantissa and
/// scale at most 28. Cross multiplication therefore needs up to 229 bits, so all valid inputs fit
/// in this fixed 256-bit representation without narrowing the integer domain to `Decimal`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Unsigned256([u64; 4]);

impl Unsigned256 {
    const ZERO: Self = Self([0; 4]);

    fn from_u128(value: u128) -> Self {
        let low = u64::try_from(value & u128::from(u64::MAX))
            .expect("a masked u128 low half always fits in u64");
        let high = u64::try_from(value >> 64).expect("a shifted u128 high half always fits in u64");
        Self([low, high, 0, 0])
    }

    fn checked_mul_u128(mut self, mut multiplier: u128) -> Option<Self> {
        let mut product = Self::ZERO;
        while multiplier != 0 {
            if multiplier & 1 != 0 {
                product = product.checked_add(self)?;
            }
            multiplier >>= 1;
            if multiplier != 0 {
                self = self.checked_double()?;
            }
        }
        Some(product)
    }

    fn checked_add(self, other: Self) -> Option<Self> {
        let mut values = [0; 4];
        let mut carry = false;
        for (index, value) in values.iter_mut().enumerate() {
            let (sum, left_carry) = self.0[index].overflowing_add(other.0[index]);
            let (sum, carry_carry) = sum.overflowing_add(u64::from(carry));
            *value = sum;
            carry = left_carry || carry_carry;
        }
        (!carry).then_some(Self(values))
    }

    fn checked_double(self) -> Option<Self> {
        let mut values = [0; 4];
        let mut carry = 0;
        for (index, value) in values.iter_mut().enumerate() {
            *value = (self.0[index] << 1) | carry;
            carry = self.0[index] >> 63;
        }
        (carry == 0).then_some(Self(values))
    }
}

impl Ord for Unsigned256 {
    fn cmp(&self, other: &Self) -> Ordering {
        for index in (0..self.0.len()).rev() {
            let ordering = self.0[index].cmp(&other.0[index]);
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for Unsigned256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ArithmeticResult {
    Decision(bool),
    Unavailable,
    Overflow,
    ZeroReference,
}

pub(super) fn parse_config_value(
    declared_type: DeclaredType,
    params: &TypeParams,
    raw: &str,
) -> Result<PolicyValue, String> {
    let canonical = parse_canonical_value(declared_type, params, raw)?;
    parse_canonical_value_as_policy_value(declared_type, params, &canonical)
}

pub(super) fn parse_observation_value(observation: &Observation) -> Result<PolicyValue, String> {
    parse_canonical_value_as_policy_value(
        observation.declared_type_for_policy(),
        observation.type_params_for_policy(),
        observation.canonical_value(),
    )
}

pub(super) fn parse_percentage(raw: &str) -> Result<Decimal, String> {
    if raw.trim() != raw || raw.is_empty() {
        return Err("must be a non-empty invariant decimal string".to_owned());
    }
    Decimal::from_str(raw)
        .map_err(|error| format!("must be an invariant decimal percentage: {error}"))
}

fn parse_canonical_value_as_policy_value(
    declared_type: DeclaredType,
    params: &TypeParams,
    canonical: &str,
) -> Result<PolicyValue, String> {
    match declared_type {
        DeclaredType::Integer => canonical
            .parse()
            .map(PolicyValue::Integer)
            .map_err(|error| format!("canonical integer is invalid: {error}")),
        DeclaredType::Decimal => Decimal::from_str(canonical)
            .map(PolicyValue::Decimal)
            .map_err(|error| format!("canonical decimal is invalid: {error}")),
        DeclaredType::Money => {
            let amount = Decimal::from_str(canonical)
                .map_err(|error| format!("canonical money amount is invalid: {error}"))?;
            let currency = params
                .currency
                .clone()
                .ok_or_else(|| "money type_params.currency is missing".to_owned())?;
            Ok(PolicyValue::Money { amount, currency })
        }
        DeclaredType::Semver => Version::parse(canonical)
            .map(PolicyValue::Semver)
            .map_err(|error| format!("canonical semver is invalid: {error}")),
        DeclaredType::Datetime => OffsetDateTime::parse(canonical, &Rfc3339)
            .map(PolicyValue::Datetime)
            .map_err(|error| format!("canonical datetime is invalid: {error}")),
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn comparison_and_integer_percentage_defenses_cover_mismatched_and_overflowing_inputs() {
        assert_eq!(
            PolicyValue::Decimal(Decimal::ONE).compare(&PolicyValue::Decimal(Decimal::TWO)),
            Some(Ordering::Less)
        );
        assert_eq!(
            integer_percentage_delta_at_least(2, 1, Decimal::NEGATIVE_ONE),
            ArithmeticResult::Unavailable
        );
        assert_eq!(multiply_by_power_of_ten(None, 1), None);
        assert_eq!(
            multiply_by_power_of_ten(Some(Unsigned256([u64::MAX; 4])), 1),
            None
        );
        assert_eq!(Unsigned256([u64::MAX; 4]).checked_mul_u128(2), None);
        assert_eq!(
            compare_percentage_cross_products(None, Some(Unsigned256::ZERO)),
            ArithmeticResult::Overflow
        );
        assert_eq!(
            compare_percentage_cross_products(Some(Unsigned256::ZERO), None),
            ArithmeticResult::Overflow
        );
    }
}

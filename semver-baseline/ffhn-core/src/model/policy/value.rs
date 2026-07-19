use std::cmp::Ordering;
use std::str::FromStr;

use rust_decimal::Decimal;
use semver::Version;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::super::observation::parse::parse_canonical_value;
use super::super::{DeclaredType, Observation, TypeParams};
use super::exact_numeric::{
    DecimalParts, DecimalPercentageResult, ExactNumericInvariantError, Unsigned256,
    absolute_delta_at_least, multiply_by_power_of_ten, percentage_delta_at_least,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PolicyValue {
    Text(String),
    Integer(i128),
    Decimal(Decimal),
    Money { amount: Decimal, currency: String },
    Semver(Version),
    Datetime(OffsetDateTime),
}

impl PolicyValue {
    pub(super) fn compare(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Text(_), Self::Text(_)) => None,
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
            Self::Text(_) => false,
            Self::Integer(value) => *value < 0,
            Self::Decimal(value) | Self::Money { amount: value, .. } => *value < Decimal::ZERO,
            Self::Semver(_) | Self::Datetime(_) => false,
        }
    }

    pub(super) fn canonical_identity_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text(left), Self::Text(right)) => left == right,
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
    ) -> Result<ArithmeticResult, ExactNumericInvariantError> {
        match (self, reference, threshold) {
            (Self::Integer(current), Self::Integer(previous), Self::Integer(threshold)) => {
                let Some(delta) = current.checked_sub(*previous).and_then(i128::checked_abs) else {
                    return Ok(ArithmeticResult::Overflow);
                };
                Ok(ArithmeticResult::Decision(delta >= *threshold))
            }
            (Self::Decimal(current), Self::Decimal(previous), Self::Decimal(threshold)) => {
                decimal_absolute_delta_at_least(*current, *previous, *threshold)
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
                decimal_absolute_delta_at_least(*current, *previous, *threshold)
            }
            _ => Ok(ArithmeticResult::Unavailable),
        }
    }

    pub(super) fn exact_percentage_delta_at_least(
        &self,
        reference: &Self,
        percentage: Decimal,
    ) -> Result<ArithmeticResult, ExactNumericInvariantError> {
        match (self, reference) {
            (Self::Integer(current), Self::Integer(previous)) => Ok(
                integer_percentage_delta_at_least(*current, *previous, percentage),
            ),
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
            _ => Ok(ArithmeticResult::Unavailable),
        }
    }
}

fn decimal_percentage_delta_at_least(
    current: Decimal,
    previous: Decimal,
    percentage: Decimal,
) -> Result<ArithmeticResult, ExactNumericInvariantError> {
    Ok(
        match percentage_delta_at_least(
            DecimalParts::from_decimal(current),
            DecimalParts::from_decimal(previous),
            DecimalParts::from_decimal(percentage),
        )? {
            DecimalPercentageResult::Decision(decision) => ArithmeticResult::Decision(decision),
            DecimalPercentageResult::Unavailable => ArithmeticResult::Unavailable,
            DecimalPercentageResult::ZeroReference => ArithmeticResult::ZeroReference,
        },
    )
}

fn decimal_absolute_delta_at_least(
    current: Decimal,
    previous: Decimal,
    threshold: Decimal,
) -> Result<ArithmeticResult, ExactNumericInvariantError> {
    Ok(ArithmeticResult::Decision(absolute_delta_at_least(
        DecimalParts::from_decimal(current),
        DecimalParts::from_decimal(previous),
        DecimalParts::from_decimal(threshold),
    )?))
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
        Unsigned256::try_from_u128(absolute_difference(current, previous))
            .and_then(|value| value.checked_mul_u128(u128::from(100_u8))),
        percentage.scale(),
    );
    let right = Unsigned256::try_from_u128(previous.unsigned_abs())
        .and_then(|value| value.checked_mul_u128(percentage_mantissa));
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
    Decimal::from_str(raw).map_err(|_| "must be an invariant decimal percentage".to_owned())
}

fn parse_canonical_value_as_policy_value(
    declared_type: DeclaredType,
    params: &TypeParams,
    canonical: &str,
) -> Result<PolicyValue, String> {
    match declared_type {
        DeclaredType::Text => Ok(PolicyValue::Text(canonical.to_owned())),
        DeclaredType::Integer => canonical
            .parse()
            .map(PolicyValue::Integer)
            .map_err(|_| "canonical integer is invalid".to_owned()),
        DeclaredType::Decimal => Decimal::from_str(canonical)
            .map(PolicyValue::Decimal)
            .map_err(|_| "canonical decimal is invalid".to_owned()),
        DeclaredType::Money => {
            let amount = Decimal::from_str(canonical)
                .map_err(|_| "canonical money amount is invalid".to_owned())?;
            let currency = params
                .currency
                .clone()
                .ok_or_else(|| "money type_params.currency is missing".to_owned())?;
            Ok(PolicyValue::Money { amount, currency })
        }
        DeclaredType::Semver => Version::parse(canonical)
            .map(PolicyValue::Semver)
            .map_err(|_| "canonical semantic version is invalid".to_owned()),
        DeclaredType::Datetime => OffsetDateTime::parse(canonical, &Rfc3339)
            .map(PolicyValue::Datetime)
            .map_err(|_| "canonical datetime is invalid".to_owned()),
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    fn decimal(raw: &str) -> Decimal {
        Decimal::from_str(raw).expect("exact decimal")
    }

    fn euro(amount: Decimal) -> PolicyValue {
        PolicyValue::Money {
            amount,
            currency: "EUR".to_owned(),
        }
    }

    #[test]
    fn comparison_and_integer_percentage_defenses_cover_mismatched_inputs() {
        assert_eq!(
            PolicyValue::Decimal(Decimal::ONE).compare(&PolicyValue::Decimal(Decimal::TWO)),
            Some(Ordering::Less)
        );
        assert_eq!(
            integer_percentage_delta_at_least(2, 1, Decimal::NEGATIVE_ONE),
            ArithmeticResult::Unavailable
        );
        assert_eq!(
            decimal_percentage_delta_at_least(Decimal::ONE, Decimal::ONE, Decimal::NEGATIVE_ONE),
            Ok(ArithmeticResult::Unavailable)
        );
        assert_eq!(
            decimal_percentage_delta_at_least(Decimal::ONE, Decimal::ZERO, Decimal::ONE),
            Ok(ArithmeticResult::ZeroReference)
        );
        assert_eq!(
            compare_percentage_cross_products(None, Some(Unsigned256::ZERO)),
            ArithmeticResult::Overflow
        );
        assert_eq!(
            compare_percentage_cross_products(Some(Unsigned256::ZERO), None),
            ArithmeticResult::Overflow
        );
    }

    #[test]
    fn decimal_and_money_policy_comparisons_do_not_round_decision_operands() {
        let tiny = decimal("0.0000000000000000000000000001");
        let one_tenth_of_a_trillionth = decimal("0.0000000000001");
        let one_quadrillionth = decimal("0.0000000000000001");

        assert_eq!(
            PolicyValue::Decimal(Decimal::MAX).exact_abs_delta_at_least(
                &PolicyValue::Decimal(tiny),
                &PolicyValue::Decimal(Decimal::MAX),
            ),
            Ok(ArithmeticResult::Decision(false))
        );
        assert_eq!(
            PolicyValue::Decimal(one_tenth_of_a_trillionth).exact_percentage_delta_at_least(
                &PolicyValue::Decimal(one_tenth_of_a_trillionth),
                one_quadrillionth,
            ),
            Ok(ArithmeticResult::Decision(false))
        );
        assert_eq!(
            euro(one_tenth_of_a_trillionth).exact_percentage_delta_at_least(
                &euro(one_tenth_of_a_trillionth),
                one_quadrillionth,
            ),
            Ok(ArithmeticResult::Decision(false))
        );
        assert_eq!(
            euro(Decimal::MAX).exact_abs_delta_at_least(&euro(tiny), &euro(Decimal::MAX),),
            Ok(ArithmeticResult::Decision(false))
        );
    }

    #[test]
    fn decimal_scale_products_can_round_in_either_direction_without_entering_policy_decisions() {
        let smallest = decimal("0.0000000000000000000000000001");
        assert_eq!(
            Decimal::checked_mul(smallest, smallest),
            Some(Decimal::ZERO)
        );
        assert_eq!(
            Decimal::checked_mul(decimal("5.5"), smallest),
            Some(decimal("0.0000000000000000000000000006"))
        );
    }

    #[test]
    fn decimal_and_money_deltas_handle_opposite_signs_and_three_scales_exactly() {
        let negative_one = decimal("-1.00");
        let positive_two = decimal("2.0");
        let three = decimal("3.000");
        let current = decimal("1.234");
        let reference = decimal("1.20");
        let just_above_delta = decimal("0.0341");

        for (current, reference, threshold, expected) in [
            (negative_one, positive_two, three, true),
            (current, reference, just_above_delta, false),
        ] {
            assert_eq!(
                PolicyValue::Decimal(current).exact_abs_delta_at_least(
                    &PolicyValue::Decimal(reference),
                    &PolicyValue::Decimal(threshold),
                ),
                Ok(ArithmeticResult::Decision(expected))
            );
            assert_eq!(
                euro(current).exact_abs_delta_at_least(&euro(reference), &euro(threshold)),
                Ok(ArithmeticResult::Decision(expected))
            );
        }

        assert_eq!(
            PolicyValue::Decimal(negative_one).exact_percentage_delta_at_least(
                &PolicyValue::Decimal(positive_two),
                decimal("150")
            ),
            Ok(ArithmeticResult::Decision(true))
        );
        assert_eq!(
            euro(negative_one).exact_percentage_delta_at_least(&euro(positive_two), decimal("150")),
            Ok(ArithmeticResult::Decision(true))
        );
    }

    #[test]
    fn integer_and_decimal_policy_decisions_are_symmetric_for_safe_whole_numbers() {
        for current in [-23_i128, -1, 0, 1, 23] {
            for reference in [-17_i128, -3, 1, 19] {
                for threshold in [0_i128, 1, 3, 40] {
                    let integer = PolicyValue::Integer(current).exact_abs_delta_at_least(
                        &PolicyValue::Integer(reference),
                        &PolicyValue::Integer(threshold),
                    );
                    let decimal = PolicyValue::Decimal(Decimal::from(current))
                        .exact_abs_delta_at_least(
                            &PolicyValue::Decimal(Decimal::from(reference)),
                            &PolicyValue::Decimal(Decimal::from(threshold)),
                        );
                    assert_eq!(
                        integer, decimal,
                        "delta_abs current={current}, reference={reference}, threshold={threshold}"
                    );
                }

                for percentage in [0_i128, 1, 5, 100, 200] {
                    let integer = PolicyValue::Integer(current).exact_percentage_delta_at_least(
                        &PolicyValue::Integer(reference),
                        Decimal::from(percentage),
                    );
                    let decimal = PolicyValue::Decimal(Decimal::from(current))
                        .exact_percentage_delta_at_least(
                            &PolicyValue::Decimal(Decimal::from(reference)),
                            Decimal::from(percentage),
                        );
                    assert_eq!(
                        integer, decimal,
                        "delta_pct current={current}, reference={reference}, percentage={percentage}"
                    );
                }
            }
        }
    }
}

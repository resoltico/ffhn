//! Predicate dispatch and exact comparison semantics for valid observations.

use std::collections::BTreeMap;

use crate::CoreError;

use crate::{DeclaredType, Observation, TypeParams};

use super::super::condition::{
    Condition, ConditionId, ConditionPredicate, ConditionReference, ThresholdDirection,
};
use super::super::value::{
    ArithmeticResult, PolicyValue, parse_config_value, parse_observation_value, parse_percentage,
};
use super::types::{
    ConditionContext, ConditionEvaluation, ConditionOutcome, ConditionReferenceEvidence,
};

/// Borrowed typed-policy contract owned by graph measurements.
///
/// It deliberately contains only the facts policy evaluation owns. Acquisition, identifiers,
/// delivery routes, and health tuning cannot influence an exact condition decision.
pub(crate) struct PolicyContract<'a> {
    declared_type: DeclaredType,
    type_params: &'a TypeParams,
    conditions: &'a [Condition],
}

impl<'a> PolicyContract<'a> {
    /// Borrows the exact typed facts needed for policy validation and evaluation.
    pub(crate) const fn new(
        declared_type: DeclaredType,
        type_params: &'a TypeParams,
        conditions: &'a [Condition],
    ) -> Self {
        Self {
            declared_type,
            type_params,
            conditions,
        }
    }
}

pub(crate) fn evaluate_conditions(
    contract: &PolicyContract<'_>,
    observation: &Observation,
    contexts: &BTreeMap<ConditionId, ConditionContext<'_>>,
) -> Result<Vec<ConditionEvaluation>, CoreError> {
    observation.validate()?;
    if !observation_matches_contract(contract, observation) {
        return Err(CoreError::contract(
            "policy observation must use the measurement declared_type and type_params",
        ));
    }
    let current = parse_observation_value(observation).map_err(CoreError::contract)?;
    contract
        .conditions
        .iter()
        .map(|condition| {
            let context = contexts
                .get(condition.id())
                .copied()
                .unwrap_or_else(ConditionContext::empty);
            evaluate_condition(contract, condition, &current, context)
        })
        .collect()
}

fn evaluate_condition(
    contract: &PolicyContract<'_>,
    condition: &Condition,
    current: &PolicyValue,
    context: ConditionContext<'_>,
) -> Result<ConditionEvaluation, CoreError> {
    let (outcome, active_after, event_predicate) = match condition.predicate() {
        ConditionPredicate::Changed { reference } => (
            evaluate_changed(current, resolve_reference(contract, *reference, context)?),
            context.active,
            true,
        ),
        ConditionPredicate::DeltaAbs {
            reference,
            threshold,
        } => (
            evaluate_delta_abs(
                current,
                resolve_reference(contract, *reference, context)?,
                &parse_threshold(contract, threshold)?,
            )?,
            context.active,
            true,
        ),
        ConditionPredicate::DeltaPct {
            reference,
            threshold,
        } => (
            evaluate_delta_pct(
                current,
                resolve_reference(contract, *reference, context)?,
                parse_percentage(threshold).map_err(CoreError::contract)?,
            )?,
            context.active,
            true,
        ),
        ConditionPredicate::Crosses {
            threshold,
            direction,
        } => (
            evaluate_crosses(
                contract,
                current,
                context.last_accepted_observation,
                &parse_threshold(contract, threshold)?,
                *direction,
            )?,
            context.active,
            true,
        ),
        ConditionPredicate::Lt { threshold } => {
            let outcome =
                evaluate_ordered(current, &parse_threshold(contract, threshold)?, |order| {
                    order.is_lt()
                });
            let active = next_level_active(outcome, context.active);
            (outcome, active, false)
        }
        ConditionPredicate::Gt { threshold } => {
            let outcome =
                evaluate_ordered(current, &parse_threshold(contract, threshold)?, |order| {
                    order.is_gt()
                });
            let active = next_level_active(outcome, context.active);
            (outcome, active, false)
        }
        ConditionPredicate::Band {
            enter_threshold,
            exit_threshold,
            direction,
        } => evaluate_band(
            current,
            &parse_threshold(contract, enter_threshold)?,
            &parse_threshold(contract, exit_threshold)?,
            *direction,
            context.active,
        ),
    };
    let trigger = if event_predicate {
        outcome == ConditionOutcome::Satisfied
    } else {
        outcome == ConditionOutcome::Satisfied && !context.active
    };
    Ok(ConditionEvaluation {
        condition_id: condition.id().clone(),
        outcome,
        trigger,
        active_before: context.active,
        active_after,
        reference_evidence: configured_reference_evidence(contract, condition.predicate(), context),
    })
}

fn configured_reference_evidence(
    contract: &PolicyContract<'_>,
    predicate: &ConditionPredicate,
    context: ConditionContext<'_>,
) -> Option<ConditionReferenceEvidence> {
    let reference = match predicate {
        ConditionPredicate::Changed { reference }
        | ConditionPredicate::DeltaAbs { reference, .. }
        | ConditionPredicate::DeltaPct { reference, .. } => *reference,
        ConditionPredicate::Crosses { .. } => ConditionReference::LastAcceptedObservation,
        ConditionPredicate::Lt { .. }
        | ConditionPredicate::Gt { .. }
        | ConditionPredicate::Band { .. } => return None,
    };
    let observation = match reference {
        ConditionReference::LastAcceptedObservation => context.last_accepted_observation,
        ConditionReference::FixedInitialBaseline => context.fixed_initial_baseline,
        ConditionReference::LastConditionTransition => context.last_condition_transition,
    };
    Some(match observation {
        Some(observation) if observation_matches_contract(contract, observation) => {
            ConditionReferenceEvidence::Resolved {
                reference,
                canonical_value: observation.canonical_value().to_owned(),
            }
        }
        Some(_) | None => ConditionReferenceEvidence::Unavailable { reference },
    })
}

fn resolve_reference(
    contract: &PolicyContract<'_>,
    reference: ConditionReference,
    context: ConditionContext<'_>,
) -> Result<Option<PolicyValue>, CoreError> {
    let observation = match reference {
        ConditionReference::LastAcceptedObservation => context.last_accepted_observation,
        ConditionReference::FixedInitialBaseline => context.fixed_initial_baseline,
        ConditionReference::LastConditionTransition => context.last_condition_transition,
    };
    match observation {
        Some(value) if observation_matches_contract(contract, value) => {
            parse_observation_value(value)
                .map(Some)
                .map_err(CoreError::contract)
        }
        Some(_) | None => Ok(None),
    }
}

fn evaluate_changed(current: &PolicyValue, reference: Option<PolicyValue>) -> ConditionOutcome {
    match reference {
        Some(reference) if current.canonical_identity_eq(&reference) => {
            ConditionOutcome::NotSatisfied
        }
        Some(_) => ConditionOutcome::Satisfied,
        None => ConditionOutcome::Unavailable,
    }
}

fn evaluate_delta_abs(
    current: &PolicyValue,
    reference: Option<PolicyValue>,
    threshold: &PolicyValue,
) -> Result<ConditionOutcome, CoreError> {
    match reference {
        None => Ok(ConditionOutcome::Unavailable),
        Some(reference) => current
            .exact_abs_delta_at_least(&reference, threshold)
            .map(arithmetic_outcome)
            .map_err(|error| CoreError::policy_invariant(error.to_string())),
    }
}

fn evaluate_delta_pct(
    current: &PolicyValue,
    reference: Option<PolicyValue>,
    percentage: rust_decimal::Decimal,
) -> Result<ConditionOutcome, CoreError> {
    match reference {
        None => Ok(ConditionOutcome::Unavailable),
        Some(reference) => current
            .exact_percentage_delta_at_least(&reference, percentage)
            .map(arithmetic_outcome)
            .map_err(|error| CoreError::policy_invariant(error.to_string())),
    }
}

fn evaluate_crosses(
    contract: &PolicyContract<'_>,
    current: &PolicyValue,
    previous: Option<&Observation>,
    threshold: &PolicyValue,
    direction: ThresholdDirection,
) -> Result<ConditionOutcome, CoreError> {
    let Some(previous) = previous else {
        return Ok(ConditionOutcome::Unavailable);
    };
    if !observation_matches_contract(contract, previous) {
        return Ok(ConditionOutcome::Unavailable);
    }
    let previous = parse_observation_value(previous).map_err(CoreError::contract)?;
    let Some(previous_order) = previous.compare(threshold) else {
        return Ok(ConditionOutcome::Unavailable);
    };
    let Some(current_order) = current.compare(threshold) else {
        return Ok(ConditionOutcome::Unavailable);
    };
    let crossed = match direction {
        ThresholdDirection::Rising => previous_order.is_lt() && !current_order.is_lt(),
        ThresholdDirection::Falling => previous_order.is_gt() && !current_order.is_gt(),
    };
    Ok(if crossed {
        ConditionOutcome::Satisfied
    } else {
        ConditionOutcome::NotSatisfied
    })
}

fn evaluate_ordered(
    current: &PolicyValue,
    threshold: &PolicyValue,
    predicate: impl FnOnce(std::cmp::Ordering) -> bool,
) -> ConditionOutcome {
    match current.compare(threshold) {
        Some(order) if predicate(order) => ConditionOutcome::Satisfied,
        Some(_) => ConditionOutcome::NotSatisfied,
        None => ConditionOutcome::Unavailable,
    }
}

fn evaluate_band(
    current: &PolicyValue,
    enter: &PolicyValue,
    exit: &PolicyValue,
    direction: ThresholdDirection,
    was_active: bool,
) -> (ConditionOutcome, bool, bool) {
    let threshold = if was_active { exit } else { enter };
    let outcome = match current.compare(threshold) {
        Some(order) => {
            let satisfied = match direction {
                ThresholdDirection::Rising => !order.is_lt(),
                ThresholdDirection::Falling => !order.is_gt(),
            };
            if satisfied {
                ConditionOutcome::Satisfied
            } else {
                ConditionOutcome::NotSatisfied
            }
        }
        None => ConditionOutcome::Unavailable,
    };
    let active = next_level_active(outcome, was_active);
    (outcome, active, false)
}

fn next_level_active(outcome: ConditionOutcome, was_active: bool) -> bool {
    match outcome {
        ConditionOutcome::Satisfied => true,
        ConditionOutcome::NotSatisfied => false,
        ConditionOutcome::Unavailable
        | ConditionOutcome::ArithmeticOverflow
        | ConditionOutcome::ZeroReference => was_active,
    }
}

fn arithmetic_outcome(result: ArithmeticResult) -> ConditionOutcome {
    match result {
        ArithmeticResult::Decision(true) => ConditionOutcome::Satisfied,
        ArithmeticResult::Decision(false) => ConditionOutcome::NotSatisfied,
        ArithmeticResult::Unavailable => ConditionOutcome::Unavailable,
        ArithmeticResult::Overflow => ConditionOutcome::ArithmeticOverflow,
        ArithmeticResult::ZeroReference => ConditionOutcome::ZeroReference,
    }
}

fn parse_threshold(
    contract: &PolicyContract<'_>,
    threshold: &str,
) -> Result<PolicyValue, CoreError> {
    parse_config_value(contract.declared_type, contract.type_params, threshold)
        .map_err(CoreError::contract)
}

fn observation_matches_contract(contract: &PolicyContract<'_>, observation: &Observation) -> bool {
    observation.declared_type_for_policy() == contract.declared_type
        && observation.type_params_for_policy() == contract.type_params
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    fn integer_observation(value: &str) -> Observation {
        crate::model::parse_json_scalar_token_for_contract(
            DeclaredType::Integer,
            &TypeParams::default(),
            value.to_owned(),
        )
        .expect("integer observation")
    }

    #[test]
    fn defensive_policy_helpers_preserve_unavailability_and_issue_state() {
        let params = TypeParams::default();
        let contract = PolicyContract::new(DeclaredType::Integer, &params, &[]);
        let previous = integer_observation("1");
        assert_eq!(
            evaluate_crosses(
                &contract,
                &PolicyValue::Integer(2),
                Some(&previous),
                &PolicyValue::Decimal(rust_decimal::Decimal::ONE),
                ThresholdDirection::Rising,
            )
            .expect("comparison"),
            ConditionOutcome::Unavailable
        );
        assert_eq!(
            evaluate_crosses(
                &contract,
                &PolicyValue::Decimal(rust_decimal::Decimal::TWO),
                Some(&previous),
                &PolicyValue::Integer(1),
                ThresholdDirection::Rising,
            )
            .expect("comparison"),
            ConditionOutcome::Unavailable
        );
        assert_eq!(
            evaluate_ordered(
                &PolicyValue::Integer(1),
                &PolicyValue::Decimal(rust_decimal::Decimal::ONE),
                |_| true,
            ),
            ConditionOutcome::Unavailable
        );
        assert_eq!(
            evaluate_band(
                &PolicyValue::Integer(1),
                &PolicyValue::Decimal(rust_decimal::Decimal::ONE),
                &PolicyValue::Decimal(rust_decimal::Decimal::ONE),
                ThresholdDirection::Rising,
                true,
            ),
            (ConditionOutcome::Unavailable, true, false)
        );
        assert!(next_level_active(
            ConditionOutcome::ArithmeticOverflow,
            true
        ));
        assert!(next_level_active(ConditionOutcome::ZeroReference, true));
        assert_eq!(
            arithmetic_outcome(ArithmeticResult::Unavailable),
            ConditionOutcome::Unavailable
        );
    }

    #[test]
    fn named_reference_resolution_and_evidence_preserve_contract_compatibility() {
        let params = TypeParams::default();
        let condition: Condition = toml::from_str(
            "condition_id = \"changed\"\n[predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"\n",
        )
        .expect("condition");
        let contract = PolicyContract::new(
            DeclaredType::Integer,
            &params,
            std::slice::from_ref(&condition),
        );
        let last = integer_observation("1");
        let fixed = integer_observation("2");
        let transition = integer_observation("3");
        let context = ConditionContext::new(Some(&last), Some(&fixed), Some(&transition), false);
        assert_eq!(
            configured_reference_evidence(&contract, condition.predicate(), context),
            Some(ConditionReferenceEvidence::Resolved {
                reference: ConditionReference::LastAcceptedObservation,
                canonical_value: "1".to_owned(),
            })
        );
        for (reference, expected) in [
            (ConditionReference::LastAcceptedObservation, 1),
            (ConditionReference::FixedInitialBaseline, 2),
            (ConditionReference::LastConditionTransition, 3),
        ] {
            assert_eq!(
                resolve_reference(&contract, reference, context).expect("reference"),
                Some(PolicyValue::Integer(expected))
            );
        }

        let text = crate::model::parse_json_scalar_token_for_contract(
            DeclaredType::Text,
            &TypeParams::default(),
            "\"text\"".to_owned(),
        )
        .expect("text observation");
        let incompatible = ConditionContext::new(Some(&text), None, None, false);
        assert_eq!(
            configured_reference_evidence(&contract, condition.predicate(), incompatible),
            Some(ConditionReferenceEvidence::Unavailable {
                reference: ConditionReference::LastAcceptedObservation,
            })
        );
        assert_eq!(
            resolve_reference(
                &contract,
                ConditionReference::LastAcceptedObservation,
                incompatible,
            )
            .expect("incompatible reference"),
            None
        );
    }
}

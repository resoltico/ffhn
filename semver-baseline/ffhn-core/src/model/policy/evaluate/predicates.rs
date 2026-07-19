//! Predicate dispatch and exact comparison semantics for valid observations.

use std::collections::BTreeMap;

use crate::CoreError;

use crate::{Observation, TargetDocument};

use super::super::condition::{
    Condition, ConditionId, ConditionPredicate, ConditionReference, ThresholdDirection,
};
use super::super::value::{
    ArithmeticResult, PolicyValue, parse_config_value, parse_observation_value, parse_percentage,
};
use super::types::{
    ConditionContext, ConditionEvaluation, ConditionOutcome, ConditionReferenceEvidence,
};

pub(super) fn evaluate_conditions(
    target: &TargetDocument,
    observation: &Observation,
    contexts: &BTreeMap<ConditionId, ConditionContext<'_>>,
) -> Result<Vec<ConditionEvaluation>, CoreError> {
    observation.validate()?;
    if !observation_matches_target(target, observation) {
        return Err(CoreError::contract(
            "policy observation must use the target declared_type and type_params",
        ));
    }
    let current = parse_observation_value(observation).map_err(CoreError::contract)?;
    target
        .conditions()
        .iter()
        .map(|condition| {
            let context = contexts
                .get(condition.id())
                .copied()
                .unwrap_or_else(ConditionContext::empty);
            evaluate_condition(target, condition, &current, context)
        })
        .collect()
}

fn evaluate_condition(
    target: &TargetDocument,
    condition: &Condition,
    current: &PolicyValue,
    context: ConditionContext<'_>,
) -> Result<ConditionEvaluation, CoreError> {
    let (outcome, active_after, event_predicate) = match condition.predicate() {
        ConditionPredicate::Changed { reference } => (
            evaluate_changed(current, resolve_reference(target, *reference, context)?),
            context.active,
            true,
        ),
        ConditionPredicate::DeltaAbs {
            reference,
            threshold,
        } => (
            evaluate_delta_abs(
                current,
                resolve_reference(target, *reference, context)?,
                &parse_threshold(target, threshold)?,
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
                resolve_reference(target, *reference, context)?,
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
                target,
                current,
                context.last_accepted_observation,
                &parse_threshold(target, threshold)?,
                *direction,
            )?,
            context.active,
            true,
        ),
        ConditionPredicate::Lt { threshold } => {
            let outcome =
                evaluate_ordered(current, &parse_threshold(target, threshold)?, |order| {
                    order.is_lt()
                });
            let active = next_level_active(outcome, context.active);
            (outcome, active, false)
        }
        ConditionPredicate::Gt { threshold } => {
            let outcome =
                evaluate_ordered(current, &parse_threshold(target, threshold)?, |order| {
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
            &parse_threshold(target, enter_threshold)?,
            &parse_threshold(target, exit_threshold)?,
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
        reference_evidence: configured_reference_evidence(target, condition.predicate(), context),
    })
}

fn configured_reference_evidence(
    target: &TargetDocument,
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
        Some(observation) if observation_matches_target(target, observation) => {
            ConditionReferenceEvidence::Resolved {
                reference,
                canonical_value: observation.canonical_value().to_owned(),
            }
        }
        Some(_) | None => ConditionReferenceEvidence::Unavailable { reference },
    })
}

fn resolve_reference(
    target: &TargetDocument,
    reference: ConditionReference,
    context: ConditionContext<'_>,
) -> Result<Option<PolicyValue>, CoreError> {
    let observation = match reference {
        ConditionReference::LastAcceptedObservation => context.last_accepted_observation,
        ConditionReference::FixedInitialBaseline => context.fixed_initial_baseline,
        ConditionReference::LastConditionTransition => context.last_condition_transition,
    };
    match observation {
        Some(value) if observation_matches_target(target, value) => parse_observation_value(value)
            .map(Some)
            .map_err(CoreError::contract),
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
    target: &TargetDocument,
    current: &PolicyValue,
    previous: Option<&Observation>,
    threshold: &PolicyValue,
    direction: ThresholdDirection,
) -> Result<ConditionOutcome, CoreError> {
    let Some(previous) = previous else {
        return Ok(ConditionOutcome::Unavailable);
    };
    if !observation_matches_target(target, previous) {
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

fn parse_threshold(target: &TargetDocument, threshold: &str) -> Result<PolicyValue, CoreError> {
    parse_config_value(target.declared_type(), target.type_params(), threshold)
        .map_err(CoreError::contract)
}

fn observation_matches_target(target: &TargetDocument, observation: &Observation) -> bool {
    observation.declared_type_for_policy() == target.declared_type()
        && observation.type_params_for_policy() == target.type_params()
}

#[cfg(test)]
mod coverage_tests {
    use super::super::types::StagedPolicyRun;
    use super::*;
    use crate::SourceSuspectReason;

    fn integer_target() -> TargetDocument {
        let source_path = crate::test_support::absolute_file_path("coverage.json");
        let target: TargetDocument = toml::from_str(&format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"coverage\"\ndisplay_name = \"Coverage\"\nenabled = true\nescalate_after = 1\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
        ))
        .expect("target TOML");
        target.validate().expect("valid target");
        target
    }

    #[test]
    fn defensive_policy_helpers_preserve_unavailability_and_issue_state() {
        let target = integer_target();
        let previous = target
            .parse_json_scalar_token("1".to_owned())
            .expect("observation");
        assert_eq!(
            evaluate_crosses(
                &target,
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
                &target,
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
        let staged = StagedPolicyRun::SourceSuspect {
            reason_class: SourceSuspectReason::FetchFailed,
            event_eligibilities: Vec::new(),
        };
        assert_eq!(staged.condition_evaluations(), None);
    }
}

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::CoreError;

use super::super::{Observation, PermanentErrorCode, SourceSuspectReason, TargetDocument};
use super::condition::{
    Condition, ConditionId, ConditionPredicate, ConditionReference, ThresholdDirection,
};
use super::value::{
    ArithmeticResult, PolicyValue, parse_config_value, parse_observation_value, parse_percentage,
};

/// The result of evaluating one named condition against one valid observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOutcome {
    /// The predicate is true for the supplied pre-run context.
    Satisfied,
    /// The predicate is false for the supplied pre-run context.
    NotSatisfied,
    /// A required named reference is absent or cannot be compared without conversion.
    Unavailable,
    /// Exact policy arithmetic exceeded its supported representation.
    ArithmeticOverflow,
    /// A percentage predicate encountered a zero runtime reference.
    ZeroReference,
}

/// The transient per-condition context supplied from the pre-run state.
#[derive(Clone, Copy, Debug)]
pub struct ConditionContext<'a> {
    last_accepted_observation: Option<&'a Observation>,
    fixed_initial_baseline: Option<&'a Observation>,
    last_condition_transition: Option<&'a Observation>,
    active: bool,
}

impl<'a> ConditionContext<'a> {
    /// Creates the full pre-run context for one currently evaluated condition.
    pub const fn new(
        last_accepted_observation: Option<&'a Observation>,
        fixed_initial_baseline: Option<&'a Observation>,
        last_condition_transition: Option<&'a Observation>,
        active: bool,
    ) -> Self {
        Self {
            last_accepted_observation,
            fixed_initial_baseline,
            last_condition_transition,
            active,
        }
    }

    pub(crate) const fn empty() -> Self {
        Self::new(None, None, None, false)
    }
}

/// One staged outcome with its trigger decision and next hysteresis state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionEvaluation {
    condition_id: ConditionId,
    outcome: ConditionOutcome,
    trigger: bool,
    active_after: bool,
}

impl ConditionEvaluation {
    pub(crate) const fn id(&self) -> &ConditionId {
        &self.condition_id
    }

    /// Returns the evaluated condition identifier.
    pub fn condition_id(&self) -> &str {
        self.condition_id.as_str()
    }

    /// Returns the exact condition outcome.
    pub const fn outcome(&self) -> ConditionOutcome {
        self.outcome
    }

    /// Returns whether this evaluation satisfies the predicate's trigger rule.
    pub const fn trigger(&self) -> bool {
        self.trigger
    }

    /// Returns the staged hysteresis state after this evaluation.
    pub const fn active_after(&self) -> bool {
        self.active_after
    }
}

/// A condition outcome that requires immediate `on_run` routing in a later delivery milestone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditionIssue {
    /// Exact policy arithmetic exceeded its supported representation.
    ArithmeticOverflow,
    /// A percentage predicate encountered a zero runtime reference.
    ZeroReference,
}

/// The cause that makes one immediate `on_run` event eligible for later durable delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OnRunEventCause {
    /// The operator completed a blind reset and began a fresh target-state lifecycle.
    Reset,
    /// The target accepted its first valid observation for the current clean state lifecycle.
    Initialized,
    /// One condition reached a reportable exact-arithmetic issue.
    ConditionIssue {
        /// Stable identifier of the condition that produced the issue.
        condition_id: ConditionId,
        /// The precise reportable issue.
        issue: ConditionIssue,
    },
    /// A source-suspect episode reached its configured escalation boundary.
    SourceSuspectEscalated {
        /// Stable reason classification for the source-suspect episode.
        reason_class: SourceSuspectReason,
    },
    /// A permanent contract-error episode began or changed identity.
    PermanentContractErrorEpisodeBegan {
        /// Stable classification for the permanent contract-error episode.
        error_code: PermanentErrorCode,
    },
}

/// A deterministic routing eligibility staged before M4 creates any durable outbox record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagedEventEligibility {
    /// A named condition satisfied its predicate-specific `on_condition` trigger rule.
    OnCondition {
        /// Stable identifier of the condition eligible for condition routing.
        condition_id: ConditionId,
    },
    /// An immediate run-level event is eligible for later `on_run` routing.
    OnRun {
        /// The policy or episode fact that made this run-level event eligible.
        cause: OnRunEventCause,
    },
}

/// A classified T0 input before any persistent mutation or delivery is allowed.
#[derive(Clone, Copy, Debug)]
pub enum PolicyRunInput<'a> {
    /// A permanent contract error, which must not advance accepted observations or conditions.
    PermanentContractError {
        /// Stable error classification for the permanent contract fault.
        error_code: PermanentErrorCode,
        /// Whether the staged permanent-error episode began during this run.
        episode_began: bool,
    },
    /// A source-suspect failure, which must not advance accepted observations or conditions.
    SourceSuspect {
        /// Stable reason classification for the source-suspect fault.
        reason_class: SourceSuspectReason,
        /// Whether the staged source-health episode reached its escalation boundary.
        escalation_reached: bool,
    },
    /// A valid observation eligible for condition evaluation and later baseline advancement.
    ValidObservation {
        /// The valid typed observation eligible for condition evaluation.
        observation: &'a Observation,
    },
}

/// A side-effect-free T0 staging result for one classified run input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagedPolicyRun {
    /// The permanent branch intentionally stages no accepted-observation or condition mutation.
    PermanentContractError {
        /// Stable error classification copied from the staged input.
        error_code: PermanentErrorCode,
        /// Deterministic routing eligibility for the staged permanent-error episode.
        event_eligibilities: Vec<StagedEventEligibility>,
    },
    /// The source-suspect branch intentionally stages no accepted-observation or condition mutation.
    SourceSuspect {
        /// Stable source-suspect reason copied from the staged input.
        reason_class: SourceSuspectReason,
        /// Deterministic routing eligibility for the staged source-suspect episode.
        event_eligibilities: Vec<StagedEventEligibility>,
    },
    /// The valid branch stages all policy outcomes before later state/outbox work.
    ValidObservation {
        /// One staged evaluation for each configured named condition.
        condition_evaluations: Vec<ConditionEvaluation>,
        /// Deterministic routing eligibility from the staged condition outcomes.
        event_eligibilities: Vec<StagedEventEligibility>,
    },
}

impl StagedPolicyRun {
    /// Returns the staged condition evaluations for a valid observation input.
    pub(crate) fn condition_evaluations(&self) -> Option<&[ConditionEvaluation]> {
        match self {
            Self::ValidObservation {
                condition_evaluations,
                ..
            } => Some(condition_evaluations),
            Self::PermanentContractError { .. } | Self::SourceSuspect { .. } => None,
        }
    }

    /// Returns the deterministic routing eligibilities that M4 must materialize without re-evaluation.
    pub fn event_eligibilities(&self) -> &[StagedEventEligibility] {
        match self {
            Self::PermanentContractError {
                event_eligibilities,
                ..
            }
            | Self::SourceSuspect {
                event_eligibilities,
                ..
            }
            | Self::ValidObservation {
                event_eligibilities,
                ..
            } => event_eligibilities,
        }
    }
}

pub(in crate::model) fn stage_policy_run(
    target: &TargetDocument,
    input: PolicyRunInput<'_>,
    contexts: &BTreeMap<ConditionId, ConditionContext<'_>>,
) -> Result<StagedPolicyRun, CoreError> {
    match input {
        PolicyRunInput::PermanentContractError { .. } => target.validate_without_projection()?,
        PolicyRunInput::SourceSuspect { .. } | PolicyRunInput::ValidObservation { .. } => {
            target.validate()?
        }
    }
    if contexts.keys().any(|context_id| {
        !target
            .conditions()
            .iter()
            .any(|condition| condition.id() == context_id)
    }) {
        return Err(CoreError::contract(
            "policy context contains a condition_id absent from the target contract",
        ));
    }
    match input {
        PolicyRunInput::PermanentContractError {
            error_code,
            episode_began,
        } => {
            let event_eligibilities = if episode_began {
                vec![StagedEventEligibility::OnRun {
                    cause: OnRunEventCause::PermanentContractErrorEpisodeBegan { error_code },
                }]
            } else {
                Vec::new()
            };
            Ok(StagedPolicyRun::PermanentContractError {
                error_code,
                event_eligibilities,
            })
        }
        PolicyRunInput::SourceSuspect {
            reason_class,
            escalation_reached,
        } => {
            let event_eligibilities = if escalation_reached {
                vec![StagedEventEligibility::OnRun {
                    cause: OnRunEventCause::SourceSuspectEscalated { reason_class },
                }]
            } else {
                Vec::new()
            };
            Ok(StagedPolicyRun::SourceSuspect {
                reason_class,
                event_eligibilities,
            })
        }
        PolicyRunInput::ValidObservation { observation } => {
            let condition_evaluations = evaluate_conditions(target, observation, contexts)?;
            let event_eligibilities = stage_condition_event_eligibilities(&condition_evaluations);
            Ok(StagedPolicyRun::ValidObservation {
                condition_evaluations,
                event_eligibilities,
            })
        }
    }
}

fn stage_condition_event_eligibilities(
    evaluations: &[ConditionEvaluation],
) -> Vec<StagedEventEligibility> {
    evaluations
        .iter()
        .flat_map(|evaluation| {
            let mut events = Vec::with_capacity(1);
            if evaluation.trigger() {
                events.push(StagedEventEligibility::OnCondition {
                    condition_id: evaluation.id().clone(),
                });
            }
            if let Some(issue) = condition_issue(evaluation.outcome()) {
                events.push(StagedEventEligibility::OnRun {
                    cause: OnRunEventCause::ConditionIssue {
                        condition_id: evaluation.id().clone(),
                        issue,
                    },
                });
            }
            events
        })
        .collect()
}

const fn condition_issue(outcome: ConditionOutcome) -> Option<ConditionIssue> {
    match outcome {
        ConditionOutcome::ArithmeticOverflow => Some(ConditionIssue::ArithmeticOverflow),
        ConditionOutcome::ZeroReference => Some(ConditionIssue::ZeroReference),
        ConditionOutcome::Satisfied
        | ConditionOutcome::NotSatisfied
        | ConditionOutcome::Unavailable => None,
    }
}

fn evaluate_conditions(
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
            ),
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
            ),
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
        active_after,
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
) -> ConditionOutcome {
    match reference {
        None => ConditionOutcome::Unavailable,
        Some(reference) => {
            arithmetic_outcome(current.exact_abs_delta_at_least(&reference, threshold))
        }
    }
}

fn evaluate_delta_pct(
    current: &PolicyValue,
    reference: Option<PolicyValue>,
    percentage: rust_decimal::Decimal,
) -> ConditionOutcome {
    match reference {
        None => ConditionOutcome::Unavailable,
        Some(reference) => {
            arithmetic_outcome(current.exact_percentage_delta_at_least(&reference, percentage))
        }
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
    use super::*;

    fn integer_target() -> TargetDocument {
        let target: TargetDocument = toml::from_str(
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"coverage\"\ndisplay_name = \"Coverage\"\nenabled = true\nescalate_after = 1\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = \"/tmp/coverage.json\"\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
        )
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

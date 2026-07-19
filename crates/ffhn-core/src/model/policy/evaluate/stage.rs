//! Side-effect-free staging of classified policy inputs.

use std::collections::BTreeMap;

use crate::CoreError;

use crate::TargetDocument;

use super::super::condition::ConditionId;
use super::predicates::evaluate_conditions;
use super::types::{
    ConditionContext, ConditionEvaluation, ConditionIssue, ConditionOutcome, OnRunEventCause,
    PolicyRunInput, StagedEventEligibility, StagedPolicyRun,
};

pub(in crate::model) fn stage_policy_run(
    target: &TargetDocument,
    input: PolicyRunInput<'_>,
    contexts: &BTreeMap<ConditionId, ConditionContext<'_>>,
) -> Result<StagedPolicyRun, CoreError> {
    match input {
        PolicyRunInput::PermanentContractError { .. } => target.validate_without_projection()?,
        PolicyRunInput::IntegrationFault { .. }
        | PolicyRunInput::SourceSuspect { .. }
        | PolicyRunInput::ValidObservation { .. } => target.validate()?,
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
        PolicyRunInput::IntegrationFault {
            integration_fault_code,
            episode_began,
        } => {
            let event_eligibilities = if episode_began {
                vec![StagedEventEligibility::OnRun {
                    cause: OnRunEventCause::IntegrationFaultEpisodeBegan {
                        integration_fault_code,
                    },
                }]
            } else {
                Vec::new()
            };
            Ok(StagedPolicyRun::IntegrationFault {
                integration_fault_code,
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

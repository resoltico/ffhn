use crate::model::{ProcessStdinEventKey, ProcessStdinPayload, StagedOutboxRecord};
use crate::{
    ConditionId, ConditionIssue, DeliveryEventKind, IntegrationFaultCode, OnRunEventCause,
    PermanentErrorCode, RouteFamily, SourceSuspectReason, StagedEventEligibility, StateDocument,
    TargetDocument, TargetId,
};

#[derive(Clone)]
struct EventFact {
    event_key: ProcessStdinEventKey,
    route_family: RouteFamily,
    summary: String,
    condition_id: Option<ConditionId>,
    observation_seq: Option<u64>,
    canonical_value: Option<String>,
    reason_class: Option<SourceSuspectReason>,
    error_code: Option<PermanentErrorCode>,
    integration_fault_code: Option<IntegrationFaultCode>,
    episode_started_at: Option<String>,
}

/// Materializes M2/M3 eligibilities into immutable route-specific outbox records.
///
/// This reads only the already staged next state and policy eligibilities. It never evaluates a
/// predicate or reconstructs a later run report.
pub(in crate::runtime) fn materialize(
    target: &TargetDocument,
    state: &StateDocument,
    eligibilities: &[StagedEventEligibility],
    contract_digest_sha256: &str,
) -> Result<Vec<StagedOutboxRecord>, crate::CoreError> {
    let target_id = TargetId::new(target.target_id())?;
    let facts = eligibilities
        .iter()
        .map(|eligibility| event_fact(target, state, eligibility, contract_digest_sha256))
        .collect::<Result<Vec<_>, _>>()?;
    let mut records = Vec::new();
    for fact in facts {
        for route in target.routes_for(fact.route_family) {
            let payload = ProcessStdinPayload::new(
                route.id(),
                fact.route_family,
                &target_id,
                target.display_name(),
                fact.event_key.clone(),
                &fact.summary,
                fact.condition_id.clone(),
                fact.observation_seq,
                fact.canonical_value.clone(),
                fact.reason_class,
                fact.error_code,
                fact.integration_fault_code,
                fact.episode_started_at.clone(),
            )?;
            records.push(StagedOutboxRecord {
                event_id: payload.event_id().to_owned(),
                route_id: route.id().clone(),
                route_family: fact.route_family,
                event_kind: fact.event_key.event_kind(),
                condition_id: fact.condition_id.clone(),
                immutable_payload: payload.immutable_bytes()?,
            });
        }
    }
    Ok(records)
}

fn event_fact(
    target: &TargetDocument,
    state: &StateDocument,
    eligibility: &StagedEventEligibility,
    contract_digest_sha256: &str,
) -> Result<EventFact, crate::CoreError> {
    match eligibility {
        StagedEventEligibility::OnCondition { condition_id } => {
            let condition = target.condition(condition_id).ok_or_else(|| {
                crate::CoreError::internal("policy staged a condition absent from its target")
            })?;
            let observation_seq = state.observation_seq();
            let event_key = if condition.predicate().is_event_predicate() {
                ProcessStdinEventKey::ConditionEvent {
                    condition_id: condition_id.clone(),
                    observation_seq,
                }
            } else {
                let entry_at = state.condition_transition_at(condition_id).ok_or_else(|| {
                    crate::CoreError::internal(
                        "level-condition trigger has no persisted entry transition",
                    )
                })?;
                ProcessStdinEventKey::ConditionLevel {
                    condition_id: condition_id.clone(),
                    entry_at: entry_at.to_owned(),
                }
            };
            let canonical_value = state
                .accepted_observation()
                .map(|observation| observation.canonical_value().to_owned());
            Ok(EventFact {
                event_key,
                route_family: RouteFamily::OnCondition,
                summary: format!(
                    "{}[condition={}]: satisfied at observation {}{}",
                    target.display_name(),
                    condition_id,
                    observation_seq,
                    canonical_value
                        .as_deref()
                        .map(|value| format!(" (value={value})"))
                        .unwrap_or_default()
                ),
                condition_id: Some(condition_id.clone()),
                observation_seq: Some(observation_seq),
                canonical_value,
                reason_class: None,
                error_code: None,
                integration_fault_code: None,
                episode_started_at: None,
            })
        }
        StagedEventEligibility::OnRun { cause } => {
            on_run_event_fact(target, state, cause, contract_digest_sha256)
        }
    }
}

fn on_run_event_fact(
    target: &TargetDocument,
    state: &StateDocument,
    cause: &OnRunEventCause,
    contract_digest_sha256: &str,
) -> Result<EventFact, crate::CoreError> {
    match cause {
        OnRunEventCause::Reset => Ok(EventFact {
            event_key: ProcessStdinEventKey::Reset {
                contract_digest_sha256: contract_digest_sha256.to_owned(),
            },
            route_family: RouteFamily::OnRun,
            summary: format!("{}: reset", target.display_name()),
            condition_id: None,
            observation_seq: None,
            canonical_value: None,
            reason_class: None,
            error_code: None,
            integration_fault_code: None,
            episode_started_at: None,
        }),
        OnRunEventCause::Initialized => Ok(EventFact {
            event_key: ProcessStdinEventKey::Initialized {
                contract_digest_sha256: contract_digest_sha256.to_owned(),
            },
            route_family: RouteFamily::OnRun,
            summary: format!("{}: initialized", target.display_name()),
            condition_id: None,
            observation_seq: Some(state.observation_seq()),
            canonical_value: state
                .accepted_observation()
                .map(|observation| observation.canonical_value().to_owned()),
            reason_class: None,
            error_code: None,
            integration_fault_code: None,
            episode_started_at: None,
        }),
        OnRunEventCause::ConditionIssue {
            condition_id,
            issue,
        } => {
            let event_kind = match issue {
                ConditionIssue::ArithmeticOverflow => DeliveryEventKind::ArithmeticOverflow,
                ConditionIssue::ZeroReference => DeliveryEventKind::ZeroReference,
            };
            let kind = event_kind.as_str();
            let observation_seq = state.observation_seq();
            Ok(EventFact {
                event_key: match issue {
                    ConditionIssue::ArithmeticOverflow => {
                        ProcessStdinEventKey::ArithmeticOverflow {
                            condition_id: condition_id.clone(),
                            observation_seq,
                        }
                    }
                    ConditionIssue::ZeroReference => ProcessStdinEventKey::ZeroReference {
                        condition_id: condition_id.clone(),
                        observation_seq,
                    },
                },
                route_family: RouteFamily::OnRun,
                summary: format!(
                    "{}[condition={}]: {kind} at observation {observation_seq}",
                    target.display_name(),
                    condition_id,
                ),
                condition_id: Some(condition_id.clone()),
                observation_seq: Some(observation_seq),
                canonical_value: state
                    .accepted_observation()
                    .map(|observation| observation.canonical_value().to_owned()),
                reason_class: None,
                error_code: None,
                integration_fault_code: None,
                episode_started_at: None,
            })
        }
        OnRunEventCause::SourceSuspectEscalated { reason_class } => {
            let episode = state.source_episode_started_at(*reason_class).ok_or_else(|| {
                crate::CoreError::internal(
                    "source escalation eligibility has no matching persisted source-health episode",
                )
            })?;
            let reason = reason_class.as_str();
            Ok(EventFact {
                event_key: ProcessStdinEventKey::SourceSuspectEscalated {
                    reason_class: *reason_class,
                    episode: episode.to_owned(),
                },
                route_family: RouteFamily::OnRun,
                summary: format!(
                    "{}: source health escalated ({reason})",
                    target.display_name()
                ),
                condition_id: None,
                observation_seq: None,
                canonical_value: None,
                reason_class: Some(*reason_class),
                error_code: None,
                integration_fault_code: None,
                episode_started_at: Some(episode.to_owned()),
            })
        }
        OnRunEventCause::PermanentContractErrorEpisodeBegan { error_code } => {
            let first_seen_at =
                state
                    .permanent_episode_started_at(*error_code)
                    .ok_or_else(|| {
                        crate::CoreError::internal(
                            "permanent-error eligibility has no matching persisted error episode",
                        )
                    })?;
            let code = error_code.as_str();
            Ok(EventFact {
                event_key: ProcessStdinEventKey::PermanentContractError {
                    contract_digest_sha256: contract_digest_sha256.to_owned(),
                    error_code: *error_code,
                    first_seen_at: first_seen_at.to_owned(),
                },
                route_family: RouteFamily::OnRun,
                summary: format!(
                    "{}: permanent contract error ({code})",
                    target.display_name()
                ),
                condition_id: None,
                observation_seq: None,
                canonical_value: None,
                reason_class: None,
                error_code: Some(*error_code),
                integration_fault_code: None,
                episode_started_at: Some(first_seen_at.to_owned()),
            })
        }
        OnRunEventCause::IntegrationFaultEpisodeBegan {
            integration_fault_code,
        } => {
            let first_seen_at = state
                .integration_fault_episode_started_at(*integration_fault_code)
                .ok_or_else(|| {
                    crate::CoreError::internal(
                        "integration-fault eligibility has no matching persisted episode",
                    )
                })?;
            let code = integration_fault_code.as_str();
            Ok(EventFact {
                event_key: ProcessStdinEventKey::IntegrationFault {
                    contract_digest_sha256: contract_digest_sha256.to_owned(),
                    integration_fault_code: *integration_fault_code,
                    first_seen_at: first_seen_at.to_owned(),
                },
                route_family: RouteFamily::OnRun,
                summary: format!("{}: integration fault ({code})", target.display_name()),
                condition_id: None,
                observation_seq: None,
                canonical_value: None,
                reason_class: None,
                error_code: None,
                integration_fault_code: Some(*integration_fault_code),
                episode_started_at: Some(first_seen_at.to_owned()),
            })
        }
    }
}

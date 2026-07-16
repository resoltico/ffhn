use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::model::{
    ProcessStdinEventKey, ProcessStdinEventKind, ProcessStdinPayload, StagedOutboxRecord,
};
use crate::{
    ConditionId, ConditionIssue, DeliveryOutcome, OnRunEventCause, OutboxPolicy,
    PermanentErrorCode, RouteFamily, SourceSuspectReason, StagedEventEligibility, StateDocument,
    TargetDocument, TargetId,
};

use super::acquire::now_utc;
use super::storage::write_state;
use crate::TargetPaths;

pub(crate) mod process;

/// Results accumulated while draining all currently due outbox records.
#[derive(Debug)]
pub(super) struct DeliveryDrain {
    outcomes: Vec<DeliveryOutcome>,
}

impl DeliveryDrain {
    pub(super) fn outcomes(&self) -> Vec<DeliveryOutcome> {
        self.outcomes.clone()
    }
}

/// A post-commit outbox-state persistence failure that still retains attempt evidence.
pub(super) struct DeliveryDrainFailure {
    pub(super) drain: DeliveryDrain,
    pub(super) error: crate::CoreError,
}

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
    episode_started_at: Option<String>,
}

/// Materializes M2/M3 eligibilities into immutable route-specific outbox records.
///
/// This reads only the already staged next state and policy eligibilities. It never evaluates a
/// predicate or reconstructs a later run report.
pub(super) fn materialize(
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
                fact.episode_started_at.clone(),
            )?;
            records.push(StagedOutboxRecord {
                event_id: payload.event_id().to_owned(),
                route_id: route.id().clone(),
                route_family: fact.route_family,
                immutable_payload: payload.immutable_bytes()?,
            });
        }
    }
    Ok(records)
}

/// Drains every currently due record after the state/outbox creation commit has succeeded.
pub(super) fn drain(
    paths: &TargetPaths,
    target: &TargetDocument,
    state: &mut StateDocument,
) -> Result<DeliveryDrain, DeliveryDrainFailure> {
    drain_with_clock(paths, target, state, &mut now_utc)
}

fn drain_with_clock<F>(
    paths: &TargetPaths,
    target: &TargetDocument,
    state: &mut StateDocument,
    now: &mut F,
) -> Result<DeliveryDrain, DeliveryDrainFailure>
where
    F: FnMut() -> Result<String, crate::CoreError>,
{
    drain_with_clock_and_persist(target, state, now, &mut |state| write_state(paths, state))
}

/// Drains due records using the supplied clock and persistence boundary.
///
/// The persistence boundary is injected only to make the externally observable, post-process
/// durability failures deterministic in tests. Production always persists through `write_state`.
fn drain_with_clock_and_persist<F, P>(
    target: &TargetDocument,
    state: &mut StateDocument,
    now: &mut F,
    persist: &mut P,
) -> Result<DeliveryDrain, DeliveryDrainFailure>
where
    F: FnMut() -> Result<String, crate::CoreError>,
    P: FnMut(&StateDocument) -> Result<(), crate::CoreError>,
{
    drain_with_dependencies(
        target,
        state,
        now,
        persist,
        &mut |state, event_id, route_id, error, next_retry_at| {
            state.record_outbox_failure(event_id, route_id, error, next_retry_at)
        },
    )
}

fn drain_with_dependencies<F, P, R>(
    target: &TargetDocument,
    state: &mut StateDocument,
    now: &mut F,
    persist: &mut P,
    record_failure: &mut R,
) -> Result<DeliveryDrain, DeliveryDrainFailure>
where
    F: FnMut() -> Result<String, crate::CoreError>,
    P: FnMut(&StateDocument) -> Result<(), crate::CoreError>,
    R: FnMut(
        &mut StateDocument,
        &str,
        &crate::RouteId,
        String,
        String,
    ) -> Result<u32, crate::CoreError>,
{
    let mut outcomes = Vec::new();
    // A retry is scheduled from the retry-state commit timestamp, but it must not become due in
    // this same drain merely because durable persistence took longer than its configured delay.
    // The snapshot still lets this drain process every record that was due when it began.
    let drain_started_at = now().map_err(|error| DeliveryDrainFailure {
        drain: DeliveryDrain {
            outcomes: outcomes.clone(),
        },
        error,
    })?;
    loop {
        let record = state
            .next_due_outbox_record(&drain_started_at)
            .map_err(|error| DeliveryDrainFailure {
                drain: DeliveryDrain {
                    outcomes: outcomes.clone(),
                },
                error,
            })?;
        let Some(record) = record else {
            return Ok(DeliveryDrain { outcomes });
        };
        let route = target
            .route(record.route_id())
            .ok_or_else(|| DeliveryDrainFailure {
                drain: DeliveryDrain {
                    outcomes: outcomes.clone(),
                },
                error: crate::CoreError::internal(
                    "validated outbox route disappeared before delivery",
                ),
            })?;
        let event_id = record.event_id().to_owned();
        let route_id = record.route_id().clone();
        let prior_attempts = record.attempt_count();
        if prior_attempts >= target.outbox().max_attempts() {
            let error = record
                .last_error()
                .unwrap_or("delivery attempts exhausted before this drain")
                .to_owned();
            if let Err(outbox_error) = state
                .remove_outbox_record(&event_id, &route_id)
                .and_then(|()| persist(state))
            {
                outcomes.push(DeliveryOutcome::dead_letter_uncommitted(
                    event_id,
                    route_id.to_string(),
                    prior_attempts,
                    error,
                    &outbox_error,
                ));
                return Err(DeliveryDrainFailure {
                    drain: DeliveryDrain { outcomes },
                    error: outbox_error,
                });
            }
            outcomes.push(DeliveryOutcome::dead_lettered(
                event_id,
                route_id.to_string(),
                prior_attempts,
                error,
            ));
            continue;
        }
        match process::deliver(route, record.immutable_payload()) {
            Ok(()) => {
                if let Err(outbox_error) = state
                    .remove_outbox_record(&event_id, &route_id)
                    .and_then(|()| persist(state))
                {
                    outcomes.push(DeliveryOutcome::delivered_uncommitted(
                        event_id,
                        route_id.to_string(),
                        prior_attempts.saturating_add(1),
                        &outbox_error,
                    ));
                    return Err(DeliveryDrainFailure {
                        drain: DeliveryDrain { outcomes },
                        error: outbox_error,
                    });
                }
                outcomes.push(DeliveryOutcome::delivered(
                    event_id,
                    route_id.to_string(),
                    prior_attempts.saturating_add(1),
                ));
            }
            Err(error) => {
                let attempt_count = prior_attempts.saturating_add(1);
                let retry_commit_time = match now() {
                    Ok(retry_commit_time) => retry_commit_time,
                    Err(outbox_error) => {
                        outcomes.push(DeliveryOutcome::retry_uncommitted(
                            event_id,
                            route_id.to_string(),
                            attempt_count,
                            &error,
                            &outbox_error,
                        ));
                        return Err(DeliveryDrainFailure {
                            drain: DeliveryDrain { outcomes },
                            error: outbox_error,
                        });
                    }
                };
                let next_retry_at =
                    match retry_at(&retry_commit_time, attempt_count, target.outbox()) {
                        Ok(next_retry_at) => next_retry_at,
                        Err(outbox_error) => {
                            outcomes.push(DeliveryOutcome::retry_uncommitted(
                                event_id,
                                route_id.to_string(),
                                attempt_count,
                                &error,
                                &outbox_error,
                            ));
                            return Err(DeliveryDrainFailure {
                                drain: DeliveryDrain { outcomes },
                                error: outbox_error,
                            });
                        }
                    };
                let attempt_count =
                    match record_failure(state, &event_id, &route_id, error.clone(), next_retry_at)
                    {
                        Ok(attempt_count) => attempt_count,
                        Err(outbox_error) => {
                            outcomes.push(DeliveryOutcome::retry_uncommitted(
                                event_id,
                                route_id.to_string(),
                                attempt_count,
                                &error,
                                &outbox_error,
                            ));
                            return Err(DeliveryDrainFailure {
                                drain: DeliveryDrain { outcomes },
                                error: outbox_error,
                            });
                        }
                    };
                let terminal = attempt_count >= target.outbox().max_attempts();
                if terminal {
                    if let Err(outbox_error) = state
                        .remove_outbox_record(&event_id, &route_id)
                        .and_then(|()| persist(state))
                    {
                        outcomes.push(DeliveryOutcome::dead_letter_uncommitted(
                            event_id,
                            route_id.to_string(),
                            attempt_count,
                            &error,
                            &outbox_error,
                        ));
                        return Err(DeliveryDrainFailure {
                            drain: DeliveryDrain { outcomes },
                            error: outbox_error,
                        });
                    }
                    outcomes.push(DeliveryOutcome::dead_lettered(
                        event_id,
                        route_id.to_string(),
                        attempt_count,
                        error,
                    ));
                } else {
                    if let Err(outbox_error) = persist(state) {
                        outcomes.push(DeliveryOutcome::retry_uncommitted(
                            event_id,
                            route_id.to_string(),
                            attempt_count,
                            &error,
                            &outbox_error,
                        ));
                        return Err(DeliveryDrainFailure {
                            drain: DeliveryDrain { outcomes },
                            error: outbox_error,
                        });
                    }
                    outcomes.push(DeliveryOutcome::retry_scheduled(
                        event_id,
                        route_id.to_string(),
                        attempt_count,
                        error,
                    ));
                }
            }
        }
    }
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
            episode_started_at: None,
        }),
        OnRunEventCause::ConditionIssue {
            condition_id,
            issue,
        } => {
            let event_kind = match issue {
                ConditionIssue::ArithmeticOverflow => ProcessStdinEventKind::ArithmeticOverflow,
                ConditionIssue::ZeroReference => ProcessStdinEventKind::ZeroReference,
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
                episode_started_at: Some(first_seen_at.to_owned()),
            })
        }
    }
}

fn retry_at(
    commit_time: &str,
    attempt_count: u32,
    policy: &OutboxPolicy,
) -> Result<String, crate::CoreError> {
    let commit_time = OffsetDateTime::parse(commit_time, &Rfc3339)?;
    let exponent = attempt_count.saturating_sub(1).min(63);
    let multiplier = 1_u64.checked_shl(exponent).unwrap_or(u64::MAX);
    let delay = policy
        .base_backoff_ms()
        .saturating_mul(multiplier)
        .min(policy.max_backoff_ms());
    (commit_time + Duration::milliseconds(delay as i64))
        .format(&Rfc3339)
        .map_err(crate::CoreError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DeliveryStatus, PermanentErrorCode, PolicyRunInput, ProcessErrorDetail, ProcessErrorKind,
        SourceSuspectReason, TargetId,
    };

    const FIRST: &str = "2026-07-15T12:00:00Z";
    const SECOND: &str = "2026-07-15T12:00:01Z";

    fn routed_target() -> TargetDocument {
        let source_path = crate::test_support::absolute_file_path("source.json");
        let program = crate::test_support::PROCESS_PROGRAM;
        let target: TargetDocument = toml::from_str(&format!(
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n\n[[conditions]]\ncondition_id = \"changed\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"\n\n[[conditions]]\ncondition_id = \"low\"\n\n[conditions.predicate]\nkind = \"lt\"\nthreshold = \"75\"\n\n[[routes]]\nroute_id = \"condition\"\nroute_family = \"on_condition\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = {program:?}\ntimeout_ms = 1000\n\n[[routes]]\nroute_id = \"run\"\nroute_family = \"on_run\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = {program:?}\ntimeout_ms = 1000\n",
        ))
        .expect("target TOML");
        target.validate().expect("valid target");
        target
    }

    fn observation(target: &TargetDocument, value: i64) -> crate::Observation {
        target
            .parse_json_scalar_token(value.to_string())
            .expect("typed observation")
    }

    fn state_with_condition_facts(target: &TargetDocument) -> StateDocument {
        let digest = target.contract_digest_sha256().expect("contract digest");
        let mut state = StateDocument::new(TargetId::new("demo").expect("target id"), digest);
        let first = observation(target, 100);
        let staged = target
            .stage_policy_run(
                PolicyRunInput::ValidObservation {
                    observation: &first,
                },
                &state.condition_contexts(target),
            )
            .expect("initial policy stage");
        state
            .apply_valid_observation(
                target,
                first,
                staged.condition_evaluations().expect("evaluations"),
                FIRST,
            )
            .expect("initial state");

        let second = observation(target, 50);
        let staged = target
            .stage_policy_run(
                PolicyRunInput::ValidObservation {
                    observation: &second,
                },
                &state.condition_contexts(target),
            )
            .expect("second policy stage");
        state
            .apply_valid_observation(
                target,
                second,
                staged.condition_evaluations().expect("evaluations"),
                SECOND,
            )
            .expect("second state");
        state
    }

    fn failing_delivery_target(max_attempts: u32) -> TargetDocument {
        let mut value = serde_json::to_value(routed_target()).expect("target value");
        value["outbox"] = serde_json::json!({
            "max_pending": 100,
            "max_attempts": max_attempts,
            "base_backoff_ms": 1_000,
            "max_backoff_ms": 1_000,
        });
        let (program, args) = process::test_process_command("fail", None);
        value["routes"][0]["adapter"]["program"] = serde_json::json!(program);
        value["routes"][0]["adapter"]["args"] = serde_json::json!(args);
        let target: TargetDocument = serde_json::from_value(value).expect("failing target");
        target.validate().expect("valid failing target");
        target
    }

    fn queued_delivery_state(target: &TargetDocument, _event_id: &str) -> StateDocument {
        let mut state = StateDocument::new(
            TargetId::new("demo").expect("target id"),
            target.contract_digest_sha256().expect("digest"),
        );
        let route_id = "condition".parse().expect("route id");
        let payload = ProcessStdinPayload::new(
            &route_id,
            RouteFamily::OnCondition,
            &TargetId::new("demo").expect("target id"),
            "Demo",
            ProcessStdinEventKey::ConditionEvent {
                condition_id: "changed".parse().expect("condition id"),
                observation_seq: 1,
            },
            "Demo[condition=changed]: satisfied at observation 1 (value=1)",
            Some("changed".parse().expect("condition id")),
            Some(1),
            Some("1".to_owned()),
            None,
            None,
            None,
        )
        .expect("payload");
        let event_id = payload.event_id().to_owned();
        let payload = payload.immutable_bytes().expect("payload bytes");
        state
            .enqueue_outbox(
                vec![StagedOutboxRecord {
                    event_id,
                    route_id,
                    route_family: RouteFamily::OnCondition,
                    immutable_payload: payload,
                }],
                target.outbox(),
                FIRST,
            )
            .expect("queued delivery");
        state
    }

    fn queued_event_id(state: &StateDocument) -> String {
        state
            .next_due_outbox_record(FIRST)
            .expect("queued state")
            .expect("queued record")
            .event_id()
            .to_owned()
    }

    #[test]
    fn materialization_preserves_all_eligibility_facts_in_immutable_route_payloads() {
        let target = routed_target();
        let digest = target.contract_digest_sha256().expect("contract digest");
        let state = state_with_condition_facts(&target);
        let eligibilities = [
            StagedEventEligibility::OnCondition {
                condition_id: "changed".parse().expect("condition id"),
            },
            StagedEventEligibility::OnCondition {
                condition_id: "low".parse().expect("condition id"),
            },
            StagedEventEligibility::OnRun {
                cause: OnRunEventCause::Initialized,
            },
            StagedEventEligibility::OnRun {
                cause: OnRunEventCause::Reset,
            },
            StagedEventEligibility::OnRun {
                cause: OnRunEventCause::ConditionIssue {
                    condition_id: "changed".parse().expect("condition id"),
                    issue: ConditionIssue::ArithmeticOverflow,
                },
            },
            StagedEventEligibility::OnRun {
                cause: OnRunEventCause::ConditionIssue {
                    condition_id: "low".parse().expect("condition id"),
                    issue: ConditionIssue::ZeroReference,
                },
            },
        ];
        let records = materialize(&target, &state, &eligibilities, &digest).expect("records");
        assert_eq!(records.len(), eligibilities.len());
        let payloads = records
            .iter()
            .map(|record| serde_json::from_slice::<serde_json::Value>(&record.immutable_payload))
            .collect::<Result<Vec<_>, _>>()
            .expect("payload JSON");
        for payload in &payloads {
            assert_eq!(payload["schema_name"], "ffhn.process_stdin");
            assert_eq!(payload["schema_version"], 2);
            assert_eq!(payload["target_id"], "demo");
        }
        assert_eq!(payloads[0]["route_id"], "condition");
        assert_eq!(payloads[0]["event_kind"], "condition_satisfied");
        assert_eq!(payloads[0]["condition_id"], "changed");
        assert_eq!(payloads[0]["observation_seq"], 2);
        assert_eq!(payloads[1]["condition_id"], "low");
        assert_eq!(
            payloads[1]["event_key"],
            serde_json::json!({
                "key_kind": "condition_level",
                "condition_id": "low",
                "entry_at": SECOND,
            })
        );
        assert_eq!(payloads[2]["route_id"], "run");
        assert_eq!(payloads[2]["event_kind"], "initialized");
        assert_eq!(payloads[3]["event_kind"], "reset");
        assert_eq!(payloads[4]["event_kind"], "arithmetic_overflow");
        assert_eq!(payloads[5]["event_kind"], "zero_reference");

        let changed_id = crate::stable_json::stable_digest(&serde_json::json!({
            "target_id": "demo",
            "route_family": "on_condition",
            "key": {"condition_id": "changed", "observation_seq": 2},
        }))
        .expect("changed id");
        assert_eq!(records[0].event_id, changed_id);
        let low_id = crate::stable_json::stable_digest(&serde_json::json!({
            "target_id": "demo",
            "route_family": "on_condition",
            "key": {"condition_id": "low", "entry_at": SECOND},
        }))
        .expect("level id");
        assert_eq!(records[1].event_id, low_id);
    }

    #[test]
    fn on_run_health_and_contract_episodes_have_stable_keys_and_missing_facts_fail_closed() {
        let target = routed_target();
        let digest = target.contract_digest_sha256().expect("contract digest");
        let mut state =
            StateDocument::new(TargetId::new("demo").expect("target id"), digest.clone());
        let detail = ProcessErrorDetail::new(ProcessErrorKind::Io, "source unavailable", None);
        assert!(
            state
                .apply_source_suspect(SourceSuspectReason::FetchFailed, detail, FIRST, 1)
                .expect("source episode")
        );
        let source = materialize(
            &target,
            &state,
            &[StagedEventEligibility::OnRun {
                cause: OnRunEventCause::SourceSuspectEscalated {
                    reason_class: SourceSuspectReason::FetchFailed,
                },
            }],
            &digest,
        )
        .expect("source materialization");
        let payload: serde_json::Value =
            serde_json::from_slice(&source[0].immutable_payload).expect("source payload");
        assert_eq!(payload["reason_class"], "fetch_failed");
        assert_eq!(payload["episode_started_at"], FIRST);

        assert!(
            state
                .apply_permanent_error(PermanentErrorCode::InvalidJsonPointer, SECOND)
                .expect("permanent episode")
        );
        let permanent = materialize(
            &target,
            &state,
            &[StagedEventEligibility::OnRun {
                cause: OnRunEventCause::PermanentContractErrorEpisodeBegan {
                    error_code: PermanentErrorCode::InvalidJsonPointer,
                },
            }],
            &digest,
        )
        .expect("permanent materialization");
        let payload: serde_json::Value =
            serde_json::from_slice(&permanent[0].immutable_payload).expect("permanent payload");
        assert_eq!(payload["error_code"], "invalid_json_pointer");
        assert_eq!(payload["episode_started_at"], SECOND);

        let empty = StateDocument::new(TargetId::new("demo").expect("target id"), digest.clone());
        assert!(
            materialize(
                &target,
                &empty,
                &[StagedEventEligibility::OnCondition {
                    condition_id: "missing".parse().expect("condition id"),
                }],
                &digest,
            )
            .is_err()
        );
        assert!(
            materialize(
                &target,
                &empty,
                &[StagedEventEligibility::OnCondition {
                    condition_id: "low".parse().expect("condition id"),
                }],
                &digest,
            )
            .is_err()
        );
        assert!(
            materialize(
                &target,
                &empty,
                &[StagedEventEligibility::OnRun {
                    cause: OnRunEventCause::SourceSuspectEscalated {
                        reason_class: SourceSuspectReason::FetchFailed,
                    },
                }],
                &digest,
            )
            .is_err()
        );
        assert!(
            materialize(
                &target,
                &empty,
                &[StagedEventEligibility::OnRun {
                    cause: OnRunEventCause::PermanentContractErrorEpisodeBegan {
                        error_code: PermanentErrorCode::InvalidJsonPointer,
                    },
                }],
                &digest,
            )
            .is_err()
        );
    }

    #[test]
    fn drain_preserves_a_clock_failure_as_post_commit_delivery_evidence() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let watch_root = temporary.path().join("watchlist");
        std::fs::create_dir_all(&watch_root).expect("watch root");
        let paths = TargetPaths::try_new(&watch_root, "demo").expect("target paths");
        let target = routed_target();
        let mut state = StateDocument::new(
            TargetId::new("demo").expect("target id"),
            target.contract_digest_sha256().expect("digest"),
        );
        let failure = drain_with_clock(&paths, &target, &mut state, &mut || {
            Err(crate::CoreError::internal("clock failed"))
        })
        .expect_err("clock failure");
        assert!(failure.error.to_string().contains("clock failed"));
        assert!(failure.drain.outcomes().is_empty());
    }

    #[test]
    fn drain_rejects_invalid_due_records_and_missing_routes_before_delivery() {
        let target = routed_target();
        let queued = queued_delivery_state(&target, &"b".repeat(64));
        let mut wire = serde_json::to_value(&queued).expect("queued state JSON");
        wire["outbox"][0]["next_retry_at"] = serde_json::json!("not-a-timestamp");
        let mut invalid_due: StateDocument =
            serde_json::from_value(wire).expect("structurally valid state");
        let mut clock = || Ok(FIRST.to_owned());
        let mut persist = |_: &StateDocument| Ok(());
        let failure =
            drain_with_clock_and_persist(&target, &mut invalid_due, &mut clock, &mut persist)
                .expect_err("invalid pending retry time");
        assert!(failure.error.to_string().contains("outbox timestamp"));
        assert!(failure.drain.outcomes().is_empty());

        let queued = queued_delivery_state(&target, &"c".repeat(64));
        let mut wire = serde_json::to_value(&queued).expect("queued state JSON");
        wire["outbox"][0]["route_id"] = serde_json::json!("missing");
        let mut missing_route: StateDocument =
            serde_json::from_value(wire).expect("structurally valid state");
        let mut clock = || Ok(FIRST.to_owned());
        let mut persist = |_: &StateDocument| Ok(());
        let failure =
            drain_with_clock_and_persist(&target, &mut missing_route, &mut clock, &mut persist)
                .expect_err("missing delivery route");
        assert!(
            failure
                .error
                .to_string()
                .contains("outbox route disappeared")
        );
        assert!(failure.drain.outcomes().is_empty());
    }

    #[test]
    fn drain_reports_every_uncommitted_outbox_transition() {
        let target = failing_delivery_target(1);
        let mut exhausted = queued_delivery_state(&target, &"e".repeat(64));
        let exhausted_id = queued_event_id(&exhausted);
        exhausted
            .record_outbox_failure(
                &exhausted_id,
                &"condition".parse().expect("route id"),
                "earlier delivery failure".to_owned(),
                FIRST.to_owned(),
            )
            .expect("exhaust prior attempt");
        let mut clock = || Ok(FIRST.to_owned());
        let mut persist = |_: &StateDocument| Err(crate::CoreError::internal("persist refused"));
        let failure =
            drain_with_clock_and_persist(&target, &mut exhausted, &mut clock, &mut persist)
                .expect_err("preexisting terminal removal cannot persist");
        let outcomes = failure.drain.outcomes();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].status(), DeliveryStatus::DeadLetterUncommitted);
        assert_eq!(outcomes[0].attempt_count(), 1);

        let mut terminal = queued_delivery_state(&target, &"d".repeat(64));
        let mut clock = || Ok(FIRST.to_owned());
        let mut persist = |_: &StateDocument| Err(crate::CoreError::internal("persist refused"));
        let failure =
            drain_with_clock_and_persist(&target, &mut terminal, &mut clock, &mut persist)
                .expect_err("terminal removal cannot persist");
        let outcomes = failure.drain.outcomes();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].status(), DeliveryStatus::DeadLetterUncommitted);
        assert_eq!(outcomes[0].attempt_count(), 1);

        let retry_target = failing_delivery_target(2);
        let mut retry = queued_delivery_state(&retry_target, &"a".repeat(64));
        let mut clock = || Ok(FIRST.to_owned());
        let mut persist = |_: &StateDocument| Err(crate::CoreError::internal("persist refused"));
        let failure =
            drain_with_clock_and_persist(&retry_target, &mut retry, &mut clock, &mut persist)
                .expect_err("retry state cannot persist");
        let outcomes = failure.drain.outcomes();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].status(), DeliveryStatus::RetryUncommitted);
        assert_eq!(outcomes[0].attempt_count(), 1);
    }

    #[test]
    fn failed_delivery_reports_retry_commit_clock_and_recording_failures_as_uncommitted() {
        let target = failing_delivery_target(2);

        let mut after_delivery_clock = queued_delivery_state(&target, &"f".repeat(64));
        let mut calls = 0;
        let mut clock = || {
            calls += 1;
            if calls == 1 {
                Ok(FIRST.to_owned())
            } else {
                Err(crate::CoreError::internal("retry-commit clock failed"))
            }
        };
        let mut persist = |_: &StateDocument| Ok(());
        let failure = drain_with_clock_and_persist(
            &target,
            &mut after_delivery_clock,
            &mut clock,
            &mut persist,
        )
        .expect_err("retry-commit clock failure");
        let outcomes = failure.drain.outcomes();
        assert_eq!(outcomes[0].status(), DeliveryStatus::RetryUncommitted);
        assert!(
            outcomes[0]
                .error()
                .is_some_and(|error| error.contains("clock failed"))
        );

        let mut invalid_retry_commit_time = queued_delivery_state(&target, &"1".repeat(64));
        let mut calls = 0;
        let mut clock = || {
            calls += 1;
            Ok(if calls == 1 {
                FIRST.to_owned()
            } else {
                "not-a-timestamp".to_owned()
            })
        };
        let mut persist = |_: &StateDocument| Ok(());
        let failure = drain_with_clock_and_persist(
            &target,
            &mut invalid_retry_commit_time,
            &mut clock,
            &mut persist,
        )
        .expect_err("invalid retry-commit timestamp");
        let outcomes = failure.drain.outcomes();
        assert_eq!(outcomes[0].status(), DeliveryStatus::RetryUncommitted);
        assert!(outcomes[0].error().is_some());

        let mut recording = queued_delivery_state(&target, &"2".repeat(64));
        let mut clock = || Ok(FIRST.to_owned());
        let mut persist = |_: &StateDocument| Ok(());
        let mut record_failure =
            |_: &mut StateDocument, _: &str, _: &crate::RouteId, _: String, _: String| {
                Err(crate::CoreError::internal("record retry failed"))
            };
        let failure = drain_with_dependencies(
            &target,
            &mut recording,
            &mut clock,
            &mut persist,
            &mut record_failure,
        )
        .expect_err("recording failure");
        let outcomes = failure.drain.outcomes();
        assert_eq!(outcomes[0].status(), DeliveryStatus::RetryUncommitted);
        assert!(
            outcomes[0]
                .error()
                .is_some_and(|error| error.contains("record retry failed"))
        );
    }

    #[test]
    fn retry_backoff_is_exact_deterministic_and_capped() {
        let policy: OutboxPolicy = serde_json::from_value(serde_json::json!({
            "max_pending": 3,
            "max_attempts": 4,
            "base_backoff_ms": 100,
            "max_backoff_ms": 250,
        }))
        .expect("outbox policy");
        let base = OffsetDateTime::parse("2026-01-01T00:00:00Z", &Rfc3339).expect("base");
        for (attempt, expected_ms) in [(1, 100), (2, 200), (3, 250), (20, 250)] {
            let retry = retry_at("2026-01-01T00:00:00Z", attempt, &policy).expect("retry");
            let retry = OffsetDateTime::parse(&retry, &Rfc3339).expect("retry timestamp");
            assert_eq!(
                (retry - base).whole_milliseconds(),
                expected_ms,
                "{attempt}"
            );
        }
    }

    #[test]
    fn retry_state_commit_is_exact_and_waits_for_a_later_drain() {
        let target = failing_delivery_target(2);
        let mut state = queued_delivery_state(&target, &"f".repeat(64));
        let mut clock_values = [FIRST.to_owned(), SECOND.to_owned()].into_iter();
        let mut calls = 0;
        let mut clock = || {
            calls += 1;
            Ok(clock_values.next().expect("no unexpected clock read"))
        };
        let mut persist = |_: &StateDocument| Ok(());

        let drain = drain_with_clock_and_persist(&target, &mut state, &mut clock, &mut persist)
            .ok()
            .expect("retry remains pending for a later drain");

        let outcomes = drain.outcomes();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].status(), DeliveryStatus::RetryScheduled);
        assert_eq!(outcomes[0].attempt_count(), 1);
        assert_eq!(calls, 2);
        let state_json = serde_json::to_value(&state).expect("state JSON");
        assert_eq!(state_json["outbox"][0]["attempt_count"], 1);
        assert_eq!(
            state_json["outbox"][0]["next_retry_at"],
            "2026-07-15T12:00:02Z"
        );
    }
}

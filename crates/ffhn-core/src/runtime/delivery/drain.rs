use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::model::{
    delivery_failure_detail, delivery_observability_detail, detail_from_core_error,
};
use crate::{
    CoreError, DeliveryOutcome, DiagnosticDetail, DiagnosticOperation, OutboxPolicy, StateDocument,
    TargetDocument, TargetPaths,
};

use super::super::acquire::now_utc;
use super::super::storage::write_state;
use super::process;
use super::{DeliveryDrain, DeliveryDrainFailure};

/// Drains every currently due record after the state/outbox creation commit has succeeded.
pub(in crate::runtime) fn drain(
    paths: &TargetPaths,
    target: &TargetDocument,
    state: &mut StateDocument,
) -> Result<DeliveryDrain, DeliveryDrainFailure> {
    drain_with_clock(paths, target, state, &mut now_utc)
}

pub(in crate::runtime) fn drain_with_clock<F>(
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
pub(in crate::runtime) fn drain_with_clock_and_persist<F, P>(
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
        &mut |state, event_id, route_id, error_detail, next_retry_at| {
            state.record_outbox_failure(event_id, route_id, error_detail, next_retry_at)
        },
    )
}

pub(in crate::runtime) fn drain_with_dependencies<F, P, R>(
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
        DiagnosticDetail,
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
        let event_kind = record.event_kind();
        let condition_id = record.condition_id().map(ToString::to_string);
        let prior_attempts = record.attempt_count();
        if prior_attempts >= target.outbox().max_attempts() {
            let error_detail = exhausted_error_detail_or_drain_failure(
                record.last_error_detail().cloned(),
                &outcomes,
            )?;
            if let Err(outbox_error) = state
                .remove_outbox_record(&event_id, &route_id)
                .and_then(|()| persist(state))
            {
                let outcome = DeliveryOutcome::dead_letter_uncommitted(
                    event_id.clone(),
                    route_id.to_string(),
                    event_kind,
                    condition_id.clone(),
                    prior_attempts,
                    error_detail,
                    detail_from_core_error(
                        &outbox_error,
                        DiagnosticOperation::OutboxStateCommit,
                        None,
                    ),
                );
                outcomes.push(outcome);
                return Err(DeliveryDrainFailure {
                    drain: DeliveryDrain { outcomes },
                    error: outbox_error,
                });
            }
            let outcome = DeliveryOutcome::dead_lettered(
                event_id.clone(),
                route_id.to_string(),
                event_kind,
                condition_id.clone(),
                prior_attempts,
                error_detail,
            );
            outcomes.push(outcome);
            continue;
        }
        let attempt = process::deliver(route, record.immutable_payload());
        if attempt.is_success() {
            let observability = attempt
                .stderr()
                .capture_problem()
                .map(delivery_observability_detail);
            if let Err(outbox_error) = state
                .remove_outbox_record(&event_id, &route_id)
                .and_then(|()| persist(state))
            {
                let outcome = DeliveryOutcome::delivered_uncommitted(
                    event_id.clone(),
                    route_id.to_string(),
                    event_kind,
                    condition_id.clone(),
                    prior_attempts.saturating_add(1),
                    detail_from_core_error(
                        &outbox_error,
                        DiagnosticOperation::OutboxStateCommit,
                        None,
                    ),
                    observability,
                );
                outcomes.push(outcome);
                return Err(DeliveryDrainFailure {
                    drain: DeliveryDrain { outcomes },
                    error: outbox_error,
                });
            }
            let outcome = DeliveryOutcome::delivered(
                event_id.clone(),
                route_id.to_string(),
                event_kind,
                condition_id.clone(),
                prior_attempts.saturating_add(1),
                observability,
            );
            outcomes.push(outcome);
        } else {
            let error_detail = delivery_failure_detail_or_drain_failure(attempt, &outcomes)?;
            let attempt_count = prior_attempts.saturating_add(1);
            let retry_commit_time = match now() {
                Ok(retry_commit_time) => retry_commit_time,
                Err(outbox_error) => {
                    let outcome = DeliveryOutcome::retry_uncommitted(
                        event_id.clone(),
                        route_id.to_string(),
                        event_kind,
                        condition_id.clone(),
                        attempt_count,
                        error_detail,
                        detail_from_core_error(
                            &outbox_error,
                            DiagnosticOperation::OutboxDrain,
                            None,
                        ),
                    );
                    outcomes.push(outcome);
                    return Err(DeliveryDrainFailure {
                        drain: DeliveryDrain { outcomes },
                        error: outbox_error,
                    });
                }
            };
            let next_retry_at = match retry_at(&retry_commit_time, attempt_count, target.outbox()) {
                Ok(next_retry_at) => next_retry_at,
                Err(outbox_error) => {
                    let outcome = DeliveryOutcome::retry_uncommitted(
                        event_id.clone(),
                        route_id.to_string(),
                        event_kind,
                        condition_id.clone(),
                        attempt_count,
                        error_detail,
                        detail_from_core_error(
                            &outbox_error,
                            DiagnosticOperation::OutboxDrain,
                            None,
                        ),
                    );
                    outcomes.push(outcome);
                    return Err(DeliveryDrainFailure {
                        drain: DeliveryDrain { outcomes },
                        error: outbox_error,
                    });
                }
            };
            let attempt_count = match record_failure(
                state,
                &event_id,
                &route_id,
                error_detail.clone(),
                next_retry_at,
            ) {
                Ok(attempt_count) => attempt_count,
                Err(outbox_error) => {
                    let outcome = DeliveryOutcome::retry_uncommitted(
                        event_id.clone(),
                        route_id.to_string(),
                        event_kind,
                        condition_id.clone(),
                        attempt_count,
                        error_detail,
                        detail_from_core_error(
                            &outbox_error,
                            DiagnosticOperation::OutboxStateCommit,
                            None,
                        ),
                    );
                    outcomes.push(outcome);
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
                    let outcome = DeliveryOutcome::dead_letter_uncommitted(
                        event_id.clone(),
                        route_id.to_string(),
                        event_kind,
                        condition_id.clone(),
                        attempt_count,
                        error_detail,
                        detail_from_core_error(
                            &outbox_error,
                            DiagnosticOperation::OutboxStateCommit,
                            None,
                        ),
                    );
                    outcomes.push(outcome);
                    return Err(DeliveryDrainFailure {
                        drain: DeliveryDrain { outcomes },
                        error: outbox_error,
                    });
                }
                let outcome = DeliveryOutcome::dead_lettered(
                    event_id.clone(),
                    route_id.to_string(),
                    event_kind,
                    condition_id.clone(),
                    attempt_count,
                    error_detail,
                );
                outcomes.push(outcome);
            } else {
                if let Err(outbox_error) = persist(state) {
                    let outcome = DeliveryOutcome::retry_uncommitted(
                        event_id.clone(),
                        route_id.to_string(),
                        event_kind,
                        condition_id.clone(),
                        attempt_count,
                        error_detail,
                        detail_from_core_error(
                            &outbox_error,
                            DiagnosticOperation::OutboxStateCommit,
                            None,
                        ),
                    );
                    outcomes.push(outcome);
                    return Err(DeliveryDrainFailure {
                        drain: DeliveryDrain { outcomes },
                        error: outbox_error,
                    });
                }
                let outcome = DeliveryOutcome::retry_scheduled(
                    event_id,
                    route_id.to_string(),
                    event_kind,
                    condition_id,
                    attempt_count,
                    error_detail,
                );
                outcomes.push(outcome);
            }
        }
    }
}

/// Converts the already-validated exhausted-record invariant into a drain-scoped failure if a
/// caller ever bypasses aggregate validation before reaching the coordinator.
fn exhausted_error_detail_or_drain_failure(
    detail: Option<DiagnosticDetail>,
    outcomes: &[DeliveryOutcome],
) -> Result<DiagnosticDetail, DeliveryDrainFailure> {
    detail.ok_or_else(|| DeliveryDrainFailure {
        drain: DeliveryDrain {
            outcomes: outcomes.to_vec(),
        },
        error: CoreError::internal("validated exhausted outbox record had no last_error_detail"),
    })
}

/// Converts the total process-attempt result into its durable failure carrier while retaining
/// prior delivery evidence if an invalid attempt ever crosses the internal process boundary.
fn delivery_failure_detail_or_drain_failure(
    attempt: crate::DeliveryProcessAttempt,
    outcomes: &[DeliveryOutcome],
) -> Result<DiagnosticDetail, DeliveryDrainFailure> {
    delivery_failure_detail(attempt).map_err(|error| DeliveryDrainFailure {
        drain: DeliveryDrain {
            outcomes: outcomes.to_vec(),
        },
        error,
    })
}

pub(in crate::runtime) fn retry_at(
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
    use crate::{
        DeliveryProcessAttempt, ExactByteCount, StderrCapture, StderrOutcome, TerminalOutcome,
        WriterOutcome,
    };

    use super::{
        delivery_failure_detail_or_drain_failure, exhausted_error_detail_or_drain_failure,
    };

    #[test]
    fn defensive_drain_fact_adapters_retain_prior_evidence_when_an_internal_invariant_is_broken() {
        let exhausted = exhausted_error_detail_or_drain_failure(None, &[])
            .expect_err("exhausted pending record must retain failure evidence");
        assert!(
            exhausted
                .error
                .to_string()
                .contains("had no last_error_detail")
        );
        assert!(exhausted.drain.outcomes.is_empty());

        let successful = DeliveryProcessAttempt::new(
            TerminalOutcome::Exited { exit_code: Some(0) },
            WriterOutcome::Completed,
            StderrOutcome::captured(
                StderrCapture::from_bytes(Vec::new(), ExactByteCount::zero())
                    .expect("empty capture"),
            ),
        );
        let failure = delivery_failure_detail_or_drain_failure(successful, &[])
            .expect_err("successful process cannot become a failure diagnostic");
        assert!(
            failure
                .error
                .to_string()
                .contains("successful delivery cannot carry a failure diagnostic")
        );
        assert!(failure.drain.outcomes.is_empty());
    }
}

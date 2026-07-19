use crate::model::{PermanentTargetError, detail_from_core_error, io_detail, plain_detail};
use crate::{
    CoreError, DiagnosticKind, DiagnosticOperation, IoErrorClass, OnRunEventCause,
    PolicyEvaluation, ResetReport, RouteFamily, RunMode, RunOutcome, RunReport,
    SourceSuspectReason, StagedEventEligibility, StateDocument, TargetDocument, TargetPaths,
};

use super::super::acquire::{AcquiredMeasurement, acquire_measurement, fetch_source, now_utc};
use super::super::lock::{LockError, lock_exclusive, lock_shared};
use super::super::report::{
    DeliveryEvidence, FinishReport, detail_from_error_for_operation, finish_report,
    report_for_target_error,
};
use super::super::storage::{blind_remove_storage_root, load_state, load_target};
use super::lifecycle::*;

type ValidObservationStager =
    fn(&TargetDocument, &StateDocument, crate::Observation, &str) -> Result<StagedRun, CoreError>;

/// Validates and returns the current target document.
pub(crate) fn validate_target(paths: &TargetPaths) -> Result<TargetDocument, CoreError> {
    paths.require_watch_root_directory()?;
    let target = load_target(paths)?;
    target.validate()?;
    Ok(target)
}

/// Blindly clears target storage and, when a valid delivery configuration remains, durably stages
/// the fresh-lifecycle reset event after the delete has completed.
pub(crate) fn reset(paths: &TargetPaths) -> Result<ResetReport, CoreError> {
    paths.require_watch_root_directory()?;
    let _lock = match lock_exclusive(paths) {
        Ok(lock) => lock,
        Err(LockError::Unavailable(error)) => {
            return Err(CoreError::io(paths.run_lock_file(), error));
        }
        Err(LockError::Io(error)) => return Err(error),
    };
    let storage_cleared = blind_remove_storage_root(&paths.storage_root())?;
    let mut delivery = DeliveryEvidence::default();
    if let Ok(target) = load_target(paths)
        && target.validate_without_projection().is_ok()
        && target.permanent_error().is_ok_and(|error| error.is_none())
        && target.routes_for(RouteFamily::OnRun).next().is_some()
    {
        let digest = target.contract_digest_sha256()?;
        let state = StateDocument::new(crate::TargetId::new(target.target_id())?, digest.clone());
        let staged = StagedRun::from_eligibilities(
            state,
            vec![StagedEventEligibility::OnRun {
                cause: OnRunEventCause::Reset,
            }],
        );
        delivery = match commit_staged_run(paths, &target, RunMode::Live, &staged, &digest) {
            Ok(commit) => commit.delivery,
            Err(failure) => reset_delivery_after_failed_commit(*failure),
        };
    }
    Ok(ResetReport::new(
        paths.target_id(),
        storage_cleared,
        delivery.outcomes,
        delivery.overflow,
        delivery.outbox_error_detail,
    ))
}

/// Retains a commit failure's delivery evidence, synthesizing the one missing outbox diagnostic
/// only for failures that occurred before drain-time evidence could exist.
fn reset_delivery_after_failed_commit(failure: CommitFailure) -> DeliveryEvidence {
    let mut delivery = failure.delivery;
    if delivery.outbox_error_detail.is_none() {
        delivery.outbox_error_detail = Some(detail_from_core_error(
            &failure.error,
            DiagnosticOperation::OutboxDrain,
            None,
        ));
    }
    delivery
}

/// Executes a live target run.
pub(crate) fn run_once(paths: &TargetPaths) -> Result<RunReport, CoreError> {
    run_once_with_mode(paths, RunMode::Live)
}

/// Executes one target under the requested mode.
pub(crate) fn run_once_with_mode(
    paths: &TargetPaths,
    mode: RunMode,
) -> Result<RunReport, CoreError> {
    run_once_with_stager(paths, mode, stage_valid_observation_run)
}

/// Runs one target with the policy-staging boundary supplied explicitly.
///
/// The production coordinator always supplies [`stage_valid_observation_run`]. Keeping that
/// fallible boundary explicit makes the otherwise proof-unreachable policy-invariant result an
/// executable lifecycle contract: it must stay target-scoped, preserve the last accepted state,
/// and report structured integration-fault evidence rather than aborting a batch.
pub(crate) fn run_once_with_stager(
    paths: &TargetPaths,
    mode: RunMode,
    stager: ValidObservationStager,
) -> Result<RunReport, CoreError> {
    run_once_with_stager_and_permanent_error_resolver(
        paths,
        mode,
        stager,
        TargetDocument::permanent_error,
    )
}

/// Runs one target with both policy staging and permanent-projection preflight explicit.
///
/// Production uses [`TargetDocument::permanent_error`]. Keeping the fallible external-contract
/// boundary explicit lets its target-scoped report behavior remain directly executable.
pub(in crate::runtime) fn run_once_with_stager_and_permanent_error_resolver(
    paths: &TargetPaths,
    mode: RunMode,
    stager: ValidObservationStager,
    permanent_error_resolver: fn(
        &TargetDocument,
    ) -> Result<Option<PermanentTargetError>, CoreError>,
) -> Result<RunReport, CoreError> {
    paths.require_watch_root_directory()?;
    let started = now_utc()?;
    let target = match load_target(paths) {
        Ok(target) => target,
        Err(error) => {
            return report_for_target_error(
                paths,
                mode,
                started,
                error,
                DiagnosticOperation::TargetLoad,
            );
        }
    };
    if let Err(error) = target.validate_without_projection() {
        return report_for_target_error(
            paths,
            mode,
            started,
            error,
            DiagnosticOperation::TargetValidation,
        );
    }
    let digest = target.contract_digest_sha256()?;
    let lock = match mode {
        RunMode::Live => match lock_exclusive(paths) {
            Ok(lock) => lock,
            Err(LockError::Unavailable(error)) => {
                return finish_report(FinishReport {
                    target: &target,
                    mode,
                    outcome: RunOutcome::LockUnavailable,
                    started,
                    digest: Some(digest),
                    observation: None,
                    previous: None,
                    error: Some(io_detail(
                        IoErrorClass::from_error(&error),
                        DiagnosticOperation::LockAcquire,
                        "target lock is held by another live run",
                        Some(paths.run_lock_file().display().to_string()),
                    )),
                    policy_evaluation: PolicyEvaluation::not_evaluated(),
                    lifecycle_before: None,
                    lifecycle_after: None,
                    persisted: false,
                    delivery: DeliveryEvidence::default(),
                });
            }
            Err(LockError::Io(error)) => return Err(error),
        },
        RunMode::DryRun => lock_shared(paths)?,
    };
    let _lock = lock;
    let prior = match load_state(paths) {
        Ok(prior) => prior,
        Err(error) => {
            return finish_report(FinishReport {
                target: &target,
                mode,
                outcome: RunOutcome::StateInvalid,
                started,
                digest: Some(digest),
                observation: None,
                previous: None,
                error: Some(detail_from_error_for_operation(
                    &error,
                    DiagnosticOperation::StateLoad,
                    Some(paths.state_file()),
                )),
                policy_evaluation: PolicyEvaluation::not_evaluated(),
                lifecycle_before: None,
                lifecycle_after: None,
                persisted: false,
                delivery: DeliveryEvidence::default(),
            });
        }
    };
    if let Some(state) = &prior
        && state.contract_digest_sha256() != digest
    {
        return finish_report(FinishReport {
            target: &target,
            mode,
            outcome: RunOutcome::RefusedContractDigest,
            started,
            digest: Some(digest),
            observation: None,
            previous: previous_value(state),
            error: Some(plain_detail(
                DiagnosticKind::Contract,
                DiagnosticOperation::StateLoad,
                "stored state belongs to a different measurement contract; run `ffhn reset --target <ID>` before running this target",
                Some(paths.state_file().display().to_string()),
            )),
            policy_evaluation: PolicyEvaluation::not_evaluated(),
            lifecycle_before: None,
            lifecycle_after: None,
            persisted: false,
            delivery: DeliveryEvidence::default(),
        });
    }
    let (state, lifecycle_before) = match prior {
        Some(state) => {
            if let Err(error) = state.validate_for_target(&target) {
                return finish_report(FinishReport {
                    target: &target,
                    mode,
                    outcome: RunOutcome::StateInvalid,
                    started,
                    digest: Some(digest),
                    observation: None,
                    previous: previous_value(&state),
                    error: Some(detail_from_error_for_operation(
                        &error,
                        DiagnosticOperation::StateLoad,
                        Some(paths.state_file()),
                    )),
                    policy_evaluation: PolicyEvaluation::not_evaluated(),
                    lifecycle_before: None,
                    lifecycle_after: None,
                    persisted: false,
                    delivery: DeliveryEvidence::default(),
                });
            }
            let lifecycle_before = Some(state.lifecycle_snapshot()?);
            (state, lifecycle_before)
        }
        None => (
            StateDocument::new(crate::TargetId::new(target.target_id())?, digest.clone()),
            None,
        ),
    };
    let previous = previous_value(&state);
    if !target.enabled() {
        return finish_report(FinishReport {
            target: &target,
            mode,
            outcome: RunOutcome::SkippedDisabled,
            started,
            digest: Some(digest),
            observation: None,
            previous,
            error: None,
            policy_evaluation: PolicyEvaluation::not_evaluated(),
            lifecycle_before,
            lifecycle_after: None,
            persisted: false,
            delivery: DeliveryEvidence::default(),
        });
    }
    let permanent_error = match permanent_error_resolver(&target) {
        Ok(error) => error,
        Err(error) => {
            return finish_report(FinishReport {
                target: &target,
                mode,
                outcome: RunOutcome::ConfigInvalid,
                started,
                digest: Some(digest),
                observation: None,
                previous,
                error: Some(detail_from_error_for_operation(
                    &error,
                    DiagnosticOperation::TargetValidation,
                    Some(paths.target_file()),
                )),
                policy_evaluation: PolicyEvaluation::not_evaluated(),
                lifecycle_before,
                lifecycle_after: None,
                persisted: false,
                delivery: DeliveryEvidence::default(),
            });
        }
    };
    if let Some(error) = permanent_error {
        let error_code = error.code();
        return finish_permanent_error(
            StatefulRun {
                paths,
                target: &target,
                mode,
                started,
                digest,
                previous,
                lifecycle_before: lifecycle_before.clone(),
            },
            &state,
            error_code,
            error.into_diagnostic_detail(Some(paths.target_file().display().to_string())),
        );
    }
    let source = match fetch_source(&target) {
        Ok(source) => source,
        Err(detail) => {
            return finish_source_suspect(
                StatefulRun {
                    paths,
                    target: &target,
                    mode,
                    started,
                    digest,
                    previous,
                    lifecycle_before: lifecycle_before.clone(),
                },
                &state,
                RunOutcome::FetchFailed,
                SourceSuspectReason::FetchFailed,
                detail,
            );
        }
    };
    let acquired =
        match acquire_measurement(&target, &source.body, source.effective_http_url.as_ref()) {
            Ok(acquired) => acquired,
            Err(failure) => {
                return finish_measurement_acquisition_failure(
                    StatefulRun {
                        paths,
                        target: &target,
                        mode,
                        started,
                        digest,
                        previous,
                        lifecycle_before: lifecycle_before.clone(),
                    },
                    &state,
                    failure,
                );
            }
        };
    let observation = match acquired {
        AcquiredMeasurement::JsonScalar(raw) => target.parse_json_scalar_token(raw),
        AcquiredMeasurement::Html(input) => target.parse_html_projection(input),
    };
    let observation = match observation {
        Ok(observation) => observation,
        Err(detail) => {
            return finish_source_suspect(
                StatefulRun {
                    paths,
                    target: &target,
                    mode,
                    started,
                    digest,
                    previous,
                    lifecycle_before: lifecycle_before.clone(),
                },
                &state,
                RunOutcome::ValueUnparseable,
                SourceSuspectReason::ValueUnparseable,
                detail,
            );
        }
    };
    let outcome = match previous.as_deref() {
        None => RunOutcome::Initialized,
        Some(previous) if previous == observation.canonical_value() => RunOutcome::Unchanged,
        Some(_) => RunOutcome::Changed,
    };
    let staged_run = match stager(&target, &state, observation.clone(), &started) {
        Ok(staged_run) => staged_run,
        Err(CoreError::PolicyInvariant(message)) => {
            return finish_policy_invariant(
                StatefulRun {
                    paths,
                    target: &target,
                    mode,
                    started,
                    digest,
                    previous,
                    lifecycle_before: lifecycle_before.clone(),
                },
                &state,
                observation,
                message,
            );
        }
        Err(error) => return Err(error),
    };
    let policy_evaluation = staged_run.policy_evaluation();
    let lifecycle_after = staged_run.next_state().lifecycle_snapshot()?;
    let commit = match commit_staged_run(paths, &target, mode, &staged_run, &digest) {
        Ok(commit) => commit,
        Err(failure) => {
            return finish_report(FinishReport {
                target: &target,
                mode,
                outcome: RunOutcome::PersistFailed,
                started,
                digest: Some(digest),
                observation: Some(observation),
                previous,
                error: Some(detail_from_error_for_operation(
                    &failure.error,
                    DiagnosticOperation::StateCommit,
                    Some(paths.state_file()),
                )),
                policy_evaluation: policy_evaluation.clone(),
                lifecycle_before: lifecycle_before.clone(),
                lifecycle_after: Some(lifecycle_after.clone()),
                persisted: failure.persisted,
                delivery: failure.delivery,
            });
        }
    };
    finish_report(FinishReport {
        target: &target,
        mode,
        outcome,
        started,
        digest: Some(digest),
        observation: Some(observation),
        previous,
        error: None,
        policy_evaluation,
        lifecycle_before,
        lifecycle_after: Some(lifecycle_after),
        persisted: commit.persisted,
        delivery: commit.delivery,
    })
}

#[cfg(test)]
mod tests {
    use crate::model::io_detail;
    use crate::{CoreError, DiagnosticOperation, IoErrorClass};

    use super::{CommitFailure, DeliveryEvidence, reset_delivery_after_failed_commit};

    #[test]
    fn reset_commit_failure_preserves_existing_evidence_or_synthesizes_one_precise_fallback() {
        let fallback = reset_delivery_after_failed_commit(CommitFailure {
            error: CoreError::internal("state commit stopped"),
            persisted: false,
            delivery: DeliveryEvidence::default(),
        });
        assert_eq!(
            fallback
                .outbox_error_detail
                .as_ref()
                .map(|detail| detail.operation()),
            Some(DiagnosticOperation::OutboxDrain)
        );

        let retained_detail = io_detail(
            IoErrorClass::StorageFull,
            DiagnosticOperation::OutboxStateCommit,
            "state update stopped",
            None,
        );
        let retained = reset_delivery_after_failed_commit(CommitFailure {
            error: CoreError::internal("later failure"),
            persisted: true,
            delivery: DeliveryEvidence {
                outcomes: Vec::new(),
                overflow: Vec::new(),
                outbox_error_detail: Some(retained_detail),
            },
        });
        assert_eq!(
            retained
                .outbox_error_detail
                .as_ref()
                .map(|detail| detail.operation()),
            Some(DiagnosticOperation::OutboxStateCommit)
        );
    }
}

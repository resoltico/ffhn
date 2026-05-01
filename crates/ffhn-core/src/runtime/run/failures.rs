use std::time::Instant;

use crate::time::elapsed_ms;
use crate::{
    CoreError, FailureClass, PersistWriteStatus, RUN_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_VERSION,
    ReasonCode, RunMode, RunOutcome, RunPersistSection, RunReport, TargetDocument, TargetPaths,
    TargetStatus,
};

use super::super::state::{
    StateLoad, prior_compare_digest, state_error_detail, state_phase_or_default,
    status_from_loaded_state, status_from_state,
};
use super::execute::RunOptions;
use super::reporting::{
    PersistFailureContext, finish_persist_failure_report, finish_report, persist_disabled_state,
    persist_write_status,
};

pub(super) fn live_state_failure_reason(
    options: RunOptions,
    state: &StateLoad,
) -> Option<ReasonCode> {
    if options.mode != RunMode::Live {
        return None;
    }

    match state {
        StateLoad::Unreadable(_) | StateLoad::InvalidDocument { .. } => {
            Some(ReasonCode::StateInvalid)
        }
        StateLoad::IntegrityMismatch { .. } => Some(ReasonCode::IntegrityMismatch),
        StateLoad::Missing | StateLoad::Valid(_) => None,
    }
}

pub(super) fn finish_lock_unavailable_report(
    paths: &TargetPaths,
    target: &TargetDocument,
    run_started_at: &str,
    options: RunOptions,
) -> Result<RunReport, CoreError> {
    let state = StateLoad::Missing;
    finish_report(
        paths,
        Some(target),
        RunReport {
            schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: RUN_REPORT_SCHEMA_VERSION,
            run_report_digest_sha256: String::new(),
            target_id: target.target_id.clone(),
            run_started_at: run_started_at.to_owned(),
            run_finished_at: String::new(),
            run_mode: options.mode,
            run_outcome: RunOutcome::FailedTransient,
            reason_code: ReasonCode::LockUnavailable,
            failure_class: Some(FailureClass::Transient),
            error_detail: Some(
                crate::ProcessErrorDetail::new(
                    crate::ProcessErrorKind::Io,
                    "another FFHN run already holds the exclusive target lock",
                    Some(paths.run_lock_file().display().to_string()),
                )
                .expect("lock-unavailable detail"),
            ),
            target_status_after_run: status_from_state(&state),
            compare_basis: target.compare.basis,
            previous_compare_digest_sha256: prior_compare_digest(&state),
            current_compare_digest_sha256: None,
            state_phase_before_run: state_phase_or_default(&state),
            state_phase_after_run: state_phase_or_default(&state),
            fetch: None,
            extraction: None,
            compare: None,
            change: None,
            persist: RunPersistSection::from_writes(
                0,
                PersistWriteStatus::NotAttempted,
                PersistWriteStatus::NotAttempted,
            ),
            notifications: Vec::new(),
            extensions: None,
        },
    )
}

pub(super) fn finish_live_state_failure_report(
    paths: &TargetPaths,
    target: &TargetDocument,
    state: &StateLoad,
    run_started_at: &str,
    options: RunOptions,
    reason_code: ReasonCode,
) -> Result<RunReport, CoreError> {
    finish_report(
        paths,
        Some(target),
        RunReport {
            schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: RUN_REPORT_SCHEMA_VERSION,
            run_report_digest_sha256: String::new(),
            target_id: target.target_id.clone(),
            run_started_at: run_started_at.to_owned(),
            run_finished_at: String::new(),
            run_mode: options.mode,
            run_outcome: RunOutcome::FailedPermanent,
            reason_code,
            failure_class: Some(FailureClass::Permanent),
            error_detail: state_error_detail(state).cloned(),
            target_status_after_run: TargetStatus::Invalid,
            compare_basis: target.compare.basis,
            previous_compare_digest_sha256: prior_compare_digest(state),
            current_compare_digest_sha256: None,
            state_phase_before_run: state_phase_or_default(state),
            state_phase_after_run: state_phase_or_default(state),
            fetch: None,
            extraction: None,
            compare: None,
            change: None,
            persist: RunPersistSection::from_writes(
                0,
                PersistWriteStatus::NotAttempted,
                PersistWriteStatus::NotAttempted,
            ),
            notifications: Vec::new(),
            extensions: None,
        },
    )
}

pub(super) fn finish_disabled_target_report(
    paths: &TargetPaths,
    target: &TargetDocument,
    state: &StateLoad,
    run_started_at: &str,
    options: RunOptions,
) -> Result<RunReport, CoreError> {
    let persist_started = Instant::now();
    let (wrote_state, state_after_run) =
        match persist_disabled_state(paths, target, state, run_started_at) {
            Ok(result) => result,
            Err(error) => {
                return finish_persist_failure_report(
                    paths,
                    target,
                    state,
                    run_started_at,
                    PersistFailureContext {
                        run_mode: options.mode,
                        fetch: None,
                        extraction: None,
                        compare: None,
                        change: None,
                        persist_duration_ms: elapsed_ms(&persist_started),
                        error: crate::ProcessErrorDetail::from(&error),
                    },
                );
            }
        };
    finish_report(
        paths,
        Some(target),
        RunReport {
            schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: RUN_REPORT_SCHEMA_VERSION,
            run_report_digest_sha256: String::new(),
            target_id: target.target_id.clone(),
            run_started_at: run_started_at.to_owned(),
            run_finished_at: String::new(),
            run_mode: options.mode,
            run_outcome: RunOutcome::SkippedDisabled,
            reason_code: ReasonCode::Disabled,
            failure_class: None,
            error_detail: None,
            target_status_after_run: status_from_loaded_state(state_after_run.as_ref()),
            compare_basis: target.compare.basis,
            previous_compare_digest_sha256: prior_compare_digest(state),
            current_compare_digest_sha256: None,
            state_phase_before_run: state_phase_or_default(state),
            state_phase_after_run: state_after_run
                .as_ref()
                .map(|state| state.state_phase)
                .unwrap_or_else(|| state_phase_or_default(state)),
            fetch: None,
            extraction: None,
            compare: None,
            change: None,
            persist: RunPersistSection::from_writes(
                elapsed_ms(&persist_started),
                persist_write_status(wrote_state),
                PersistWriteStatus::NotAttempted,
            ),
            notifications: Vec::new(),
            extensions: None,
        },
    )
}

use std::time::Instant;

use crate::time::elapsed_ms;
use crate::{
    CoreError, PersistWriteStatus, RunFailureCause, RunMode, RunPersistSection, RunReport,
    RunResult, TargetDocument, TargetPaths,
};

use super::super::state::{StateLoad, state_error_detail};
use super::execute::RunOptions;
use super::report_builder::{
    RunReportDraft, RunReportLifecycle, RunReportSections, build_run_report,
};
use super::reporting::{
    PersistFailureContext, finish_persist_failure_report, finish_report, persist_disabled_state,
    persist_write_status,
};

pub(super) fn live_state_failure_reason(
    options: RunOptions,
    state: &StateLoad,
) -> Option<RunFailureCause> {
    if options.mode != RunMode::Live {
        return None;
    }

    match state {
        StateLoad::Unreadable(_) | StateLoad::InvalidDocument { .. } => {
            Some(RunFailureCause::StateInvalid)
        }
        StateLoad::IntegrityMismatch { .. } => Some(RunFailureCause::IntegrityMismatch),
        StateLoad::Missing | StateLoad::Valid(_) => None,
    }
}

pub(super) fn finish_lock_unavailable_report(
    paths: &TargetPaths,
    target: &TargetDocument,
    state: &StateLoad,
    run_started_at: &str,
    options: RunOptions,
) -> Result<RunReport, CoreError> {
    finish_report(
        paths,
        Some(target),
        build_run_report(RunReportDraft {
            target_id: target.target_id.clone(),
            display_name: Some(target.display_name.clone()),
            run_started_at: run_started_at.to_owned(),
            run_mode: options.mode,
            result: RunResult::FailedTransient {
                cause: RunFailureCause::LockUnavailable,
                error_detail: crate::ProcessErrorDetail::new(
                    crate::ProcessErrorKind::Io,
                    "another FFHN run already holds the exclusive target lock",
                    Some(paths.run_lock_file().display().to_string()),
                )
                .expect("lock-unavailable detail"),
            },
            compare_basis: target.compare.basis,
            lifecycle: RunReportLifecycle::from_state_snapshot(state, None),
            sections: RunReportSections::default(),
            persist: RunPersistSection::from_writes(
                0,
                PersistWriteStatus::NotAttempted,
                0,
                PersistWriteStatus::NotAttempted,
            ),
        }),
    )
}

pub(super) fn finish_live_state_failure_report(
    paths: &TargetPaths,
    target: &TargetDocument,
    state: &StateLoad,
    run_started_at: &str,
    options: RunOptions,
    failure_cause: RunFailureCause,
) -> Result<RunReport, CoreError> {
    finish_report(
        paths,
        Some(target),
        build_run_report(RunReportDraft {
            target_id: target.target_id.clone(),
            display_name: Some(target.display_name.clone()),
            run_started_at: run_started_at.to_owned(),
            run_mode: options.mode,
            result: RunResult::FailedPermanent {
                cause: failure_cause,
                error_detail: state_error_detail(state)
                    .cloned()
                    .expect("state failure reports carry detail"),
            },
            compare_basis: target.compare.basis,
            lifecycle: RunReportLifecycle::from_state_snapshot(state, None),
            sections: RunReportSections::default(),
            persist: RunPersistSection::from_writes(
                0,
                PersistWriteStatus::NotAttempted,
                0,
                PersistWriteStatus::NotAttempted,
            ),
        }),
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
    let (wrote_state, state_after_run) = if options.mode == RunMode::Live {
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
                        current_compare_digest_sha256: None,
                        fetch: None,
                        extraction: None,
                        compare: None,
                        change: None,
                        state_commit_duration_ms: elapsed_ms(&persist_started),
                        error: crate::ProcessErrorDetail::from(&error),
                    },
                );
            }
        }
    } else {
        (false, None)
    };
    finish_report(
        paths,
        Some(target),
        build_run_report(RunReportDraft {
            target_id: target.target_id.clone(),
            display_name: Some(target.display_name.clone()),
            run_started_at: run_started_at.to_owned(),
            run_mode: options.mode,
            result: RunResult::SkippedDisabled,
            compare_basis: target.compare.basis,
            lifecycle: if options.mode == RunMode::Live {
                RunReportLifecycle::from_live_state_transition(
                    state,
                    state_after_run.as_ref(),
                    None,
                )
            } else {
                RunReportLifecycle::from_state_snapshot(state, None)
            },
            sections: RunReportSections::default(),
            persist: RunPersistSection::from_writes(
                if options.mode == RunMode::Live {
                    elapsed_ms(&persist_started)
                } else {
                    0
                },
                if options.mode == RunMode::Live {
                    persist_write_status(wrote_state)
                } else {
                    PersistWriteStatus::NotAttempted
                },
                0,
                PersistWriteStatus::NotAttempted,
            ),
        }),
    )
}

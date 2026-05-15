use std::time::Instant;

use crate::time::elapsed_ms;
use crate::{
    CompareBasis, CoreError, PersistWriteStatus, ProcessErrorDetail, RunChangeSection,
    RunCompareSection, RunExtractionSection, RunFailureCause, RunFetchSection, RunMode,
    RunPersistSection, RunReport, TargetId, TargetPaths,
};

use super::super::persist::{persist_state_only, write_last_run};
use super::super::state::StateLoad;
use super::super::storage::now_utc;
use super::notifications::dispatch_notifications;
use super::outcome::failure_run_outcome;
use super::report_builder::{
    RunReportDraft, RunReportLifecycle, RunReportSections, build_run_report, failure_result,
};

pub(super) const fn persist_write_status(wrote: bool) -> PersistWriteStatus {
    if wrote {
        PersistWriteStatus::Written
    } else {
        PersistWriteStatus::NotAttempted
    }
}

#[derive(Clone, Debug)]
pub(super) struct PersistFailureContext {
    pub(super) run_mode: RunMode,
    pub(super) current_compare_digest_sha256: Option<String>,
    pub(super) fetch: Option<RunFetchSection>,
    pub(super) extraction: Option<RunExtractionSection>,
    pub(super) compare: Option<RunCompareSection>,
    pub(super) change: Option<RunChangeSection>,
    pub(super) state_commit_duration_ms: u64,
    pub(super) error: ProcessErrorDetail,
}

#[derive(Clone, Debug)]
pub(super) struct FailedRunContext {
    pub(super) run_mode: RunMode,
    pub(super) failure_cause: RunFailureCause,
    pub(super) error_detail: ProcessErrorDetail,
    pub(super) fetch: Option<RunFetchSection>,
}

pub(super) fn invalid_target_run_report(
    paths: &TargetPaths,
    run_started_at: &str,
    run_mode: RunMode,
    error_detail: ProcessErrorDetail,
) -> Result<RunReport, CoreError> {
    Ok(build_run_report(RunReportDraft {
        target_id: TargetId::new(paths.target_id()).expect("validated path target id"),
        display_name: None,
        run_started_at: run_started_at.to_owned(),
        run_mode,
        result: failure_result(RunFailureCause::ConfigInvalid, error_detail),
        compare_basis: CompareBasis::CanonicalTextSha256,
        lifecycle: RunReportLifecycle::invalid_target(),
        sections: RunReportSections::default(),
        persist: RunPersistSection::from_writes(
            0,
            PersistWriteStatus::NotAttempted,
            0,
            PersistWriteStatus::NotAttempted,
        ),
    }))
}

pub(super) fn unavailable_target_run_report(
    paths: &TargetPaths,
    run_started_at: &str,
    run_mode: RunMode,
    error_detail: ProcessErrorDetail,
) -> Result<RunReport, CoreError> {
    Ok(build_run_report(RunReportDraft {
        target_id: TargetId::new(paths.target_id()).expect("validated path target id"),
        display_name: None,
        run_started_at: run_started_at.to_owned(),
        run_mode,
        result: failure_result(RunFailureCause::TargetUnavailable, error_detail),
        compare_basis: CompareBasis::CanonicalTextSha256,
        lifecycle: RunReportLifecycle::invalid_target(),
        sections: RunReportSections::default(),
        persist: RunPersistSection::from_writes(
            0,
            PersistWriteStatus::NotAttempted,
            0,
            PersistWriteStatus::NotAttempted,
        ),
    }))
}

pub(super) fn finalize_run_report(mut report: RunReport) -> Result<RunReport, CoreError> {
    report.run_finished_at = now_utc()?;
    seal_run_report(report)
}

pub(super) fn seal_run_report(report: RunReport) -> Result<RunReport, CoreError> {
    let report = report.with_digest()?;
    report.validate()?;
    Ok(report)
}

pub(super) fn finalize_failed_run(
    paths: &TargetPaths,
    target: &crate::TargetDocument,
    state: &StateLoad,
    run_started_at: &str,
    context: FailedRunContext,
) -> Result<RunReport, CoreError> {
    let persist_started = Instant::now();
    let (wrote_state, state_after_run) = if context.run_mode == RunMode::Live {
        match persist_failed_state(paths, target, state, context.failure_cause, run_started_at) {
            Ok(result) => result,
            Err(error) => {
                return finish_persist_failure_report(
                    paths,
                    target,
                    state,
                    run_started_at,
                    PersistFailureContext {
                        run_mode: context.run_mode,
                        current_compare_digest_sha256: None,
                        fetch: context.fetch.clone(),
                        extraction: None,
                        compare: None,
                        change: None,
                        state_commit_duration_ms: elapsed_ms(&persist_started),
                        error: ProcessErrorDetail::from(&error),
                    },
                );
            }
        }
    } else {
        (false, None)
    };
    let FailedRunContext {
        run_mode,
        failure_cause,
        error_detail,
        fetch,
    } = context;
    finish_report(
        paths,
        Some(target),
        build_run_report(RunReportDraft {
            target_id: target.target_id.clone(),
            display_name: Some(target.display_name.clone()),
            run_started_at: run_started_at.to_owned(),
            run_mode,
            result: failure_result(failure_cause, error_detail),
            compare_basis: target.compare.basis,
            lifecycle: if run_mode == RunMode::Live {
                RunReportLifecycle::from_live_state_transition(
                    state,
                    state_after_run.as_ref(),
                    None,
                )
            } else {
                RunReportLifecycle::from_state_snapshot(state, None)
            },
            sections: RunReportSections {
                fetch,
                extraction: None,
                compare: None,
                change: None,
            },
            persist: RunPersistSection::from_writes(
                elapsed_ms(&persist_started),
                if run_mode == RunMode::Live {
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

pub(super) fn finish_persist_failure_report(
    paths: &TargetPaths,
    target: &crate::TargetDocument,
    state: &StateLoad,
    run_started_at: &str,
    context: PersistFailureContext,
) -> Result<RunReport, CoreError> {
    finish_report(
        paths,
        Some(target),
        build_run_report(RunReportDraft {
            target_id: target.target_id.clone(),
            display_name: Some(target.display_name.clone()),
            run_started_at: run_started_at.to_owned(),
            run_mode: context.run_mode,
            result: failure_result(RunFailureCause::PersistError, context.error.clone()),
            compare_basis: target.compare.basis,
            lifecycle: RunReportLifecycle::from_state_snapshot(
                state,
                context.current_compare_digest_sha256,
            ),
            sections: RunReportSections {
                fetch: context.fetch,
                extraction: context.extraction,
                compare: context.compare,
                change: context.change,
            },
            persist: RunPersistSection::from_writes(
                context.state_commit_duration_ms,
                PersistWriteStatus::Failed {
                    error: context.error.clone(),
                },
                0,
                PersistWriteStatus::NotAttempted,
            ),
        }),
    )
}

pub(super) fn persist_disabled_state(
    paths: &TargetPaths,
    target: &crate::TargetDocument,
    state: &StateLoad,
    run_started_at: &str,
) -> Result<(bool, Option<super::super::domain::PersistedState>), CoreError> {
    persist_run_state(
        paths,
        target,
        state,
        crate::RunOutcome::SkippedDisabled,
        None,
        run_started_at,
    )
}

pub(super) fn persist_failed_state(
    paths: &TargetPaths,
    target: &crate::TargetDocument,
    state: &StateLoad,
    failure_cause: RunFailureCause,
    run_started_at: &str,
) -> Result<(bool, Option<super::super::domain::PersistedState>), CoreError> {
    persist_run_state(
        paths,
        target,
        state,
        failure_run_outcome(failure_cause),
        Some(failure_cause),
        run_started_at,
    )
}

fn persist_run_state(
    paths: &TargetPaths,
    target: &crate::TargetDocument,
    state: &StateLoad,
    run_outcome: crate::RunOutcome,
    failure_cause: Option<RunFailureCause>,
    run_started_at: &str,
) -> Result<(bool, Option<super::super::domain::PersistedState>), CoreError> {
    persist_state_only(
        paths,
        target,
        state,
        run_outcome,
        failure_cause,
        run_started_at,
    )
}

pub(super) fn finish_report(
    paths: &TargetPaths,
    target: Option<&crate::TargetDocument>,
    report: RunReport,
) -> Result<RunReport, CoreError> {
    let mut report = finalize_run_report(report)?;
    if report.run_mode == RunMode::DryRun {
        return Ok(report);
    }

    if let Some(target) = target {
        report.notifications = dispatch_notifications(target, &report);
        report = finalize_run_report(report)?;
        let last_run_started = Instant::now();
        match write_last_run(paths, &report) {
            Ok(()) => {
                report.persist.last_run_write = PersistWriteStatus::Written;
                report.persist.last_run_write_duration_ms = elapsed_ms(&last_run_started);
                return seal_run_report(report);
            }
            Err(error) => {
                let error_detail = ProcessErrorDetail::from(&error);
                report.persist.last_run_write = PersistWriteStatus::Failed {
                    error: error_detail,
                };
                report.persist.last_run_write_duration_ms = elapsed_ms(&last_run_started);
                return seal_run_report(report);
            }
        }
    }

    Ok(report)
}

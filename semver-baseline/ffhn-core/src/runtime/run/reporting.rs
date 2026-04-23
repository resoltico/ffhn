use std::time::Instant;

use crate::{
    CompareBasis, CoreError, FailureClass, ProcessErrorDetail, RUN_REPORT_SCHEMA_NAME,
    RUN_REPORT_SCHEMA_VERSION, RunChangeSection, RunCompareSection, RunExtractionSection,
    RunFetchSection, RunMode, RunOutcome, RunPersistSection, RunReport, StateDocument, TargetPaths,
    TargetStatus,
};

use super::super::persist::{persist_state_only, write_last_run};
use super::super::state::{
    StateLoad, prior_compare_digest, state_phase_or_default, status_from_loaded_state,
    status_from_state,
};
use super::super::storage::now_utc;
use super::execute::RunOptions;
use super::notifications::dispatch_notifications;
use super::outcome::failure_run_outcome;

#[derive(Clone, Debug)]
pub(super) struct PersistFailureContext {
    pub(super) run_mode: RunMode,
    pub(super) fetch: Option<RunFetchSection>,
    pub(super) extraction: Option<RunExtractionSection>,
    pub(super) compare: Option<RunCompareSection>,
    pub(super) change: Option<RunChangeSection>,
    pub(super) persist_duration_ms: u64,
    pub(super) error: ProcessErrorDetail,
}

pub(super) fn invalid_target_run_report(
    paths: &TargetPaths,
    run_started_at: &str,
    run_mode: RunMode,
    started: Instant,
) -> Result<RunReport, CoreError> {
    let state = StateLoad::Missing;
    Ok(RunReport {
        schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: RUN_REPORT_SCHEMA_VERSION,
        run_report_digest_sha256: String::new(),
        target_id: paths.target_id().to_owned(),
        run_started_at: run_started_at.to_owned(),
        run_finished_at: String::new(),
        run_mode,
        run_outcome: RunOutcome::FailedPermanent,
        reason_code: crate::ReasonCode::ConfigInvalid,
        failure_class: Some(FailureClass::Permanent),
        target_status_after_run: TargetStatus::Invalid,
        compare_basis: CompareBasis::CanonicalTextSha256,
        previous_compare_digest_sha256: prior_compare_digest(&state),
        current_compare_digest_sha256: None,
        state_phase_before_run: state_phase_or_default(&state),
        state_phase_after_run: state_phase_or_default(&state),
        fetch: None,
        extraction: None,
        compare: None,
        change: None,
        persist: RunPersistSection {
            duration_ms: started.elapsed().as_millis() as u64,
            wrote_state: false,
            wrote_last_run: false,
            error: None,
        },
        notifications: Vec::new(),
        extensions: None,
    })
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
    reason_code: crate::ReasonCode,
    fetch: Option<RunFetchSection>,
    options: RunOptions,
) -> Result<RunReport, CoreError> {
    let persist_started = Instant::now();
    let (wrote_state, state_after_run) = if options.mode == RunMode::Live {
        match persist_failed_state(paths, target, state, reason_code, run_started_at) {
            Ok(result) => result,
            Err(error) => {
                return finish_persist_failure_report(
                    paths,
                    target,
                    state,
                    run_started_at,
                    PersistFailureContext {
                        run_mode: options.mode,
                        fetch: fetch.clone(),
                        extraction: None,
                        compare: None,
                        change: None,
                        persist_duration_ms: persist_started.elapsed().as_millis() as u64,
                        error: ProcessErrorDetail::from(&error),
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
        RunReport {
            schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: RUN_REPORT_SCHEMA_VERSION,
            run_report_digest_sha256: String::new(),
            target_id: target.target_id.clone(),
            run_started_at: run_started_at.to_owned(),
            run_finished_at: String::new(),
            run_mode: options.mode,
            run_outcome: failure_run_outcome(reason_code),
            reason_code,
            failure_class: reason_code.failure_class(),
            target_status_after_run: if options.mode == RunMode::Live {
                status_from_loaded_state(state_after_run.as_ref())
            } else {
                status_from_state(state)
            },
            compare_basis: target.compare.basis,
            previous_compare_digest_sha256: prior_compare_digest(state),
            current_compare_digest_sha256: None,
            state_phase_before_run: state_phase_or_default(state),
            state_phase_after_run: state_after_run
                .as_ref()
                .map(|state| state.state_phase)
                .unwrap_or_else(|| state_phase_or_default(state)),
            fetch,
            extraction: None,
            compare: None,
            change: None,
            persist: RunPersistSection {
                duration_ms: persist_started.elapsed().as_millis() as u64,
                wrote_state,
                wrote_last_run: false,
                error: None,
            },
            notifications: Vec::new(),
            extensions: None,
        },
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
        RunReport {
            schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: RUN_REPORT_SCHEMA_VERSION,
            run_report_digest_sha256: String::new(),
            target_id: target.target_id.clone(),
            run_started_at: run_started_at.to_owned(),
            run_finished_at: String::new(),
            run_mode: context.run_mode,
            run_outcome: RunOutcome::FailedTransient,
            reason_code: crate::ReasonCode::PersistError,
            failure_class: Some(FailureClass::Transient),
            target_status_after_run: status_from_state(state),
            compare_basis: target.compare.basis,
            previous_compare_digest_sha256: prior_compare_digest(state),
            current_compare_digest_sha256: None,
            state_phase_before_run: state_phase_or_default(state),
            state_phase_after_run: state_phase_or_default(state),
            fetch: context.fetch,
            extraction: context.extraction,
            compare: context.compare,
            change: context.change,
            persist: RunPersistSection {
                duration_ms: context.persist_duration_ms,
                wrote_state: false,
                wrote_last_run: false,
                error: Some(context.error),
            },
            notifications: Vec::new(),
            extensions: None,
        },
    )
}

pub(super) fn persist_disabled_state(
    paths: &TargetPaths,
    target: &crate::TargetDocument,
    state: &StateLoad,
    run_started_at: &str,
) -> Result<(bool, Option<StateDocument>), CoreError> {
    persist_run_state(
        paths,
        target,
        state,
        RunOutcome::SkippedDisabled,
        crate::ReasonCode::Disabled,
        run_started_at,
    )
}

pub(super) fn persist_failed_state(
    paths: &TargetPaths,
    target: &crate::TargetDocument,
    state: &StateLoad,
    reason_code: crate::ReasonCode,
    run_started_at: &str,
) -> Result<(bool, Option<StateDocument>), CoreError> {
    persist_run_state(
        paths,
        target,
        state,
        failure_run_outcome(reason_code),
        reason_code,
        run_started_at,
    )
}

fn persist_run_state(
    paths: &TargetPaths,
    target: &crate::TargetDocument,
    state: &StateLoad,
    run_outcome: RunOutcome,
    reason_code: crate::ReasonCode,
    run_started_at: &str,
) -> Result<(bool, Option<StateDocument>), CoreError> {
    persist_state_only(
        paths,
        target,
        state,
        run_outcome,
        reason_code,
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
        let mut persisted_report = report.clone();
        persisted_report.persist.wrote_last_run = true;
        persisted_report = seal_run_report(persisted_report)?;
        match write_last_run(paths, &persisted_report) {
            Ok(()) => return Ok(persisted_report),
            Err(error) => {
                report.persist.error = Some(ProcessErrorDetail::from(&error));
                return seal_run_report(report);
            }
        }
    }

    Ok(report)
}

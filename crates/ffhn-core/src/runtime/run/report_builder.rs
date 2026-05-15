use crate::{
    BaselinePhase, CompareBasis, CoreError, ProcessErrorDetail, RUN_REPORT_SCHEMA_NAME,
    RUN_REPORT_SCHEMA_VERSION, RunChangeSection, RunCompareSection, RunExtractionSection,
    RunFailureCause, RunFetchSection, RunMode, RunPersistSection, RunReport, RunResult, TargetId,
};

use super::super::domain::PersistedState;
use super::super::state::{StateLoad, baseline_phase_or_default, prior_compare_digest};

#[derive(Clone, Debug, Default)]
pub(super) struct RunReportSections {
    pub(super) fetch: Option<RunFetchSection>,
    pub(super) extraction: Option<RunExtractionSection>,
    pub(super) compare: Option<RunCompareSection>,
    pub(super) change: Option<RunChangeSection>,
}

#[derive(Clone, Debug)]
pub(super) struct RunReportLifecycle {
    previous_compare_digest_sha256: Option<String>,
    current_compare_digest_sha256: Option<String>,
    baseline_phase_before_run: BaselinePhase,
    baseline_phase_after_run: BaselinePhase,
}

impl RunReportLifecycle {
    pub(super) fn invalid_target() -> Self {
        Self {
            previous_compare_digest_sha256: None,
            current_compare_digest_sha256: None,
            baseline_phase_before_run: BaselinePhase::NeverSucceeded,
            baseline_phase_after_run: BaselinePhase::NeverSucceeded,
        }
    }

    pub(super) fn from_state_snapshot(
        state: &StateLoad,
        current_compare_digest_sha256: Option<String>,
    ) -> Self {
        Self {
            previous_compare_digest_sha256: prior_compare_digest(state),
            current_compare_digest_sha256,
            baseline_phase_before_run: baseline_phase_or_default(state),
            baseline_phase_after_run: baseline_phase_or_default(state),
        }
    }

    pub(super) fn from_live_state_transition(
        before: &StateLoad,
        after: Option<&PersistedState>,
        current_compare_digest_sha256: Option<String>,
    ) -> Self {
        Self {
            previous_compare_digest_sha256: prior_compare_digest(before),
            current_compare_digest_sha256,
            baseline_phase_before_run: baseline_phase_or_default(before),
            baseline_phase_after_run: after
                .map(PersistedState::baseline_phase)
                .unwrap_or_else(|| baseline_phase_or_default(before)),
        }
    }
}

pub(super) struct RunReportDraft {
    pub(super) target_id: TargetId,
    pub(super) display_name: Option<String>,
    pub(super) run_started_at: String,
    pub(super) run_mode: RunMode,
    pub(super) result: RunResult,
    pub(super) compare_basis: CompareBasis,
    pub(super) lifecycle: RunReportLifecycle,
    pub(super) sections: RunReportSections,
    pub(super) persist: RunPersistSection,
}

pub(super) fn build_run_report(draft: RunReportDraft) -> RunReport {
    RunReport {
        schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: RUN_REPORT_SCHEMA_VERSION,
        run_report_digest_sha256: String::new(),
        target_id: draft.target_id,
        display_name: draft.display_name,
        run_started_at: draft.run_started_at,
        run_finished_at: String::new(),
        run_mode: draft.run_mode,
        result: draft.result,
        compare_basis: draft.compare_basis,
        previous_compare_digest_sha256: draft.lifecycle.previous_compare_digest_sha256,
        current_compare_digest_sha256: draft.lifecycle.current_compare_digest_sha256,
        baseline_phase_before_run: draft.lifecycle.baseline_phase_before_run,
        baseline_phase_after_run: draft.lifecycle.baseline_phase_after_run,
        fetch: draft.sections.fetch,
        extraction: draft.sections.extraction,
        compare: draft.sections.compare,
        change: draft.sections.change,
        persist: draft.persist,
        notifications: Vec::new(),
        extensions: None,
    }
}

pub(super) fn successful_result(outcome: crate::RunOutcome) -> Result<RunResult, CoreError> {
    match outcome {
        crate::RunOutcome::Initialized => Ok(RunResult::Initialized),
        crate::RunOutcome::Changed => Ok(RunResult::Changed),
        crate::RunOutcome::Unchanged => Ok(RunResult::Unchanged),
        crate::RunOutcome::FailedTransient
        | crate::RunOutcome::FailedPermanent
        | crate::RunOutcome::SkippedDisabled => Err(CoreError::internal(
            "successful execute path received a non-success run outcome",
        )),
    }
}

pub(super) fn failure_result(
    cause: RunFailureCause,
    error_detail: ProcessErrorDetail,
) -> RunResult {
    match cause.failure_class() {
        crate::FailureClass::Transient => RunResult::FailedTransient {
            cause,
            error_detail,
        },
        crate::FailureClass::Permanent => RunResult::FailedPermanent {
            cause,
            error_detail,
        },
    }
}

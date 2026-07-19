use crate::model::detail_from_core_error;
use crate::{
    CoreError, DeliveryOutcome, DiagnosticDetail, DiagnosticOperation, LifecycleSnapshot,
    OutboxOverflow, PolicyEvaluation, RunMode, RunOutcome, RunReport, StatusKind, TargetDocument,
    TargetPaths,
};

use super::acquire::now_utc;

pub(super) struct FinishReport<'a> {
    pub(super) target: &'a TargetDocument,
    pub(super) mode: RunMode,
    pub(super) outcome: RunOutcome,
    pub(super) started: String,
    pub(super) digest: Option<String>,
    pub(super) observation: Option<crate::Observation>,
    pub(super) previous: Option<String>,
    pub(super) error: Option<DiagnosticDetail>,
    pub(super) policy_evaluation: PolicyEvaluation,
    pub(super) lifecycle_before: Option<LifecycleSnapshot>,
    pub(super) lifecycle_after: Option<LifecycleSnapshot>,
    pub(super) persisted: bool,
    pub(super) delivery: DeliveryEvidence,
}

#[derive(Clone, Default)]
pub(super) struct DeliveryEvidence {
    pub(super) outcomes: Vec<DeliveryOutcome>,
    pub(super) overflow: Vec<OutboxOverflow>,
    pub(super) outbox_error_detail: Option<DiagnosticDetail>,
}

pub(super) fn finish_report(input: FinishReport<'_>) -> Result<RunReport, CoreError> {
    RunReport::new(crate::model::RunReportParts {
        target_id: input.target.target_id().to_owned(),
        display_name: Some(input.target.display_name().to_owned()),
        run_mode: input.mode,
        outcome: input.outcome,
        started: input.started,
        finished: now_utc()?,
        digest: input.digest,
        observation: input.observation,
        previous: input.previous,
        error: input.error,
        policy_evaluation: input.policy_evaluation,
        lifecycle_before: input.lifecycle_before,
        lifecycle_after: input.lifecycle_after,
        state_persisted: input.persisted,
        delivery_outcomes: input.delivery.outcomes,
        outbox_overflow: input.delivery.overflow,
        outbox_error_detail: input.delivery.outbox_error_detail,
    })
}

pub(super) fn report_for_target_error(
    paths: &TargetPaths,
    mode: RunMode,
    started: String,
    error: CoreError,
    operation: DiagnosticOperation,
) -> Result<RunReport, CoreError> {
    let outcome = if matches!(error, CoreError::Io { .. }) {
        RunOutcome::TargetUnavailable
    } else {
        RunOutcome::ConfigInvalid
    };
    RunReport::new(crate::model::RunReportParts {
        target_id: paths.target_id().to_owned(),
        display_name: None,
        run_mode: mode,
        outcome,
        started,
        finished: now_utc()?,
        digest: None,
        observation: None,
        previous: None,
        error: Some(detail_from_error_for_operation(
            &error,
            operation,
            Some(paths.target_file()),
        )),
        policy_evaluation: PolicyEvaluation::not_evaluated(),
        lifecycle_before: None,
        lifecycle_after: None,
        state_persisted: false,
        delivery_outcomes: Vec::new(),
        outbox_overflow: Vec::new(),
        outbox_error_detail: None,
    })
}

pub(super) fn target_load_status_kind(error: &CoreError) -> StatusKind {
    if matches!(error, CoreError::Io { .. }) {
        StatusKind::UnavailableTarget
    } else {
        StatusKind::InvalidConfig
    }
}

/// Decomposes a core error at the exact closed operation where it was observed.
pub(super) fn detail_from_error_for_operation(
    error: &CoreError,
    operation: DiagnosticOperation,
    path: Option<std::path::PathBuf>,
) -> DiagnosticDetail {
    detail_from_core_error(
        error,
        operation,
        path.map(|value| value.display().to_string()),
    )
}

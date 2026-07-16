use crate::{
    CoreError, DeliveryOutcome, OutboxOverflow, ProcessErrorDetail, ProcessErrorKind, RunMode,
    RunOutcome, RunReport, StatusKind, TargetDocument, TargetPaths,
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
    pub(super) error: Option<ProcessErrorDetail>,
    pub(super) persisted: bool,
    pub(super) delivery: DeliveryEvidence,
}

#[derive(Clone, Default)]
pub(super) struct DeliveryEvidence {
    pub(super) outcomes: Vec<DeliveryOutcome>,
    pub(super) overflow: Vec<OutboxOverflow>,
    pub(super) outbox_error: Option<String>,
}

pub(super) fn finish_report(input: FinishReport<'_>) -> Result<RunReport, CoreError> {
    Ok(RunReport::new(crate::model::RunReportParts {
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
        state_persisted: input.persisted,
        delivery_outcomes: input.delivery.outcomes,
        outbox_overflow: input.delivery.overflow,
        outbox_error: input.delivery.outbox_error,
    }))
}

pub(super) fn report_for_target_load(
    paths: &TargetPaths,
    mode: RunMode,
    started: String,
    error: CoreError,
) -> Result<RunReport, CoreError> {
    let outcome = if matches!(error, CoreError::Io { .. }) {
        RunOutcome::TargetUnavailable
    } else {
        RunOutcome::ConfigInvalid
    };
    Ok(RunReport::new(crate::model::RunReportParts {
        target_id: paths.target_id().to_owned(),
        display_name: None,
        run_mode: mode,
        outcome,
        started,
        finished: now_utc()?,
        digest: None,
        observation: None,
        previous: None,
        error: Some(detail_from_error(&error, Some(paths.target_file()))),
        state_persisted: false,
        delivery_outcomes: Vec::new(),
        outbox_overflow: Vec::new(),
        outbox_error: None,
    }))
}

pub(super) fn target_load_status_kind(error: &CoreError) -> StatusKind {
    if matches!(error, CoreError::Io { .. }) {
        StatusKind::UnavailableTarget
    } else {
        StatusKind::InvalidConfig
    }
}

pub(super) fn detail_from_error(
    error: &CoreError,
    path: Option<std::path::PathBuf>,
) -> ProcessErrorDetail {
    let kind = match error {
        CoreError::Io { .. } => ProcessErrorKind::Io,
        CoreError::Json(_) => ProcessErrorKind::Json,
        CoreError::Toml(_) => ProcessErrorKind::Toml,
        _ => ProcessErrorKind::Contract,
    };
    ProcessErrorDetail::new(
        kind,
        error.to_string(),
        path.map(|value| value.display().to_string()),
    )
}

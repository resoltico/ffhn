use crate::{
    CoreError, ProcessErrorDetail, ReasonCode, STATUS_REPORT_SCHEMA_NAME,
    STATUS_REPORT_SCHEMA_VERSION, StatePhase, StatusReport, TargetDocument, TargetId, TargetPaths,
    TargetStatus,
};

use super::lock::lock_shared;
use super::state::{StateLoad, load_state, snapshot_digest_summary, state_error_detail};
use super::target_load::{TargetLoad, load_target_document, read_target_document};

pub(crate) fn validate_target(paths: &TargetPaths) -> Result<TargetDocument, CoreError> {
    paths.require_watch_root_directory()?;
    let target = read_target_document(&paths.target_file())?;
    validate_target_against_paths(paths, target)
}

pub(crate) fn status(paths: &TargetPaths) -> Result<StatusReport, CoreError> {
    paths.require_watch_root_directory()?;
    match load_target_document(paths)? {
        TargetLoad::Valid(target) => {
            let _lock = lock_shared(paths)?;
            status_for_valid_target(&target, load_state(paths))
        }
        TargetLoad::Invalid(error_detail) => {
            let report = invalid_target_status_report(paths, error_detail);
            report.validate()?;
            Ok(report)
        }
    }
}

fn invalid_target_status_report(
    paths: &TargetPaths,
    error_detail: ProcessErrorDetail,
) -> StatusReport {
    StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: TargetId::new(paths.target_id()).expect("validated path target id"),
        target_status: TargetStatus::Invalid,
        reason_code: ReasonCode::ConfigInvalid,
        error_detail: Some(error_detail),
        state_phase: None,
        current_snapshot: None,
        snapshot_history: Vec::new(),
        extensions: None,
    }
}

pub(crate) fn validate_target_against_paths(
    paths: &TargetPaths,
    target: TargetDocument,
) -> Result<TargetDocument, CoreError> {
    target.validate()?;
    if target.target_id.as_str() != paths.target_id() {
        return Err(CoreError::contract(
            "target.toml target_id does not match its directory name",
        ));
    }
    Ok(target)
}

fn status_for_valid_target(
    target: &TargetDocument,
    state: StateLoad,
) -> Result<StatusReport, CoreError> {
    let report = match state {
        StateLoad::Missing => StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target.target_id.clone(),
            target_status: TargetStatus::Pending,
            reason_code: ReasonCode::Ok,
            error_detail: None,
            state_phase: Some(StatePhase::NeverSucceeded),
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        },
        StateLoad::Unreadable(_) => StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target.target_id.clone(),
            target_status: TargetStatus::Invalid,
            reason_code: ReasonCode::StateInvalid,
            error_detail: state_error_detail(&state).cloned(),
            state_phase: Some(StatePhase::NeverSucceeded),
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        },
        StateLoad::InvalidDocument { phase, .. } => StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target.target_id.clone(),
            target_status: TargetStatus::Invalid,
            reason_code: ReasonCode::StateInvalid,
            error_detail: state_error_detail(&state).cloned(),
            state_phase: Some(phase.unwrap_or(StatePhase::NeverSucceeded)),
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        },
        StateLoad::IntegrityMismatch { phase, .. } => StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target.target_id.clone(),
            target_status: TargetStatus::Invalid,
            reason_code: ReasonCode::IntegrityMismatch,
            error_detail: state_error_detail(&state).cloned(),
            state_phase: Some(phase.unwrap_or(StatePhase::NeverSucceeded)),
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        },
        StateLoad::Valid(loaded) => StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target.target_id.clone(),
            target_status: if loaded.document.state_phase == StatePhase::HasBaseline {
                TargetStatus::Ready
            } else {
                TargetStatus::Pending
            },
            reason_code: ReasonCode::Ok,
            error_detail: None,
            state_phase: Some(loaded.document.state_phase),
            current_snapshot: loaded
                .document
                .current_snapshot
                .as_ref()
                .map(snapshot_digest_summary),
            snapshot_history: loaded
                .document
                .snapshot_history
                .iter()
                .map(snapshot_digest_summary)
                .collect(),
            extensions: None,
        },
    };
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests;

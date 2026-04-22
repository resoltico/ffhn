use crate::{
    ArtifactStatus, CoreError, ReasonCode, STATUS_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_VERSION,
    StatePhase, StatusReport, TargetDocument, TargetPaths, TargetStatus,
};

use super::lock::lock_shared;
use super::state::{StateLoad, load_state, snapshot_digest_summary};
use super::storage::read_toml;

pub(crate) fn validate_target(paths: &TargetPaths) -> Result<TargetDocument, CoreError> {
    let target = read_toml::<TargetDocument>(&paths.target_file())?;
    validate_target_against_paths(paths, target)
}

pub(crate) fn status(paths: &TargetPaths) -> Result<StatusReport, CoreError> {
    match validate_target(paths) {
        Ok(target) => {
            let _lock = lock_shared(paths)?;
            status_for_valid_target(&target, load_state(paths))
        }
        Err(_) => {
            let report = invalid_target_status_report(paths);
            report.validate()?;
            Ok(report)
        }
    }
}

fn invalid_target_status_report(paths: &TargetPaths) -> StatusReport {
    StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: paths.target_id().to_owned(),
        target_status: TargetStatus::Invalid,
        reason_code: ReasonCode::ConfigInvalid,
        state_phase: None,
        artifacts: ArtifactStatus {
            current_valid: false,
            previous_valid: false,
        },
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
    if target.target_id != paths.target_id() {
        return Err(CoreError::htmlcut(
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
            state_phase: Some(StatePhase::NeverSucceeded),
            artifacts: ArtifactStatus {
                current_valid: true,
                previous_valid: true,
            },
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        },
        StateLoad::Unreadable => StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target.target_id.clone(),
            target_status: TargetStatus::Invalid,
            reason_code: ReasonCode::StateInvalid,
            state_phase: Some(StatePhase::NeverSucceeded),
            artifacts: ArtifactStatus {
                current_valid: false,
                previous_valid: false,
            },
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        },
        StateLoad::InvalidSchema(phase) => StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target.target_id.clone(),
            target_status: TargetStatus::Invalid,
            reason_code: ReasonCode::StateInvalid,
            state_phase: Some(phase.unwrap_or(StatePhase::NeverSucceeded)),
            artifacts: ArtifactStatus {
                current_valid: false,
                previous_valid: false,
            },
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        },
        StateLoad::IntegrityMismatch(phase) => StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target.target_id.clone(),
            target_status: TargetStatus::Invalid,
            reason_code: ReasonCode::IntegrityMismatch,
            state_phase: Some(phase.unwrap_or(StatePhase::NeverSucceeded)),
            artifacts: ArtifactStatus {
                current_valid: false,
                previous_valid: false,
            },
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
            state_phase: Some(loaded.document.state_phase),
            artifacts: ArtifactStatus {
                current_valid: true,
                previous_valid: true,
            },
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

use crate::{
    BaselinePhase, CoreError, ProcessErrorDetail, STATUS_REPORT_SCHEMA_NAME,
    STATUS_REPORT_SCHEMA_VERSION, StatusReport, StatusSummary, TargetDocument, TargetId,
    TargetPaths,
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
        TargetLoad::Unavailable(error_detail) => {
            let report = unavailable_target_status_report(paths, error_detail);
            report.validate()?;
            Ok(report)
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
        display_name: None,
        enabled: None,
        status: StatusSummary::InvalidConfig { error_detail },
        extensions: None,
    }
}

fn unavailable_target_status_report(
    paths: &TargetPaths,
    error_detail: ProcessErrorDetail,
) -> StatusReport {
    StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: TargetId::new(paths.target_id()).expect("validated path target id"),
        display_name: None,
        enabled: None,
        status: StatusSummary::UnavailableTarget { error_detail },
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
            display_name: Some(target.display_name.clone()),
            enabled: Some(target.enabled),
            status: StatusSummary::Pending,
            extensions: None,
        },
        StateLoad::Unreadable(_) => StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target.target_id.clone(),
            display_name: Some(target.display_name.clone()),
            enabled: Some(target.enabled),
            status: StatusSummary::InvalidState {
                baseline_phase: BaselinePhase::NeverSucceeded,
                error_detail: state_error_detail(&state)
                    .cloned()
                    .expect("unreadable state carries detail"),
            },
            extensions: None,
        },
        StateLoad::InvalidDocument { phase, .. } => StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target.target_id.clone(),
            display_name: Some(target.display_name.clone()),
            enabled: Some(target.enabled),
            status: StatusSummary::InvalidState {
                baseline_phase: phase.unwrap_or(BaselinePhase::NeverSucceeded),
                error_detail: state_error_detail(&state)
                    .cloned()
                    .expect("invalid state carries detail"),
            },
            extensions: None,
        },
        StateLoad::IntegrityMismatch { phase, .. } => StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target.target_id.clone(),
            display_name: Some(target.display_name.clone()),
            enabled: Some(target.enabled),
            status: StatusSummary::IntegrityMismatch {
                baseline_phase: phase.unwrap_or(BaselinePhase::NeverSucceeded),
                error_detail: state_error_detail(&state)
                    .cloned()
                    .expect("integrity mismatch carries detail"),
            },
            extensions: None,
        },
        StateLoad::Valid(loaded) => StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target.target_id.clone(),
            display_name: Some(target.display_name.clone()),
            enabled: Some(target.enabled),
            status: match &loaded.state.baseline {
                super::domain::PersistedBaselineState::Pending => StatusSummary::Pending,
                super::domain::PersistedBaselineState::Ready {
                    current_snapshot,
                    snapshot_history,
                } => StatusSummary::Ready {
                    current_snapshot: snapshot_digest_summary(current_snapshot),
                    snapshot_history: snapshot_history
                        .iter()
                        .map(snapshot_digest_summary)
                        .collect(),
                },
            },
            extensions: None,
        },
    };
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests;

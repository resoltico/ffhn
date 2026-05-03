use super::checks::validate_status_report_identity;
use super::*;
use crate::TargetId;

mod wire;

/// Digest summary for one snapshot in `ffhn.status_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotDigestSummary {
    /// Canonical text digest.
    pub(crate) canonical_text_sha256: String,
    /// Outer HTML digest.
    pub(crate) outer_html_sha256: String,
    /// Capture timestamp.
    pub(crate) captured_at: String,
}

/// `ffhn.status_report` schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusReport {
    /// Frozen schema identity.
    pub(crate) schema_name: String,
    /// Frozen schema version.
    pub(crate) schema_version: u32,
    /// Target id.
    pub(crate) target_id: TargetId,
    /// Target status.
    pub(crate) target_status: TargetStatus,
    /// Reason code.
    pub(crate) reason_code: ReasonCode,
    /// Structured detail for invalid target or state reports.
    pub(crate) error_detail: Option<ProcessErrorDetail>,
    /// State phase.
    pub(crate) state_phase: Option<StatePhase>,
    /// Current snapshot digest summary.
    pub(crate) current_snapshot: Option<SnapshotDigestSummary>,
    /// Older retained snapshots, newest first.
    pub(crate) snapshot_history: Vec<SnapshotDigestSummary>,
    /// Reserved extensions.
    pub(crate) extensions: Extensions,
}

impl StatusReport {
    /// Returns the frozen schema name.
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    /// Returns the frozen schema version.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the target id encoded in the report.
    pub fn target_id(&self) -> &str {
        self.target_id.as_str()
    }

    /// Returns the target status.
    pub fn target_status(&self) -> TargetStatus {
        self.target_status
    }

    /// Returns the reason code.
    pub fn reason_code(&self) -> ReasonCode {
        self.reason_code
    }

    /// Returns the structured invalid-target or invalid-state detail when one exists.
    pub fn error_detail(&self) -> Option<&ProcessErrorDetail> {
        self.error_detail.as_ref()
    }

    /// Returns the parsed state phase when one exists.
    pub fn state_phase(&self) -> Option<StatePhase> {
        self.state_phase
    }

    /// Returns the current snapshot digest summary when one exists.
    pub fn current_snapshot(&self) -> Option<&SnapshotDigestSummary> {
        self.current_snapshot.as_ref()
    }

    /// Returns historical snapshot summaries in newest-first order.
    pub fn snapshot_history(&self) -> &[SnapshotDigestSummary] {
        &self.snapshot_history
    }

    /// Returns any reserved extensions.
    pub fn extensions(&self) -> Option<&std::collections::BTreeMap<String, serde_json::Value>> {
        self.extensions.as_ref()
    }

    /// Validates one status report.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the schema identity, snapshot digests, timestamp ordering, or
    /// status/state/snapshot combination does not match FFHN's frozen status-report contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_status_report_identity(&self.schema_name, self.schema_version)?;
        self.error_detail
            .as_ref()
            .map(ProcessErrorDetail::validate)
            .transpose()?;
        if let Some(snapshot) = &self.current_snapshot {
            validate_snapshot_digest_summary(snapshot)?;
        }
        for snapshot in &self.snapshot_history {
            validate_snapshot_digest_summary(snapshot)?;
        }
        validate_status_contract(self)?;
        validate_snapshot_history_order(&self.snapshot_history)?;
        if let (Some(current_snapshot), Some(previous_snapshot)) =
            (&self.current_snapshot, self.snapshot_history.first())
        {
            crate::model::validate::validate_timestamp_not_before(
                "status.snapshot_history[0].captured_at",
                &previous_snapshot.captured_at,
                "status.current_snapshot.captured_at",
                &current_snapshot.captured_at,
            )?;
        }
        Ok(())
    }
}

impl SnapshotDigestSummary {
    /// Returns the canonical-text digest.
    pub fn canonical_text_sha256(&self) -> &str {
        &self.canonical_text_sha256
    }

    /// Returns the outer-HTML digest.
    pub fn outer_html_sha256(&self) -> &str {
        &self.outer_html_sha256
    }

    /// Returns the capture timestamp.
    pub fn captured_at(&self) -> &str {
        &self.captured_at
    }
}

fn validate_snapshot_digest_summary(snapshot: &SnapshotDigestSummary) -> Result<(), CoreError> {
    validate_sha256(&snapshot.canonical_text_sha256)?;
    validate_sha256(&snapshot.outer_html_sha256)?;
    validate_timestamp(&snapshot.captured_at)
}

fn validate_status_contract(report: &StatusReport) -> Result<(), CoreError> {
    match (report.target_status, report.reason_code, report.state_phase) {
        (TargetStatus::Invalid, ReasonCode::ConfigInvalid, None) => {
            require_error_detail(report, "config_invalid status reports")?;
            validate_null_snapshot_state(report, "config_invalid status reports")?
        }
        (TargetStatus::Pending, ReasonCode::Ok, Some(StatePhase::NeverSucceeded)) => {
            forbid_error_detail(report, "pending status reports")?;
            validate_null_snapshot_state(report, "pending status reports")?
        }
        (TargetStatus::Ready, ReasonCode::Ok, Some(StatePhase::HasBaseline)) => {
            forbid_error_detail(report, "ready status reports")?;
            if report.current_snapshot.is_none() {
                return Err(CoreError::contract(
                    "ready status reports require current_snapshot",
                ));
            }
        }
        (
            TargetStatus::Invalid,
            ReasonCode::StateInvalid | ReasonCode::IntegrityMismatch,
            Some(_),
        ) => {
            require_error_detail(report, "invalid state status reports")?;
            validate_null_snapshot_state(report, "invalid state status reports")?
        }
        (TargetStatus::Invalid, _, None) => {
            return Err(CoreError::contract(
                "null status.state_phase is only valid for config_invalid",
            ));
        }
        _ => {
            return Err(CoreError::contract(
                "status target_status, reason_code, and state_phase must use one supported FFHN combination",
            ));
        }
    }

    Ok(())
}

fn require_error_detail(report: &StatusReport, context: &str) -> Result<(), CoreError> {
    if report.error_detail.is_none() {
        return Err(CoreError::contract(format!(
            "{context} require error_detail",
        )));
    }
    Ok(())
}

fn forbid_error_detail(report: &StatusReport, context: &str) -> Result<(), CoreError> {
    if report.error_detail.is_some() {
        return Err(CoreError::contract(format!(
            "{context} must not include error_detail",
        )));
    }
    Ok(())
}

fn validate_null_snapshot_state(report: &StatusReport, context: &str) -> Result<(), CoreError> {
    if report.current_snapshot.is_some() || !report.snapshot_history.is_empty() {
        return Err(CoreError::contract(format!(
            "{context} must not include snapshot summaries",
        )));
    }
    Ok(())
}

fn validate_snapshot_history_order(
    snapshot_history: &[SnapshotDigestSummary],
) -> Result<(), CoreError> {
    let mut previous_captured_at = None;
    for snapshot in snapshot_history {
        let captured_at = crate::model::validate::parse_timestamp(&snapshot.captured_at)?;
        if let Some(previous) = previous_captured_at
            && captured_at > previous
        {
            return Err(CoreError::contract(
                "status.snapshot_history must be ordered newest first",
            ));
        }
        previous_captured_at = Some(captured_at);
    }
    Ok(())
}

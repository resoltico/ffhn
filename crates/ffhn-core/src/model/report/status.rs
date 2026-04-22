use super::checks::validate_status_report_identity;
use super::*;

/// Artifact-integrity summary inside `ffhn.status_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArtifactStatus {
    /// Whether current-snapshot artifacts are valid.
    pub current_valid: bool,
    /// Whether previous-snapshot artifacts are valid.
    pub previous_valid: bool,
}

/// Digest summary for one snapshot in `ffhn.status_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SnapshotDigestSummary {
    /// Canonical text digest.
    pub canonical_text_sha256: String,
    /// Outer HTML digest.
    pub outer_html_sha256: String,
    /// Capture timestamp.
    pub captured_at: String,
}

/// `ffhn.status_report` schema.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StatusReport {
    /// Frozen schema identity.
    pub schema_name: String,
    /// Frozen schema version.
    pub schema_version: u32,
    /// Target id.
    pub target_id: String,
    /// Target status.
    pub target_status: TargetStatus,
    /// Reason code.
    pub reason_code: ReasonCode,
    /// State phase.
    pub state_phase: Option<StatePhase>,
    /// Artifact integrity summary.
    pub artifacts: ArtifactStatus,
    /// Current snapshot digest summary.
    pub current_snapshot: Option<SnapshotDigestSummary>,
    /// Older retained snapshots, newest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snapshot_history: Vec<SnapshotDigestSummary>,
    /// Reserved extensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Extensions,
}

impl StatusReport {
    /// Validates one status report.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_status_report_identity(&self.schema_name, self.schema_version)?;
        validate_target_id(&self.target_id)?;
        if self.state_phase.is_none()
            && !(self.reason_code == ReasonCode::ConfigInvalid
                && self.target_status == TargetStatus::Invalid)
        {
            return Err(CoreError::htmlcut(
                "null status.state_phase is only valid for config_invalid",
            ));
        }
        if let Some(snapshot) = &self.current_snapshot {
            validate_sha256(&snapshot.canonical_text_sha256)?;
            validate_sha256(&snapshot.outer_html_sha256)?;
            validate_timestamp(&snapshot.captured_at)?;
        }
        for snapshot in &self.snapshot_history {
            validate_sha256(&snapshot.canonical_text_sha256)?;
            validate_sha256(&snapshot.outer_html_sha256)?;
            validate_timestamp(&snapshot.captured_at)?;
        }
        Ok(())
    }
}

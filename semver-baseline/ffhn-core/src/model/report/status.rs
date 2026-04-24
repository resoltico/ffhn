use super::checks::validate_status_report_identity;
use super::*;
use crate::TargetId;

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
#[serde(try_from = "RawStatusReport")]
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
    /// State phase.
    pub(crate) state_phase: Option<StatePhase>,
    /// Artifact integrity summary.
    pub(crate) artifacts: ArtifactStatus,
    /// Current snapshot digest summary.
    pub(crate) current_snapshot: Option<SnapshotDigestSummary>,
    /// Older retained snapshots, newest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) snapshot_history: Vec<SnapshotDigestSummary>,
    /// Reserved extensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) extensions: Extensions,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStatusReport {
    schema_name: String,
    schema_version: u32,
    target_id: String,
    target_status: TargetStatus,
    reason_code: ReasonCode,
    state_phase: Option<StatePhase>,
    artifacts: ArtifactStatus,
    current_snapshot: Option<SnapshotDigestSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    snapshot_history: Vec<SnapshotDigestSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
}

impl TryFrom<RawStatusReport> for StatusReport {
    type Error = CoreError;

    fn try_from(raw: RawStatusReport) -> Result<Self, Self::Error> {
        let report = Self {
            schema_name: raw.schema_name,
            schema_version: raw.schema_version,
            target_id: raw.target_id.try_into()?,
            target_status: raw.target_status,
            reason_code: raw.reason_code,
            state_phase: raw.state_phase,
            artifacts: raw.artifacts,
            current_snapshot: raw.current_snapshot,
            snapshot_history: raw.snapshot_history,
            extensions: raw.extensions,
        };
        report.validate()?;
        Ok(report)
    }
}

impl StatusReport {
    /// Validates one status report.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_status_report_identity(&self.schema_name, self.schema_version)?;
        if self.state_phase.is_none()
            && !(self.reason_code == ReasonCode::ConfigInvalid
                && self.target_status == TargetStatus::Invalid)
        {
            return Err(CoreError::contract(
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

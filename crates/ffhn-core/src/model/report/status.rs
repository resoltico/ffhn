use super::checks::validate_status_report_identity;
use super::*;
use crate::TargetId;

mod wire;

/// Digest summary for one snapshot in `ffhn.status_report`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotDigestSummary {
    /// Canonical text digest.
    pub(crate) canonical_text_sha256: String,
    /// Outer HTML digest.
    pub(crate) outer_html_sha256: String,
    /// Capture timestamp.
    pub(crate) captured_at: String,
}

/// Target status summary inside `ffhn.status_report`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum StatusSummary {
    /// Valid target without a baseline.
    Pending,
    /// Valid target with one current baseline and optional retained history.
    Ready {
        /// Current snapshot digest summary.
        current_snapshot: SnapshotDigestSummary,
        /// Older retained snapshots, newest first.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        snapshot_history: Vec<SnapshotDigestSummary>,
    },
    /// Invalid target configuration.
    InvalidConfig {
        /// Structured invalid-target detail.
        error_detail: ProcessErrorDetail,
    },
    /// Explicit target path missing or unreadable.
    UnavailableTarget {
        /// Structured unavailable-target detail.
        error_detail: ProcessErrorDetail,
    },
    /// Invalid persisted state.
    InvalidState {
        /// Parsed persisted state phase when FFHN could still recover it.
        baseline_phase: BaselinePhase,
        /// Structured invalid-state detail.
        error_detail: ProcessErrorDetail,
    },
    /// Snapshot artifact integrity mismatch.
    IntegrityMismatch {
        /// Parsed persisted state phase when FFHN could still recover it.
        baseline_phase: BaselinePhase,
        /// Structured integrity-failure detail.
        error_detail: ProcessErrorDetail,
    },
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
    /// Parsed display name when FFHN could trust the target document.
    pub(crate) display_name: Option<String>,
    /// Parsed enablement when FFHN could validate the target document.
    pub(crate) enabled: Option<bool>,
    /// Status summary.
    pub(crate) status: StatusSummary,
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

    /// Returns the parsed target display name when FFHN could trust the target document.
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    /// Returns the parsed target enablement when FFHN could trust the target document.
    pub const fn enabled(&self) -> Option<bool> {
        self.enabled
    }

    /// Returns the local status summary.
    pub const fn status(&self) -> &StatusSummary {
        &self.status
    }

    /// Returns the structured invalid-target or invalid-state detail when one exists.
    pub fn error_detail(&self) -> Option<&ProcessErrorDetail> {
        self.status.error_detail()
    }

    /// Returns the parsed state phase when one exists.
    pub const fn baseline_phase(&self) -> Option<BaselinePhase> {
        self.status.baseline_phase()
    }

    /// Returns the current snapshot digest summary when one exists.
    pub fn current_snapshot(&self) -> Option<&SnapshotDigestSummary> {
        self.status.current_snapshot()
    }

    /// Returns historical snapshot summaries in newest-first order.
    pub fn snapshot_history(&self) -> &[SnapshotDigestSummary] {
        self.status.snapshot_history()
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
    /// status summary does not match FFHN's frozen status-report contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_status_report_identity(&self.schema_name, self.schema_version)?;
        validate_optional_non_empty("status_report.display_name", self.display_name.as_deref())?;
        self.validate_enabled_contract()?;
        self.status.validate()
    }

    fn validate_enabled_contract(&self) -> Result<(), CoreError> {
        match (&self.display_name, &self.enabled, &self.status) {
            (
                None,
                None,
                StatusSummary::InvalidConfig { .. } | StatusSummary::UnavailableTarget { .. },
            ) => Ok(()),
            (
                Some(_),
                Some(_),
                StatusSummary::Pending
                | StatusSummary::Ready { .. }
                | StatusSummary::InvalidState { .. }
                | StatusSummary::IntegrityMismatch { .. },
            ) => Ok(()),
            (Some(_), None, _) | (None, Some(_), _) => Err(CoreError::contract(
                "status report display_name and enabled must appear together",
            )),
            (None, None, _) => Err(CoreError::contract(
                "status report must carry display_name and enabled for every valid-target state",
            )),
            (
                Some(_),
                Some(_),
                StatusSummary::InvalidConfig { .. } | StatusSummary::UnavailableTarget { .. },
            ) => Err(CoreError::contract(
                "invalid target-load status reports must not carry display_name or enabled",
            )),
        }
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

impl StatusSummary {
    /// Returns the stable public status token.
    pub const fn kind_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready { .. } => "ready",
            Self::InvalidConfig { .. } => "invalid_config",
            Self::UnavailableTarget { .. } => "unavailable_target",
            Self::InvalidState { .. } => "invalid_state",
            Self::IntegrityMismatch { .. } => "integrity_mismatch",
        }
    }

    /// Returns whether the target is pending a first baseline.
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Returns whether the target has a valid baseline.
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    /// Returns whether the target is invalid or has an integrity failure.
    pub fn is_invalid(&self) -> bool {
        self.error_detail().is_some()
    }

    /// Returns the structured error detail when one exists.
    pub fn error_detail(&self) -> Option<&ProcessErrorDetail> {
        match self {
            Self::InvalidConfig { error_detail }
            | Self::UnavailableTarget { error_detail }
            | Self::InvalidState { error_detail, .. }
            | Self::IntegrityMismatch { error_detail, .. } => Some(error_detail),
            Self::Pending | Self::Ready { .. } => None,
        }
    }

    /// Returns the modeled state phase when one exists.
    pub const fn baseline_phase(&self) -> Option<BaselinePhase> {
        match self {
            Self::Pending => Some(BaselinePhase::NeverSucceeded),
            Self::Ready { .. } => Some(BaselinePhase::HasBaseline),
            Self::InvalidConfig { .. } | Self::UnavailableTarget { .. } => None,
            Self::InvalidState { baseline_phase, .. }
            | Self::IntegrityMismatch { baseline_phase, .. } => Some(*baseline_phase),
        }
    }

    /// Returns the current snapshot digest summary when one exists.
    pub const fn current_snapshot(&self) -> Option<&SnapshotDigestSummary> {
        match self {
            Self::Ready {
                current_snapshot, ..
            } => Some(current_snapshot),
            Self::Pending
            | Self::InvalidConfig { .. }
            | Self::UnavailableTarget { .. }
            | Self::InvalidState { .. }
            | Self::IntegrityMismatch { .. } => None,
        }
    }

    /// Returns historical snapshot summaries in newest-first order.
    pub fn snapshot_history(&self) -> &[SnapshotDigestSummary] {
        match self {
            Self::Ready {
                snapshot_history, ..
            } => snapshot_history,
            Self::Pending
            | Self::InvalidConfig { .. }
            | Self::UnavailableTarget { .. }
            | Self::InvalidState { .. }
            | Self::IntegrityMismatch { .. } => &[],
        }
    }

    fn validate(&self) -> Result<(), CoreError> {
        match self {
            Self::Pending => Ok(()),
            Self::Ready {
                current_snapshot,
                snapshot_history,
            } => {
                validate_snapshot_digest_summary(current_snapshot)?;
                for snapshot in snapshot_history {
                    validate_snapshot_digest_summary(snapshot)?;
                }
                validate_snapshot_history_order(snapshot_history)?;
                if let Some(previous_snapshot) = snapshot_history.first() {
                    crate::model::validate::validate_timestamp_not_before(
                        "status.ready.snapshot_history[0].captured_at",
                        &previous_snapshot.captured_at,
                        "status.ready.current_snapshot.captured_at",
                        &current_snapshot.captured_at,
                    )?;
                }
                Ok(())
            }
            Self::InvalidConfig { error_detail }
            | Self::UnavailableTarget { error_detail }
            | Self::InvalidState { error_detail, .. }
            | Self::IntegrityMismatch { error_detail, .. } => error_detail.validate(),
        }
    }
}

fn validate_optional_non_empty(field: &str, value: Option<&str>) -> Result<(), CoreError> {
    if let Some(value) = value {
        require_non_empty(field, value)?;
    }
    Ok(())
}

fn validate_snapshot_digest_summary(snapshot: &SnapshotDigestSummary) -> Result<(), CoreError> {
    validate_sha256(&snapshot.canonical_text_sha256)?;
    validate_sha256(&snapshot.outer_html_sha256)?;
    validate_timestamp(&snapshot.captured_at)
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
                "status.ready snapshot_history must be ordered newest first",
            ));
        }
        previous_captured_at = Some(captured_at);
    }
    Ok(())
}

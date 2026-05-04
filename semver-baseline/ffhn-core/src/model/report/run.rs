use url::Url;

use crate::TargetId;
use crate::model::validate::validate_timestamp_not_before;
use crate::stable_json::stable_digest_omitting_field;

use super::checks::{
    validate_notification_delivery, validate_run_change_section, validate_run_report_identity,
};
use super::notification::{ProcessErrorDetail, RunNotificationDelivery};
use super::*;

mod access;
mod wire;

pub use access::{RunCompareView, RunFetchView, RunNotificationDeliveryView, RunPersistView};

/// Fetch subsection inside `ffhn.run_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunFetchSection {
    /// Fetch engine used for the run.
    pub(crate) engine: FetchEngine,
    /// Final URL after redirects when known.
    pub(crate) final_url: Option<String>,
    /// HTTP status when known.
    pub(crate) http_status: Option<u16>,
    /// Response content type when known.
    pub(crate) content_type: Option<String>,
    /// Bytes actually read when known.
    pub(crate) bytes_read: Option<usize>,
    /// Wall-clock duration in milliseconds.
    pub(crate) duration_ms: u64,
}

/// Extraction subsection inside `ffhn.run_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunExtractionSection {
    /// Interop profile.
    pub(crate) interop_profile: String,
    /// HTMLCut plan digest.
    pub(crate) htmlcut_plan_digest_sha256: String,
    /// HTMLCut result digest.
    pub(crate) htmlcut_result_digest_sha256: String,
    /// Comparison-input digest.
    pub(crate) comparison_input_sha256: String,
    /// Outer-HTML digest.
    pub(crate) outer_html_sha256: String,
    /// Strategy kind.
    pub(crate) strategy_kind: SelectionKind,
    /// Selection mode.
    pub(crate) selection_mode: SelectionMatch,
    /// Output kind.
    pub(crate) output_kind: OutputKind,
    /// Candidate count.
    pub(crate) candidate_count: usize,
    /// Selected candidate index.
    pub(crate) selected_candidate_index: usize,
    /// Warning codes only.
    pub(crate) warning_codes: Vec<String>,
    /// Extraction-stage duration.
    pub(crate) duration_ms: u64,
}

/// Compare subsection inside `ffhn.run_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunCompareSection {
    /// Canonicalizer kinds applied in order.
    pub(crate) canonicalizers: Vec<String>,
    /// Compare-stage duration.
    pub(crate) duration_ms: u64,
}

/// Stable write status for one persisted FFHN artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PersistWriteStatus {
    /// FFHN did not attempt this write during the run.
    NotAttempted,
    /// FFHN wrote the artifact successfully.
    Written,
    /// FFHN attempted the write and it failed.
    Failed {
        /// Structured failure detail for the write attempt.
        error: ProcessErrorDetail,
    },
}

impl PersistWriteStatus {
    /// Returns whether FFHN did not attempt this write.
    pub const fn is_not_attempted(&self) -> bool {
        matches!(self, Self::NotAttempted)
    }

    /// Returns whether FFHN wrote the artifact successfully.
    pub const fn is_written(&self) -> bool {
        matches!(self, Self::Written)
    }

    /// Returns whether FFHN attempted the write and it failed.
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Returns the structured failure detail when the write failed.
    pub const fn error(&self) -> Option<&ProcessErrorDetail> {
        match self {
            Self::NotAttempted | Self::Written => None,
            Self::Failed { error } => Some(error),
        }
    }
}

/// Persist subsection inside `ffhn.run_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunPersistSection {
    /// Persist-stage duration.
    pub(crate) duration_ms: u64,
    /// Result of the `state.json` write path.
    pub(crate) state_write: PersistWriteStatus,
    /// Result of the `last_run.json` write path.
    pub(crate) last_run_write: PersistWriteStatus,
}

/// One changed region summary inside `ffhn.run_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunChangeRegion {
    /// One-based start line in the previous canonical text.
    pub(crate) previous_start_line: usize,
    /// Number of previous lines in the changed region.
    pub(crate) previous_line_count: usize,
    /// One-based start line in the current canonical text.
    pub(crate) current_start_line: usize,
    /// Number of current lines in the changed region.
    pub(crate) current_line_count: usize,
    /// Compact excerpt from the previous canonical text when available.
    pub(crate) previous_excerpt: Option<String>,
    /// Compact excerpt from the current canonical text when available.
    pub(crate) current_excerpt: Option<String>,
    /// Digest of the previous excerpt when available.
    pub(crate) previous_excerpt_sha256: Option<String>,
    /// Digest of the current excerpt when available.
    pub(crate) current_excerpt_sha256: Option<String>,
}

/// Machine-usable change summary inside `ffhn.run_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunChangeSection {
    /// Change discriminator.
    pub(crate) kind: ChangeKind,
    /// Previous canonical-text byte length.
    pub(crate) previous_text_bytes: Option<usize>,
    /// Current canonical-text byte length.
    pub(crate) current_text_bytes: usize,
    /// Previous canonical-text line count.
    pub(crate) previous_line_count: Option<usize>,
    /// Current canonical-text line count.
    pub(crate) current_line_count: usize,
    /// Number of equal leading lines.
    pub(crate) common_prefix_lines: usize,
    /// Number of equal trailing lines.
    pub(crate) common_suffix_lines: usize,
    /// Replaced line region when one exists.
    pub(crate) changed_region: Option<RunChangeRegion>,
}

/// `ffhn.run_report` schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunReport {
    /// Frozen schema identity.
    pub(crate) schema_name: String,
    /// Frozen schema version.
    pub(crate) schema_version: u32,
    /// Digest of this report with the field omitted.
    pub(crate) run_report_digest_sha256: String,
    /// Target id.
    pub(crate) target_id: TargetId,
    /// Run start time.
    pub(crate) run_started_at: String,
    /// Run finish time.
    pub(crate) run_finished_at: String,
    /// Execution mode.
    pub(crate) run_mode: RunMode,
    /// Run outcome.
    pub(crate) run_outcome: RunOutcome,
    /// Reason code.
    pub(crate) reason_code: ReasonCode,
    /// Failure classification when the run failed.
    pub(crate) failure_class: Option<FailureClass>,
    /// Structured detail for the primary run failure when one exists.
    pub(crate) error_detail: Option<ProcessErrorDetail>,
    /// Target status after the run.
    pub(crate) target_status_after_run: TargetStatus,
    /// Compare basis.
    pub(crate) compare_basis: CompareBasis,
    /// Previous compare digest.
    pub(crate) previous_compare_digest_sha256: Option<String>,
    /// Current compare digest.
    pub(crate) current_compare_digest_sha256: Option<String>,
    /// State phase before the run.
    pub(crate) state_phase_before_run: StatePhase,
    /// State phase after the run.
    pub(crate) state_phase_after_run: StatePhase,
    /// Fetch subsection.
    pub(crate) fetch: Option<RunFetchSection>,
    /// Extraction subsection.
    pub(crate) extraction: Option<RunExtractionSection>,
    /// Compare subsection.
    pub(crate) compare: Option<RunCompareSection>,
    /// Machine-usable change summary.
    pub(crate) change: Option<RunChangeSection>,
    /// Persist subsection.
    pub(crate) persist: RunPersistSection,
    /// Best-effort notification deliveries attempted by the run.
    pub(crate) notifications: Vec<RunNotificationDelivery>,
    /// Reserved extensions.
    pub(crate) extensions: Extensions,
}

impl RunReport {
    /// Computes and stores the report digest.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when FFHN cannot serialize the report body deterministically for the
    /// digest computation.
    pub fn with_digest(mut self) -> Result<Self, CoreError> {
        self.run_report_digest_sha256 =
            stable_digest_omitting_field(&self, "run_report_digest_sha256")?;
        Ok(self)
    }

    /// Validates one run report.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the schema identity, digest, timestamps, persist contract,
    /// outcome/reason coupling, extraction details, change summary, or notification deliveries do
    /// not match FFHN's frozen run-report contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_run_report_identity(&self.schema_name, self.schema_version)?;
        validate_sha256(&self.run_report_digest_sha256)?;
        self.error_detail
            .as_ref()
            .map(ProcessErrorDetail::validate)
            .transpose()?;
        let expected_digest = stable_digest_omitting_field(self, "run_report_digest_sha256")?;
        if expected_digest != self.run_report_digest_sha256 {
            return Err(CoreError::contract(
                "run_report_digest_sha256 does not match the report body",
            ));
        }
        validate_timestamp(&self.run_started_at)?;
        validate_timestamp(&self.run_finished_at)?;
        validate_timestamp_not_before(
            "run_report.run_started_at",
            &self.run_started_at,
            "run_report.run_finished_at",
            &self.run_finished_at,
        )?;
        validate_persist_contract(self)?;
        if let Some(previous) = &self.previous_compare_digest_sha256 {
            validate_sha256(previous)?;
        }
        if let Some(current) = &self.current_compare_digest_sha256 {
            validate_sha256(current)?;
        }
        if matches!(
            self.run_outcome,
            RunOutcome::Initialized | RunOutcome::Changed | RunOutcome::Unchanged
        ) && self.reason_code != ReasonCode::Ok
        {
            return Err(CoreError::contract(
                "successful outcomes require reason_code = ok",
            ));
        }
        if matches!(
            self.run_outcome,
            RunOutcome::Initialized | RunOutcome::Changed | RunOutcome::Unchanged
        ) && (self.failure_class.is_some() || self.error_detail.is_some())
        {
            return Err(CoreError::contract(
                "successful outcomes must not carry failure_class or error_detail",
            ));
        }
        if matches!(
            self.run_outcome,
            RunOutcome::Initialized | RunOutcome::Changed | RunOutcome::Unchanged
        ) && self.current_compare_digest_sha256.is_none()
        {
            return Err(CoreError::contract(
                "successful outcomes require current_compare_digest_sha256",
            ));
        }
        if matches!(
            self.run_outcome,
            RunOutcome::Initialized | RunOutcome::Changed | RunOutcome::Unchanged
        ) && (self.fetch.is_none() || self.extraction.is_none() || self.compare.is_none())
        {
            return Err(CoreError::contract(
                "successful outcomes require fetch, extraction, and compare sections",
            ));
        }
        if self.run_outcome == RunOutcome::SkippedDisabled
            && self.reason_code != ReasonCode::Disabled
        {
            return Err(CoreError::contract(
                "skipped_disabled requires reason_code = disabled",
            ));
        }
        if self.run_outcome == RunOutcome::SkippedDisabled
            && (self.fetch.is_some()
                || self.extraction.is_some()
                || self.compare.is_some()
                || self.current_compare_digest_sha256.is_some()
                || self.error_detail.is_some())
        {
            return Err(CoreError::contract(
                "skipped_disabled must not carry fetch, extraction, compare, current digests, or error_detail",
            ));
        }
        if matches!(
            self.run_outcome,
            RunOutcome::FailedTransient | RunOutcome::FailedPermanent | RunOutcome::SkippedDisabled
        ) && self.current_compare_digest_sha256.is_some()
            && self.reason_code != ReasonCode::PersistError
        {
            return Err(CoreError::contract(
                "failed or skipped runs must not carry current_compare_digest_sha256",
            ));
        }
        match self.run_outcome {
            RunOutcome::FailedTransient => {
                if self.error_detail.is_none() {
                    return Err(CoreError::contract(
                        "failed_transient requires error_detail",
                    ));
                }
                if self.failure_class != Some(FailureClass::Transient)
                    || self.reason_code.failure_class() != Some(FailureClass::Transient)
                {
                    return Err(CoreError::contract(
                        "failed_transient requires a transient reason_code and failure_class",
                    ));
                }
            }
            RunOutcome::FailedPermanent => {
                if self.error_detail.is_none() {
                    return Err(CoreError::contract(
                        "failed_permanent requires error_detail",
                    ));
                }
                if self.failure_class != Some(FailureClass::Permanent)
                    || self.reason_code.failure_class() != Some(FailureClass::Permanent)
                {
                    return Err(CoreError::contract(
                        "failed_permanent requires a permanent reason_code and failure_class",
                    ));
                }
            }
            RunOutcome::Initialized | RunOutcome::Changed | RunOutcome::Unchanged => {}
            RunOutcome::SkippedDisabled => {
                if self.failure_class.is_some() {
                    return Err(CoreError::contract(
                        "skipped_disabled must not carry failure_class",
                    ));
                }
            }
        }
        if let Some(fetch) = &self.fetch
            && let Some(final_url) = &fetch.final_url
        {
            Url::parse(final_url)?;
        }
        if let Some(extraction) = &self.extraction {
            if extraction.interop_profile != HTMLCUT_INTEROP_PROFILE {
                return Err(CoreError::contract(
                    "run_report.extraction.interop_profile must match the FFHN HTMLCut profile",
                ));
            }
            validate_sha256(&extraction.htmlcut_plan_digest_sha256)?;
            validate_sha256(&extraction.htmlcut_result_digest_sha256)?;
            validate_sha256(&extraction.comparison_input_sha256)?;
            validate_sha256(&extraction.outer_html_sha256)?;
            if extraction.candidate_count == 0 || extraction.selected_candidate_index == 0 {
                return Err(CoreError::contract(
                    "run_report.extraction candidate counts must be positive",
                ));
            }
            if extraction.selected_candidate_index > extraction.candidate_count {
                return Err(CoreError::contract(
                    "run_report.extraction selected_candidate_index must be within candidate_count",
                ));
            }
        }
        if let Some(change) = &self.change {
            validate_run_change_section(change)?;
        }
        for notification in &self.notifications {
            validate_notification_delivery(notification)?;
        }
        Ok(())
    }
}

fn validate_persist_contract(report: &RunReport) -> Result<(), CoreError> {
    validate_persist_write_status(&report.persist.state_write)?;
    validate_persist_write_status(&report.persist.last_run_write)?;

    if report.run_mode == RunMode::DryRun {
        if !report.persist.state_write.is_not_attempted() {
            return Err(CoreError::contract(
                "dry_run reports must not persist state or emit notifications",
            ));
        }
        if !report.persist.last_run_write.is_not_attempted() {
            return Err(CoreError::contract(
                "dry_run reports must not persist state or emit notifications",
            ));
        }
        if !report.notifications.is_empty() {
            return Err(CoreError::contract(
                "dry_run reports must not persist state or emit notifications",
            ));
        }
    }

    let has_persist_failure = report.persist.has_failure();
    if has_persist_failure {
        if report.run_outcome != RunOutcome::FailedTransient {
            return Err(CoreError::contract(
                "persist write failures require failed_transient with reason_code = persist_error",
            ));
        }
        if report.reason_code != ReasonCode::PersistError {
            return Err(CoreError::contract(
                "persist write failures require failed_transient with reason_code = persist_error",
            ));
        }
        if report.failure_class != Some(FailureClass::Transient) {
            return Err(CoreError::contract(
                "persist write failures require failed_transient with reason_code = persist_error",
            ));
        }
    }

    if !has_persist_failure && report.reason_code == ReasonCode::PersistError {
        return Err(CoreError::contract(
            "reason_code = persist_error requires a failed persist write",
        ));
    }

    Ok(())
}

fn validate_persist_write_status(status: &PersistWriteStatus) -> Result<(), CoreError> {
    if let PersistWriteStatus::Failed { error } = status {
        error.validate()?;
    }
    Ok(())
}

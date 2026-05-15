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
    /// Comparison-input digest.
    pub(crate) comparison_input_sha256: String,
    /// Outer-HTML digest.
    pub(crate) outer_html_sha256: String,
    /// Strategy kind.
    pub(crate) selection_kind: SelectionKind,
    /// Selection mode.
    pub(crate) selection_match: SelectionMatch,
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
#[non_exhaustive]
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
    /// Returns the stable public status token.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NotAttempted => "not_attempted",
            Self::Written => "written",
            Self::Failed { .. } => "failed",
        }
    }

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
    /// Result of the primary persistence transaction that publishes snapshots and `state.json`.
    pub(crate) state_commit: PersistWriteStatus,
    /// Wall-clock duration of the primary persistence transaction in milliseconds.
    pub(crate) state_commit_duration_ms: u64,
    /// Result of the `last_run.json` write path.
    pub(crate) last_run_write: PersistWriteStatus,
    /// Wall-clock duration of the `last_run.json` write path in milliseconds.
    pub(crate) last_run_write_duration_ms: u64,
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

/// Structured run result inside `ffhn.run_report`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum RunResult {
    /// First successful baseline capture.
    Initialized,
    /// Successful change.
    Changed,
    /// Successful no-change run.
    Unchanged,
    /// Target disabled during a live explicit run.
    SkippedDisabled,
    /// Structured retry-later failure.
    FailedTransient {
        /// Local cause for the failed run.
        cause: RunFailureCause,
        /// Structured primary failure detail.
        error_detail: ProcessErrorDetail,
    },
    /// Structured investigate-now failure.
    FailedPermanent {
        /// Local cause for the failed run.
        cause: RunFailureCause,
        /// Structured primary failure detail.
        error_detail: ProcessErrorDetail,
    },
}

/// Read-only reportable run body.
#[derive(Clone, Copy, Debug)]
pub struct ReportableRunBodyView<'a> {
    pub(crate) fetch: &'a RunFetchSection,
    pub(crate) extraction: &'a RunExtractionSection,
    pub(crate) compare: &'a RunCompareSection,
    pub(crate) change: &'a RunChangeSection,
    pub(crate) previous_compare_digest_sha256: Option<&'a str>,
    pub(crate) current_compare_digest_sha256: &'a str,
}

/// Read-only failed-run view.
#[derive(Clone, Copy, Debug)]
pub struct FailedRunReportView<'a> {
    pub(crate) report: &'a RunReport,
}

/// Read-only successful-run view.
#[derive(Clone, Copy, Debug)]
pub struct SuccessfulRunReportView<'a> {
    pub(crate) report: &'a RunReport,
}

/// Coherent read-only view over the available run body sections.
#[derive(Clone, Copy, Debug)]
pub enum RunBodyView<'a> {
    /// No fetch or extraction work completed.
    None,
    /// Fetch completed but extraction did not.
    Fetch {
        /// Completed fetch section.
        fetch: RunFetchView<'a>,
    },
    /// Fetch and extraction completed but compare did not.
    FetchAndExtraction {
        /// Completed fetch section.
        fetch: RunFetchView<'a>,
        /// Completed extraction section.
        extraction: &'a RunExtractionSection,
    },
    /// Fetch, extraction, and compare completed but change classification did not.
    FetchExtractionCompare {
        /// Completed fetch section.
        fetch: RunFetchView<'a>,
        /// Completed extraction section.
        extraction: &'a RunExtractionSection,
        /// Completed compare section.
        compare: RunCompareView<'a>,
    },
    /// FFHN produced the full reportable body including change classification.
    Reportable(ReportableRunBodyView<'a>),
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
    /// Parsed display name when FFHN could trust the target document.
    pub(crate) display_name: Option<String>,
    /// Run start time.
    pub(crate) run_started_at: String,
    /// Run finish time.
    pub(crate) run_finished_at: String,
    /// Execution mode.
    pub(crate) run_mode: RunMode,
    /// Structured run result.
    pub(crate) result: RunResult,
    /// Compare basis.
    pub(crate) compare_basis: CompareBasis,
    /// Previous compare digest.
    pub(crate) previous_compare_digest_sha256: Option<String>,
    /// Current compare digest.
    pub(crate) current_compare_digest_sha256: Option<String>,
    /// Durable baseline phase before the run.
    pub(crate) baseline_phase_before_run: BaselinePhase,
    /// Durable baseline phase after the run.
    pub(crate) baseline_phase_after_run: BaselinePhase,
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
    /// result details, extraction details, change summary, or notification deliveries do not
    /// match FFHN's frozen run-report contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_run_report_identity(&self.schema_name, self.schema_version)?;
        validate_sha256(&self.run_report_digest_sha256)?;
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
        validate_optional_non_empty("run_report.display_name", self.display_name.as_deref())?;
        self.result.validate()?;
        self.validate_display_name_contract()?;
        validate_lifecycle_contract(self)?;
        validate_persist_contract(self)?;
        if let Some(previous) = &self.previous_compare_digest_sha256 {
            validate_sha256(previous)?;
        }
        if let Some(current) = &self.current_compare_digest_sha256 {
            validate_sha256(current)?;
        }
        if (self.compare.is_some() || self.change.is_some())
            && self.current_compare_digest_sha256.is_none()
        {
            return Err(CoreError::contract(
                "run_report.compare and run_report.change require current_compare_digest_sha256",
            ));
        }
        match self.result.outcome() {
            RunOutcome::Initialized | RunOutcome::Changed | RunOutcome::Unchanged => {
                if self.current_compare_digest_sha256.is_none() {
                    return Err(CoreError::contract(
                        "successful outcomes require current_compare_digest_sha256",
                    ));
                }
                if self.fetch.is_none() || self.extraction.is_none() || self.compare.is_none() {
                    return Err(CoreError::contract(
                        "successful outcomes require fetch, extraction, and compare sections",
                    ));
                }
            }
            RunOutcome::SkippedDisabled => {
                if self.fetch.is_some()
                    || self.extraction.is_some()
                    || self.compare.is_some()
                    || self.current_compare_digest_sha256.is_some()
                {
                    return Err(CoreError::contract(
                        "skipped_disabled must not carry fetch, extraction, compare, or current digests",
                    ));
                }
            }
            RunOutcome::FailedTransient | RunOutcome::FailedPermanent => {
                if self.current_compare_digest_sha256.is_some()
                    && self.failure_cause() != Some(RunFailureCause::PersistError)
                {
                    return Err(CoreError::contract(
                        "failed runs must not carry current_compare_digest_sha256 unless cause = persist_error",
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

    fn validate_display_name_contract(&self) -> Result<(), CoreError> {
        match (self.display_name.as_deref(), self.failure_cause()) {
            (None, Some(RunFailureCause::ConfigInvalid | RunFailureCause::TargetUnavailable)) => {
                Ok(())
            }
            (
                Some(_),
                Some(RunFailureCause::ConfigInvalid | RunFailureCause::TargetUnavailable),
            ) => Err(CoreError::contract(
                "config_invalid and target_unavailable reports must not carry display_name",
            )),
            (Some(_), _) => Ok(()),
            (None, _) => Err(CoreError::contract(
                "run reports must carry display_name when FFHN trusted the target document",
            )),
        }
    }
}

impl RunResult {
    /// Returns the stable run outcome represented by this result.
    pub const fn outcome(&self) -> RunOutcome {
        match self {
            Self::Initialized => RunOutcome::Initialized,
            Self::Changed => RunOutcome::Changed,
            Self::Unchanged => RunOutcome::Unchanged,
            Self::SkippedDisabled => RunOutcome::SkippedDisabled,
            Self::FailedTransient { .. } => RunOutcome::FailedTransient,
            Self::FailedPermanent { .. } => RunOutcome::FailedPermanent,
        }
    }

    /// Returns the failure class when one exists.
    pub const fn failure_class(&self) -> Option<FailureClass> {
        match self {
            Self::FailedTransient { .. } => Some(FailureClass::Transient),
            Self::FailedPermanent { .. } => Some(FailureClass::Permanent),
            Self::Initialized | Self::Changed | Self::Unchanged | Self::SkippedDisabled => None,
        }
    }

    /// Returns the structured primary failure detail when one exists.
    pub const fn error_detail(&self) -> Option<&ProcessErrorDetail> {
        match self {
            Self::FailedTransient { error_detail, .. }
            | Self::FailedPermanent { error_detail, .. } => Some(error_detail),
            Self::Initialized | Self::Changed | Self::Unchanged | Self::SkippedDisabled => None,
        }
    }

    /// Returns the run-failure cause when the run failed.
    pub const fn failure_cause(&self) -> Option<RunFailureCause> {
        match self {
            Self::FailedTransient { cause, .. } | Self::FailedPermanent { cause, .. } => {
                Some(*cause)
            }
            Self::Initialized | Self::Changed | Self::Unchanged | Self::SkippedDisabled => None,
        }
    }

    fn validate(&self) -> Result<(), CoreError> {
        match self {
            Self::FailedTransient {
                cause,
                error_detail,
            } => {
                error_detail.validate()?;
                if cause.failure_class() != FailureClass::Transient {
                    return Err(CoreError::contract(
                        "failed_transient results require a transient cause",
                    ));
                }
            }
            Self::FailedPermanent {
                cause,
                error_detail,
            } => {
                error_detail.validate()?;
                if cause.failure_class() != FailureClass::Permanent {
                    return Err(CoreError::contract(
                        "failed_permanent results require a permanent cause",
                    ));
                }
            }
            Self::Initialized | Self::Changed | Self::Unchanged | Self::SkippedDisabled => {}
        }
        Ok(())
    }
}

fn validate_persist_contract(report: &RunReport) -> Result<(), CoreError> {
    validate_persist_write_status(&report.persist.state_commit)?;
    validate_persist_write_status(&report.persist.last_run_write)?;
    validate_persist_write_duration(
        "run_report.persist.state_commit_duration_ms",
        report.persist.state_commit_duration_ms,
        &report.persist.state_commit,
    )?;
    validate_persist_write_duration(
        "run_report.persist.last_run_write_duration_ms",
        report.persist.last_run_write_duration_ms,
        &report.persist.last_run_write,
    )?;

    if report.run_mode == RunMode::DryRun {
        if !report.persist.state_commit.is_not_attempted() {
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

    let has_state_commit_failure = report.persist.state_commit.is_failed();
    if has_state_commit_failure
        && (report.result.outcome() != RunOutcome::FailedTransient
            || report.failure_cause() != Some(RunFailureCause::PersistError))
    {
        return Err(CoreError::contract(
            "failed state commits require failed_transient with cause = persist_error",
        ));
    }

    if !has_state_commit_failure && report.failure_cause() == Some(RunFailureCause::PersistError) {
        return Err(CoreError::contract(
            "cause = persist_error requires a failed state commit",
        ));
    }

    Ok(())
}

fn validate_persist_write_duration(
    field: &str,
    duration_ms: u64,
    status: &PersistWriteStatus,
) -> Result<(), CoreError> {
    if status.is_not_attempted() && duration_ms != 0 {
        return Err(CoreError::contract(format!(
            "{field} must be zero when the persist step was not attempted",
        )));
    }
    Ok(())
}

fn validate_lifecycle_contract(report: &RunReport) -> Result<(), CoreError> {
    if report.run_mode == RunMode::DryRun
        && report.baseline_phase_before_run != report.baseline_phase_after_run
    {
        return Err(CoreError::contract(
            "dry_run reports must not change the durable baseline phase",
        ));
    }

    match report.result.outcome() {
        RunOutcome::Initialized | RunOutcome::Changed | RunOutcome::Unchanged
            if report.run_mode == RunMode::Live =>
        {
            if report.baseline_phase_after_run != BaselinePhase::HasBaseline {
                return Err(CoreError::contract(
                    "successful live runs require baseline_phase_after_run = has_baseline",
                ));
            }
        }
        RunOutcome::SkippedDisabled => {
            if report.baseline_phase_before_run != report.baseline_phase_after_run {
                return Err(CoreError::contract(
                    "skipped_disabled must not change the durable baseline phase",
                ));
            }
        }
        RunOutcome::Initialized
        | RunOutcome::Changed
        | RunOutcome::Unchanged
        | RunOutcome::FailedTransient
        | RunOutcome::FailedPermanent => {}
    }

    if report.result.outcome() == RunOutcome::Initialized
        && report.run_mode == RunMode::Live
        && report.baseline_phase_before_run != BaselinePhase::NeverSucceeded
    {
        return Err(CoreError::contract(
            "live initialized runs require baseline_phase_before_run = never_succeeded",
        ));
    }

    if matches!(
        report.result.outcome(),
        RunOutcome::Changed | RunOutcome::Unchanged
    ) && report.run_mode == RunMode::Live
        && report.baseline_phase_before_run != BaselinePhase::HasBaseline
    {
        return Err(CoreError::contract(
            "live changed and unchanged runs require baseline_phase_before_run = has_baseline",
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

fn validate_optional_non_empty(field: &str, value: Option<&str>) -> Result<(), CoreError> {
    if let Some(value) = value {
        require_non_empty(field, value)?;
    }
    Ok(())
}

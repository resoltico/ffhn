use url::Url;

use crate::TargetId;
use crate::stable_json::stable_digest_omitting_field;

use super::checks::{
    validate_notification_delivery, validate_run_change_section, validate_run_report_identity,
};
use super::notification::{ProcessErrorDetail, RunNotificationDelivery};
use super::*;

/// Fetch subsection inside `ffhn.run_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunFetchSection {
    /// Fetch engine used for the run.
    pub engine: FetchEngine,
    /// Final URL after redirects when known.
    pub final_url: Option<String>,
    /// HTTP status when known.
    pub http_status: Option<u16>,
    /// Response content type when known.
    pub content_type: Option<String>,
    /// Bytes actually read when known.
    pub bytes_read: Option<usize>,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
}

/// Extraction subsection inside `ffhn.run_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunExtractionSection {
    /// Interop profile.
    pub interop_profile: String,
    /// HTMLCut plan digest.
    pub htmlcut_plan_digest_sha256: String,
    /// HTMLCut result digest.
    pub htmlcut_result_digest_sha256: String,
    /// Comparison-input digest.
    pub comparison_input_sha256: String,
    /// Outer-HTML digest.
    pub outer_html_sha256: String,
    /// Strategy kind.
    pub strategy_kind: SelectionKind,
    /// Selection mode.
    pub selection_mode: SelectionMatch,
    /// Output kind.
    pub output_kind: OutputKind,
    /// Candidate count.
    pub candidate_count: usize,
    /// Selected candidate index.
    pub selected_candidate_index: usize,
    /// Warning codes only.
    pub warning_codes: Vec<String>,
    /// Extraction-stage duration.
    pub duration_ms: u64,
}

/// Compare subsection inside `ffhn.run_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunCompareSection {
    /// Canonicalizer kinds applied in order.
    pub canonicalizers: Vec<String>,
    /// Compare-stage duration.
    pub duration_ms: u64,
}

/// Persist subsection inside `ffhn.run_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunPersistSection {
    /// Persist-stage duration.
    pub duration_ms: u64,
    /// Whether state.json was written.
    pub wrote_state: bool,
    /// Whether last_run.json was written.
    pub wrote_last_run: bool,
    /// Structured write failure detail when a live persist step failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ProcessErrorDetail>,
}

/// One changed region summary inside `ffhn.run_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunChangeRegion {
    /// One-based start line in the previous canonical text.
    pub previous_start_line: usize,
    /// Number of previous lines in the changed region.
    pub previous_line_count: usize,
    /// One-based start line in the current canonical text.
    pub current_start_line: usize,
    /// Number of current lines in the changed region.
    pub current_line_count: usize,
    /// Compact excerpt from the previous canonical text when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_excerpt: Option<String>,
    /// Compact excerpt from the current canonical text when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_excerpt: Option<String>,
    /// Digest of the previous excerpt when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_excerpt_sha256: Option<String>,
    /// Digest of the current excerpt when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_excerpt_sha256: Option<String>,
}

/// Machine-usable change summary inside `ffhn.run_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunChangeSection {
    /// Change discriminator.
    pub kind: ChangeKind,
    /// Previous canonical-text byte length.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_text_bytes: Option<usize>,
    /// Current canonical-text byte length.
    pub current_text_bytes: usize,
    /// Previous canonical-text line count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_line_count: Option<usize>,
    /// Current canonical-text line count.
    pub current_line_count: usize,
    /// Number of equal leading lines.
    pub common_prefix_lines: usize,
    /// Number of equal trailing lines.
    pub common_suffix_lines: usize,
    /// Replaced line region when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_region: Option<RunChangeRegion>,
}

/// `ffhn.run_report` schema.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "RawRunReport")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) failure_class: Option<FailureClass>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) change: Option<RunChangeSection>,
    /// Persist subsection.
    pub(crate) persist: RunPersistSection,
    /// Best-effort notification deliveries attempted by the run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) notifications: Vec<RunNotificationDelivery>,
    /// Reserved extensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) extensions: Extensions,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunReport {
    schema_name: String,
    schema_version: u32,
    run_report_digest_sha256: String,
    target_id: String,
    run_started_at: String,
    run_finished_at: String,
    run_mode: RunMode,
    run_outcome: RunOutcome,
    reason_code: ReasonCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_class: Option<FailureClass>,
    target_status_after_run: TargetStatus,
    compare_basis: CompareBasis,
    previous_compare_digest_sha256: Option<String>,
    current_compare_digest_sha256: Option<String>,
    state_phase_before_run: StatePhase,
    state_phase_after_run: StatePhase,
    fetch: Option<RunFetchSection>,
    extraction: Option<RunExtractionSection>,
    compare: Option<RunCompareSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    change: Option<RunChangeSection>,
    persist: RunPersistSection,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notifications: Vec<RunNotificationDelivery>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
}

impl RunReport {
    /// Returns the run mode.
    pub fn run_mode(&self) -> RunMode {
        self.run_mode
    }

    /// Returns the run outcome.
    pub fn run_outcome(&self) -> RunOutcome {
        self.run_outcome
    }

    /// Returns the reason code.
    pub fn reason_code(&self) -> ReasonCode {
        self.reason_code
    }

    /// Returns the persisted target id string.
    pub fn target_id(&self) -> &str {
        self.target_id.as_str()
    }

    /// Returns the persist subsection.
    pub fn persist(&self) -> &RunPersistSection {
        &self.persist
    }

    /// Returns the attempted notification deliveries.
    pub fn notifications(&self) -> &[RunNotificationDelivery] {
        &self.notifications
    }

    /// Computes and stores the report digest.
    pub fn with_digest(mut self) -> Result<Self, CoreError> {
        self.run_report_digest_sha256 =
            stable_digest_omitting_field(&self, "run_report_digest_sha256")?;
        Ok(self)
    }

    /// Validates one run report.
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
        if self.run_mode == RunMode::DryRun
            && (self.persist.wrote_state
                || self.persist.wrote_last_run
                || self.persist.error.is_some()
                || !self.notifications.is_empty())
        {
            return Err(CoreError::contract(
                "dry_run reports must not persist state or emit notifications",
            ));
        }
        if let Some(error) = &self.persist.error {
            error.validate()?;
            if self.persist.wrote_state && self.persist.wrote_last_run {
                return Err(CoreError::contract(
                    "run_report.persist.error requires at least one failed persist write",
                ));
            }
        }
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
        ) && self.failure_class.is_some()
        {
            return Err(CoreError::contract(
                "successful outcomes must not carry failure_class",
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
                || self.current_compare_digest_sha256.is_some())
        {
            return Err(CoreError::contract(
                "skipped_disabled must not carry fetch, extraction, compare, or current digests",
            ));
        }
        if matches!(
            self.run_outcome,
            RunOutcome::FailedTransient | RunOutcome::FailedPermanent | RunOutcome::SkippedDisabled
        ) && self.current_compare_digest_sha256.is_some()
        {
            return Err(CoreError::contract(
                "failed or skipped runs must not carry current_compare_digest_sha256",
            ));
        }
        match self.run_outcome {
            RunOutcome::FailedTransient => {
                if self.failure_class != Some(FailureClass::Transient)
                    || self.reason_code.failure_class() != Some(FailureClass::Transient)
                {
                    return Err(CoreError::contract(
                        "failed_transient requires a transient reason_code and failure_class",
                    ));
                }
            }
            RunOutcome::FailedPermanent => {
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

impl TryFrom<RawRunReport> for RunReport {
    type Error = CoreError;

    fn try_from(raw: RawRunReport) -> Result<Self, Self::Error> {
        let report = Self {
            schema_name: raw.schema_name,
            schema_version: raw.schema_version,
            run_report_digest_sha256: raw.run_report_digest_sha256,
            target_id: raw.target_id.try_into()?,
            run_started_at: raw.run_started_at,
            run_finished_at: raw.run_finished_at,
            run_mode: raw.run_mode,
            run_outcome: raw.run_outcome,
            reason_code: raw.reason_code,
            failure_class: raw.failure_class,
            target_status_after_run: raw.target_status_after_run,
            compare_basis: raw.compare_basis,
            previous_compare_digest_sha256: raw.previous_compare_digest_sha256,
            current_compare_digest_sha256: raw.current_compare_digest_sha256,
            state_phase_before_run: raw.state_phase_before_run,
            state_phase_after_run: raw.state_phase_after_run,
            fetch: raw.fetch,
            extraction: raw.extraction,
            compare: raw.compare,
            change: raw.change,
            persist: raw.persist,
            notifications: raw.notifications,
            extensions: raw.extensions,
        };
        report.validate()?;
        Ok(report)
    }
}

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::CoreError;
use crate::stable_json::stable_digest_omitting_field;

use super::schema::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, HTMLCUT_INTEROP_PROFILE,
    RUN_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_VERSION, STATUS_REPORT_SCHEMA_NAME,
    STATUS_REPORT_SCHEMA_VERSION,
};
use super::validate::{
    require_non_empty, validate_identity, validate_sha256, validate_target_id, validate_timestamp,
};
use super::{
    ChangeKind, CompareBasis, Extensions, FailureClass, FetchEngine, NotificationEvent, OutputKind,
    ReasonCode, RunMode, RunOutcome, SelectionKind, SelectionMatch, StatePhase, TargetStatus,
};

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

/// Best-effort notification delivery result inside `ffhn.run_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunNotificationDelivery {
    /// Hook name from `target.toml`.
    pub hook_name: String,
    /// Notification event that triggered the hook.
    pub event: NotificationEvent,
    /// Whether the hook process exited successfully.
    pub delivered: bool,
    /// Whether the hook timed out.
    pub timed_out: bool,
    /// Exit status when available.
    pub exit_code: Option<i32>,
    /// Delivery duration in milliseconds.
    pub duration_ms: u64,
    /// Best-effort error detail for failures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `ffhn.run_report` schema.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunReport {
    /// Frozen schema identity.
    pub schema_name: String,
    /// Frozen schema version.
    pub schema_version: u32,
    /// Digest of this report with the field omitted.
    pub run_report_digest_sha256: String,
    /// Target id.
    pub target_id: String,
    /// Run start time.
    pub run_started_at: String,
    /// Run finish time.
    pub run_finished_at: String,
    /// Execution mode.
    pub run_mode: RunMode,
    /// Run outcome.
    pub run_outcome: RunOutcome,
    /// Reason code.
    pub reason_code: ReasonCode,
    /// Failure classification when the run failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<FailureClass>,
    /// Target status after the run.
    pub target_status_after_run: TargetStatus,
    /// Compare basis.
    pub compare_basis: CompareBasis,
    /// Previous compare digest.
    pub previous_compare_digest_sha256: Option<String>,
    /// Current compare digest.
    pub current_compare_digest_sha256: Option<String>,
    /// State phase before the run.
    pub state_phase_before_run: StatePhase,
    /// State phase after the run.
    pub state_phase_after_run: StatePhase,
    /// Fetch subsection.
    pub fetch: Option<RunFetchSection>,
    /// Extraction subsection.
    pub extraction: Option<RunExtractionSection>,
    /// Compare subsection.
    pub compare: Option<RunCompareSection>,
    /// Machine-usable change summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<RunChangeSection>,
    /// Persist subsection.
    pub persist: RunPersistSection,
    /// Best-effort notification deliveries attempted by the run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notifications: Vec<RunNotificationDelivery>,
    /// Reserved extensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Extensions,
}

impl RunReport {
    /// Computes and stores the report digest.
    pub fn with_digest(mut self) -> Result<Self, CoreError> {
        self.run_report_digest_sha256 =
            stable_digest_omitting_field(&self, "run_report_digest_sha256")?;
        Ok(self)
    }

    /// Validates one run report.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_run_report_identity(&self.schema_name, self.schema_version)?;
        validate_target_id(&self.target_id)?;
        validate_sha256(&self.run_report_digest_sha256)?;
        let expected_digest = stable_digest_omitting_field(self, "run_report_digest_sha256")?;
        if expected_digest != self.run_report_digest_sha256 {
            return Err(CoreError::htmlcut(
                "run_report_digest_sha256 does not match the report body",
            ));
        }
        validate_timestamp(&self.run_started_at)?;
        validate_timestamp(&self.run_finished_at)?;
        if self.run_mode == RunMode::DryRun
            && (self.persist.wrote_state
                || self.persist.wrote_last_run
                || !self.notifications.is_empty())
        {
            return Err(CoreError::htmlcut(
                "dry_run reports must not persist state or emit notifications",
            ));
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
            return Err(CoreError::htmlcut(
                "successful outcomes require reason_code = ok",
            ));
        }
        if matches!(
            self.run_outcome,
            RunOutcome::Initialized | RunOutcome::Changed | RunOutcome::Unchanged
        ) && self.failure_class.is_some()
        {
            return Err(CoreError::htmlcut(
                "successful outcomes must not carry failure_class",
            ));
        }
        if matches!(
            self.run_outcome,
            RunOutcome::Initialized | RunOutcome::Changed | RunOutcome::Unchanged
        ) && self.current_compare_digest_sha256.is_none()
        {
            return Err(CoreError::htmlcut(
                "successful outcomes require current_compare_digest_sha256",
            ));
        }
        if matches!(
            self.run_outcome,
            RunOutcome::Initialized | RunOutcome::Changed | RunOutcome::Unchanged
        ) && (self.fetch.is_none() || self.extraction.is_none() || self.compare.is_none())
        {
            return Err(CoreError::htmlcut(
                "successful outcomes require fetch, extraction, and compare sections",
            ));
        }
        if self.run_outcome == RunOutcome::SkippedDisabled
            && self.reason_code != ReasonCode::Disabled
        {
            return Err(CoreError::htmlcut(
                "skipped_disabled requires reason_code = disabled",
            ));
        }
        if self.run_outcome == RunOutcome::SkippedDisabled
            && (self.fetch.is_some()
                || self.extraction.is_some()
                || self.compare.is_some()
                || self.current_compare_digest_sha256.is_some())
        {
            return Err(CoreError::htmlcut(
                "skipped_disabled must not carry fetch, extraction, compare, or current digests",
            ));
        }
        if matches!(
            self.run_outcome,
            RunOutcome::FailedTransient | RunOutcome::FailedPermanent | RunOutcome::SkippedDisabled
        ) && self.current_compare_digest_sha256.is_some()
        {
            return Err(CoreError::htmlcut(
                "failed or skipped runs must not carry current_compare_digest_sha256",
            ));
        }
        match self.run_outcome {
            RunOutcome::FailedTransient => {
                if self.failure_class != Some(FailureClass::Transient)
                    || self.reason_code.failure_class() != Some(FailureClass::Transient)
                {
                    return Err(CoreError::htmlcut(
                        "failed_transient requires a transient reason_code and failure_class",
                    ));
                }
            }
            RunOutcome::FailedPermanent => {
                if self.failure_class != Some(FailureClass::Permanent)
                    || self.reason_code.failure_class() != Some(FailureClass::Permanent)
                {
                    return Err(CoreError::htmlcut(
                        "failed_permanent requires a permanent reason_code and failure_class",
                    ));
                }
            }
            RunOutcome::Initialized | RunOutcome::Changed | RunOutcome::Unchanged => {}
            RunOutcome::SkippedDisabled => {
                if self.failure_class.is_some() {
                    return Err(CoreError::htmlcut(
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
                return Err(CoreError::htmlcut(
                    "run_report.extraction.interop_profile must match the FFHN HTMLCut profile",
                ));
            }
            validate_sha256(&extraction.htmlcut_plan_digest_sha256)?;
            validate_sha256(&extraction.htmlcut_result_digest_sha256)?;
            validate_sha256(&extraction.comparison_input_sha256)?;
            validate_sha256(&extraction.outer_html_sha256)?;
            if extraction.candidate_count == 0 || extraction.selected_candidate_index == 0 {
                return Err(CoreError::htmlcut(
                    "run_report.extraction candidate counts must be positive",
                ));
            }
            if extraction.selected_candidate_index > extraction.candidate_count {
                return Err(CoreError::htmlcut(
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

/// Aggregate outcome counts inside `ffhn.batch_run_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BatchOutcomeCounts {
    /// Initialized runs.
    pub initialized: usize,
    /// Changed runs.
    pub changed: usize,
    /// Unchanged runs.
    pub unchanged: usize,
    /// Transient failures.
    pub failed_transient: usize,
    /// Permanent failures.
    pub failed_permanent: usize,
    /// Disabled skips.
    pub skipped_disabled: usize,
    /// Fatal process-level per-target failures.
    pub fatal_error: usize,
}

/// One target result inside `ffhn.batch_run_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BatchRunEntry {
    /// Target id requested for the entry.
    pub target_id: String,
    /// Structured run report when FFHN could emit one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_report: Option<RunReport>,
    /// Fatal process-level error when FFHN could not emit a run report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fatal_error: Option<String>,
}

/// Aggregate batch report emitted by multi-target runs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BatchRunReport {
    /// Frozen schema identity.
    pub schema_name: String,
    /// Frozen schema version.
    pub schema_version: u32,
    /// Execution mode applied to every target.
    pub run_mode: RunMode,
    /// Watch-root path used for the batch.
    pub watch_root: String,
    /// Targets requested for the batch, in requested order.
    pub requested_targets: Vec<String>,
    /// Batch start time.
    pub run_started_at: String,
    /// Batch finish time.
    pub run_finished_at: String,
    /// Maximum concurrent target runs.
    pub max_concurrency: usize,
    /// Per-target results in stable output order.
    pub entries: Vec<BatchRunEntry>,
    /// Aggregate counts by outcome class.
    pub outcome_counts: BatchOutcomeCounts,
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

impl BatchRunReport {
    /// Validates one batch run report.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_batch_run_report_identity(&self.schema_name, self.schema_version)?;
        require_non_empty("batch.watch_root", &self.watch_root)?;
        validate_timestamp(&self.run_started_at)?;
        validate_timestamp(&self.run_finished_at)?;
        validate_batch_request_contract(&self.requested_targets, self.max_concurrency)?;
        if self.entries.len() != self.requested_targets.len() {
            return Err(CoreError::htmlcut(
                "batch entries must align with requested_targets",
            ));
        }
        let mut counts = BatchOutcomeCounts {
            initialized: 0,
            changed: 0,
            unchanged: 0,
            failed_transient: 0,
            failed_permanent: 0,
            skipped_disabled: 0,
            fatal_error: 0,
        };
        for entry in &self.entries {
            validate_target_id(&entry.target_id)?;
            match (&entry.run_report, &entry.fatal_error) {
                (Some(report), None) => {
                    report.validate()?;
                    if report.target_id != entry.target_id {
                        return Err(CoreError::htmlcut(
                            "batch entry target_id must match run_report.target_id",
                        ));
                    }
                    match report.run_outcome {
                        RunOutcome::Initialized => counts.initialized += 1,
                        RunOutcome::Changed => counts.changed += 1,
                        RunOutcome::Unchanged => counts.unchanged += 1,
                        RunOutcome::FailedTransient => counts.failed_transient += 1,
                        RunOutcome::FailedPermanent => counts.failed_permanent += 1,
                        RunOutcome::SkippedDisabled => counts.skipped_disabled += 1,
                    }
                }
                (None, Some(error)) => {
                    require_non_empty("batch.fatal_error", error)?;
                    counts.fatal_error += 1;
                }
                _ => {
                    return Err(CoreError::htmlcut(
                        "batch entries must carry exactly one of run_report or fatal_error",
                    ));
                }
            }
        }
        if counts != self.outcome_counts {
            return Err(CoreError::htmlcut(
                "batch outcome_counts do not match the batch entries",
            ));
        }
        Ok(())
    }
}

fn validate_run_report_identity(name: &str, version: u32) -> Result<(), CoreError> {
    validate_identity(
        name,
        RUN_REPORT_SCHEMA_NAME,
        version,
        RUN_REPORT_SCHEMA_VERSION,
    )
}

fn validate_status_report_identity(name: &str, version: u32) -> Result<(), CoreError> {
    validate_identity(
        name,
        STATUS_REPORT_SCHEMA_NAME,
        version,
        STATUS_REPORT_SCHEMA_VERSION,
    )
}

fn validate_batch_run_report_identity(name: &str, version: u32) -> Result<(), CoreError> {
    validate_identity(
        name,
        BATCH_RUN_REPORT_SCHEMA_NAME,
        version,
        BATCH_RUN_REPORT_SCHEMA_VERSION,
    )
}

pub(crate) fn validate_batch_request_contract(
    requested_targets: &[String],
    max_concurrency: usize,
) -> Result<(), CoreError> {
    if max_concurrency == 0 {
        return Err(CoreError::htmlcut("batch.max_concurrency must be positive"));
    }

    let mut seen = BTreeSet::new();
    for target_id in requested_targets {
        validate_target_id(target_id)?;
        if !seen.insert(target_id.as_str()) {
            return Err(CoreError::htmlcut(
                "batch.requested_targets values must be unique",
            ));
        }
    }

    Ok(())
}

fn validate_run_change_section(change: &RunChangeSection) -> Result<(), CoreError> {
    if change.current_line_count == 0 && change.current_text_bytes > 0 {
        return Err(CoreError::htmlcut(
            "run_report.change current_line_count must be positive when text exists",
        ));
    }
    if let Some(previous_line_count) = change.previous_line_count
        && change.previous_text_bytes.unwrap_or(0) > 0
        && previous_line_count == 0
    {
        return Err(CoreError::htmlcut(
            "run_report.change previous_line_count must be positive when previous text exists",
        ));
    }
    if let Some(region) = &change.changed_region {
        if matches!(
            (
                region.current_line_count > 0,
                region.current_excerpt.as_ref(),
                region.current_excerpt_sha256.as_ref(),
            ),
            (true, Some(_), None)
        ) {
            return Err(CoreError::htmlcut(
                "run_report.change changed_region current excerpts require a digest",
            ));
        }
        if matches!(
            (
                region.previous_line_count > 0,
                region.previous_excerpt.as_ref(),
                region.previous_excerpt_sha256.as_ref(),
            ),
            (true, Some(_), None)
        ) {
            return Err(CoreError::htmlcut(
                "run_report.change changed_region previous excerpts require a digest",
            ));
        }
        region
            .current_excerpt_sha256
            .as_deref()
            .map(validate_sha256)
            .transpose()?;
        region
            .previous_excerpt_sha256
            .as_deref()
            .map(validate_sha256)
            .transpose()?;
    }
    Ok(())
}

fn validate_notification_delivery(delivery: &RunNotificationDelivery) -> Result<(), CoreError> {
    require_non_empty("notifications.hook_name", &delivery.hook_name)?;
    match (delivery.delivered, delivery.timed_out, delivery.exit_code) {
        (true, true, _) => {
            return Err(CoreError::htmlcut(
                "notifications cannot be both delivered and timed_out",
            ));
        }
        (true, false, Some(0)) | (false, _, _) => {}
        (true, false, _) => {
            return Err(CoreError::htmlcut(
                "delivered notifications must exit with code 0",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

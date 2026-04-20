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
    ChangeKind, CompareBasis, Extensions, FailureClass, FetchEngine, NotificationEvent,
    OutputKind, ReasonCode, RunMode, RunOutcome, SelectionKind, SelectionMatch, StatePhase,
    TargetStatus,
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
            && (self.persist.wrote_state || self.persist.wrote_last_run || !self.notifications.is_empty())
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
        if self.max_concurrency == 0 {
            return Err(CoreError::htmlcut(
                "batch.max_concurrency must be positive",
            ));
        }
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
        if region.current_line_count > 0
            && region.current_excerpt_sha256.is_none()
            && region.current_excerpt.is_some()
        {
            return Err(CoreError::htmlcut(
                "run_report.change changed_region current excerpts require a digest",
            ));
        }
        if region.previous_line_count > 0
            && region.previous_excerpt_sha256.is_none()
            && region.previous_excerpt.is_some()
        {
            return Err(CoreError::htmlcut(
                "run_report.change changed_region previous excerpts require a digest",
            ));
        }
        if let Some(digest) = &region.current_excerpt_sha256 {
            validate_sha256(digest)?;
        }
        if let Some(digest) = &region.previous_excerpt_sha256 {
            validate_sha256(digest)?;
        }
    }
    Ok(())
}

fn validate_notification_delivery(delivery: &RunNotificationDelivery) -> Result<(), CoreError> {
    require_non_empty("notifications.hook_name", &delivery.hook_name)?;
    if delivery.delivered && delivery.timed_out {
        return Err(CoreError::htmlcut(
            "notifications cannot be both delivered and timed_out",
        ));
    }
    if delivery.delivered && delivery.exit_code != Some(0) {
        return Err(CoreError::htmlcut(
            "delivered notifications must exit with code 0",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        HTMLCUT_INTEROP_PROFILE, RUN_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_VERSION,
        STATUS_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_VERSION,
    };

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn valid_run_report() -> RunReport {
        RunReport {
            schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: RUN_REPORT_SCHEMA_VERSION,
            run_report_digest_sha256: String::new(),
            target_id: "demo".to_owned(),
            run_started_at: "2026-04-05T10:15:30Z".to_owned(),
            run_finished_at: "2026-04-05T10:15:31Z".to_owned(),
            run_mode: RunMode::Live,
            run_outcome: RunOutcome::Changed,
            reason_code: ReasonCode::Ok,
            failure_class: None,
            target_status_after_run: TargetStatus::Ready,
            compare_basis: CompareBasis::CanonicalTextSha256,
            previous_compare_digest_sha256: Some(DIGEST.to_owned()),
            current_compare_digest_sha256: Some(DIGEST.to_owned()),
            state_phase_before_run: StatePhase::HasBaseline,
            state_phase_after_run: StatePhase::HasBaseline,
            fetch: Some(RunFetchSection {
                engine: FetchEngine::Http,
                final_url: Some("https://example.com/final".to_owned()),
                http_status: Some(200),
                content_type: Some("text/html".to_owned()),
                bytes_read: Some(42),
                duration_ms: 12,
            }),
            extraction: Some(RunExtractionSection {
                interop_profile: HTMLCUT_INTEROP_PROFILE.to_owned(),
                htmlcut_plan_digest_sha256: DIGEST.to_owned(),
                htmlcut_result_digest_sha256: DIGEST.to_owned(),
                comparison_input_sha256: DIGEST.to_owned(),
                outer_html_sha256: DIGEST.to_owned(),
                strategy_kind: SelectionKind::CssSelector,
                selection_mode: SelectionMatch::Single,
                output_kind: OutputKind::OuterHtml,
                candidate_count: 1,
                selected_candidate_index: 1,
                warning_codes: Vec::new(),
                duration_ms: 8,
            }),
            compare: Some(RunCompareSection {
                canonicalizers: vec!["trim".to_owned()],
                duration_ms: 3,
            }),
            change: Some(RunChangeSection {
                kind: ChangeKind::Changed,
                previous_text_bytes: Some(6),
                current_text_bytes: 7,
                previous_line_count: Some(1),
                current_line_count: 1,
                common_prefix_lines: 0,
                common_suffix_lines: 0,
                changed_region: Some(RunChangeRegion {
                    previous_start_line: 1,
                    previous_line_count: 1,
                    current_start_line: 1,
                    current_line_count: 1,
                    previous_excerpt: Some("Before".to_owned()),
                    current_excerpt: Some("Changed".to_owned()),
                    previous_excerpt_sha256: Some(DIGEST.to_owned()),
                    current_excerpt_sha256: Some(DIGEST.to_owned()),
                }),
            }),
            persist: RunPersistSection {
                duration_ms: 2,
                wrote_state: true,
                wrote_last_run: true,
            },
            notifications: Vec::new(),
            extensions: None,
        }
        .with_digest()
        .expect("digest")
    }

    #[test]
    fn run_report_validation_accepts_a_digest_checked_success_report() {
        valid_run_report().validate().expect("run report");
    }

    #[test]
    fn run_report_validation_rejects_invalid_reason_and_digest_combinations() {
        let report = RunReport {
            reason_code: ReasonCode::Disabled,
            ..valid_run_report()
        }
        .with_digest()
        .expect("disabled digest");
        assert!(report.validate().is_err());

        let report = RunReport {
            run_outcome: RunOutcome::SkippedDisabled,
            reason_code: ReasonCode::Ok,
            ..valid_run_report()
        }
        .with_digest()
        .expect("skipped digest");
        assert!(report.validate().is_err());

        let report = RunReport {
            run_outcome: RunOutcome::FailedPermanent,
            reason_code: ReasonCode::FetchHttpClientError,
            failure_class: Some(FailureClass::Permanent),
            change: None,
            ..valid_run_report()
        }
        .with_digest()
        .expect("failed digest");
        assert!(report.validate().is_err());

        let report = RunReport {
            current_compare_digest_sha256: None,
            ..valid_run_report()
        }
        .with_digest()
        .expect("missing current digest");
        assert!(report.validate().is_err());
    }

    #[test]
    fn run_report_validation_accepts_failed_reports_without_compare_digests() {
        let report = RunReport {
            run_outcome: RunOutcome::FailedPermanent,
            reason_code: ReasonCode::FetchHttpClientError,
            failure_class: Some(FailureClass::Permanent),
            current_compare_digest_sha256: None,
            fetch: None,
            extraction: None,
            compare: None,
            change: None,
            ..valid_run_report()
        }
        .with_digest()
        .expect("failed digest");

        report.validate().expect("failed report");
    }

    #[test]
    fn run_report_validation_accepts_skipped_disabled_reports_with_optional_fields() {
        let report = RunReport {
            run_outcome: RunOutcome::SkippedDisabled,
            reason_code: ReasonCode::Disabled,
            previous_compare_digest_sha256: None,
            current_compare_digest_sha256: None,
            fetch: None,
            extraction: None,
            compare: None,
            ..valid_run_report()
        }
        .with_digest()
        .expect("skipped-disabled digest");

        report.validate().expect("skipped-disabled report");
    }

    #[test]
    fn run_report_validation_accepts_fetch_sections_without_final_url() {
        let mut report = valid_run_report();
        report.fetch.as_mut().expect("fetch").final_url = None;

        report
            .with_digest()
            .expect("missing final-url digest")
            .validate()
            .expect("run report without redirect url");
    }

    #[test]
    fn run_report_validation_rejects_stale_report_digests() {
        let mut report = valid_run_report();
        report.reason_code = ReasonCode::Disabled;

        assert!(report.validate().is_err());
    }

    #[test]
    fn run_report_validation_checks_nested_fetch_and_extraction_fields() {
        let mut report = valid_run_report();
        report.fetch.as_mut().expect("fetch").final_url = Some("not a url".to_owned());
        let report = report.with_digest().expect("fetch digest");
        assert!(report.validate().is_err());

        let report = RunReport {
            fetch: None,
            ..valid_run_report()
        }
        .with_digest()
        .expect("missing fetch digest");
        assert!(report.validate().is_err());

        let report = RunReport {
            extraction: None,
            ..valid_run_report()
        }
        .with_digest()
        .expect("missing extraction digest");
        assert!(report.validate().is_err());

        let report = RunReport {
            compare: None,
            ..valid_run_report()
        }
        .with_digest()
        .expect("missing compare digest");
        assert!(report.validate().is_err());

        let mut report = valid_run_report();
        report
            .extraction
            .as_mut()
            .expect("extraction")
            .interop_profile = "wrong".to_owned();
        let report = report.with_digest().expect("interop digest");
        assert!(report.validate().is_err());

        let mut report = valid_run_report();
        report
            .extraction
            .as_mut()
            .expect("extraction")
            .outer_html_sha256 = "bad".to_owned();
        let report = report.with_digest().expect("extraction digest");
        assert!(report.validate().is_err());

        let mut report = valid_run_report();
        report
            .extraction
            .as_mut()
            .expect("extraction")
            .selected_candidate_index = 0;
        let report = report.with_digest().expect("zero-selected digest");
        assert!(report.validate().is_err());

        let mut report = valid_run_report();
        report
            .extraction
            .as_mut()
            .expect("extraction")
            .selected_candidate_index = 2;
        let report = report.with_digest().expect("candidate digest");
        assert!(report.validate().is_err());

        let mut report = valid_run_report();
        report
            .extraction
            .as_mut()
            .expect("extraction")
            .candidate_count = 0;
        let report = report.with_digest().expect("zero-candidate digest");
        assert!(report.validate().is_err());

        let mut report = valid_run_report();
        report.previous_compare_digest_sha256 = Some("bad".to_owned());
        let report = report.with_digest().expect("previous digest");
        assert!(report.validate().is_err());
    }

    #[test]
    fn run_report_validation_rejects_skipped_disabled_payload_sections() {
        let report = RunReport {
            run_outcome: RunOutcome::SkippedDisabled,
            reason_code: ReasonCode::Disabled,
            previous_compare_digest_sha256: None,
            current_compare_digest_sha256: None,
            fetch: Some(RunFetchSection {
                engine: FetchEngine::Http,
                final_url: None,
                http_status: None,
                content_type: None,
                bytes_read: None,
                duration_ms: 1,
            }),
            extraction: None,
            compare: None,
            ..valid_run_report()
        }
        .with_digest()
        .expect("skipped fetch digest");
        assert!(report.validate().is_err());

        let report = RunReport {
            run_outcome: RunOutcome::SkippedDisabled,
            reason_code: ReasonCode::Disabled,
            previous_compare_digest_sha256: None,
            current_compare_digest_sha256: None,
            fetch: None,
            extraction: valid_run_report().extraction,
            compare: None,
            ..valid_run_report()
        }
        .with_digest()
        .expect("skipped extraction digest");
        assert!(report.validate().is_err());

        let report = RunReport {
            run_outcome: RunOutcome::SkippedDisabled,
            reason_code: ReasonCode::Disabled,
            previous_compare_digest_sha256: None,
            current_compare_digest_sha256: None,
            fetch: None,
            extraction: None,
            compare: valid_run_report().compare,
            ..valid_run_report()
        }
        .with_digest()
        .expect("skipped compare digest");
        assert!(report.validate().is_err());

        let report = RunReport {
            run_outcome: RunOutcome::SkippedDisabled,
            reason_code: ReasonCode::Disabled,
            previous_compare_digest_sha256: None,
            current_compare_digest_sha256: Some(DIGEST.to_owned()),
            fetch: None,
            extraction: None,
            compare: None,
            ..valid_run_report()
        }
        .with_digest()
        .expect("skipped current digest");
        assert!(report.validate().is_err());
    }

    #[test]
    fn status_report_validation_enforces_state_phase_rules() {
        StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
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
        .validate()
        .expect("config invalid report");

        let invalid = StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            target_status: TargetStatus::Ready,
            reason_code: ReasonCode::Ok,
            state_phase: None,
            artifacts: ArtifactStatus {
                current_valid: true,
                previous_valid: true,
            },
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        };
        assert!(invalid.validate().is_err());

        let wrong_target_status = StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            target_status: TargetStatus::Ready,
            reason_code: ReasonCode::ConfigInvalid,
            state_phase: None,
            artifacts: ArtifactStatus {
                current_valid: false,
                previous_valid: false,
            },
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        };
        assert!(wrong_target_status.validate().is_err());

        let invalid_identity = StatusReport {
            schema_name: "wrong".to_owned(),
            ..StatusReport {
                schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
                schema_version: STATUS_REPORT_SCHEMA_VERSION,
                target_id: "demo".to_owned(),
                target_status: TargetStatus::Ready,
                reason_code: ReasonCode::Ok,
                state_phase: Some(StatePhase::HasBaseline),
                artifacts: ArtifactStatus {
                    current_valid: true,
                    previous_valid: true,
                },
                current_snapshot: None,
                snapshot_history: Vec::new(),
                extensions: None,
            }
        };
        assert!(invalid_identity.validate().is_err());

        let mut invalid_snapshot = StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            target_status: TargetStatus::Ready,
            reason_code: ReasonCode::Ok,
            state_phase: Some(StatePhase::HasBaseline),
            artifacts: ArtifactStatus {
                current_valid: true,
                previous_valid: true,
            },
            current_snapshot: Some(SnapshotDigestSummary {
                canonical_text_sha256: "bad".to_owned(),
                outer_html_sha256: DIGEST.to_owned(),
                captured_at: "2026-04-05T10:15:30Z".to_owned(),
            }),
            snapshot_history: Vec::new(),
            extensions: None,
        };
        assert!(invalid_snapshot.validate().is_err());

        invalid_snapshot.current_snapshot = Some(SnapshotDigestSummary {
            canonical_text_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            captured_at: "2026-04-05T10:15:30Z".to_owned(),
        });
        invalid_snapshot.snapshot_history = vec![SnapshotDigestSummary {
            canonical_text_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            captured_at: "2026-04-05T10:14:30Z".to_owned(),
        }];
        invalid_snapshot.validate().expect("ready status report");
    }
}

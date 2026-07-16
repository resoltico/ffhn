use htmlcut_core::interop::v1::{ErrorCode, InteropError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{HtmlcutDiagnostic, Observation, OutboxOverflow};

/// Canonical schema name for run reports.
pub const RUN_REPORT_SCHEMA_NAME: &str = "ffhn.run_report";
/// Canonical run-report-schema version.
pub const RUN_REPORT_SCHEMA_VERSION: u32 = 8;
/// Canonical schema name for batch reports.
pub const BATCH_RUN_REPORT_SCHEMA_NAME: &str = "ffhn.batch_run_report";
/// Canonical batch-report-schema version.
pub const BATCH_RUN_REPORT_SCHEMA_VERSION: u32 = 8;
/// Canonical schema name for status reports.
pub const STATUS_REPORT_SCHEMA_NAME: &str = "ffhn.status_report";
/// Canonical status-report-schema version.
pub const STATUS_REPORT_SCHEMA_VERSION: u32 = 7;
/// Canonical schema name for reset reports.
pub const RESET_REPORT_SCHEMA_NAME: &str = "ffhn.reset_report";
/// Canonical reset-report-schema version.
pub const RESET_REPORT_SCHEMA_VERSION: u32 = 3;

/// Execution mode for one run request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// Persist an accepted observation when the run succeeds.
    Live,
    /// Evaluate without altering persistent storage.
    DryRun,
}

/// Measurement result classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    /// First valid observation was accepted.
    Initialized,
    /// A valid observation differs from the prior canonical value.
    Changed,
    /// A valid observation equals the prior canonical value.
    Unchanged,
    /// The target is disabled.
    SkippedDisabled,
    /// Existing state belongs to a different source contract.
    RefusedContractDigest,
    /// Source JSON could not produce one scalar projection.
    AcquisitionFailed,
    /// A selected scalar could not satisfy its declared type.
    ValueUnparseable,
    /// Target configuration is invalid.
    ConfigInvalid,
    /// Target configuration could not be read.
    TargetUnavailable,
    /// State is unreadable or invalid.
    StateInvalid,
    /// The target lock is currently held by another live run.
    LockUnavailable,
    /// Fetch did not produce source text.
    FetchFailed,
    /// State persistence did not commit.
    PersistFailed,
}

/// Structured, stable diagnostic detail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessErrorDetail {
    kind: ProcessErrorKind,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    htmlcut_failure: Option<Box<HtmlcutFailureDetails>>,
}

/// HTMLCut failure evidence retained when an HTML projection rejects a plan or cannot yield one
/// measurement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HtmlcutFailureDetails {
    reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    candidate_count: Option<usize>,
    plan_digest_sha256: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<HtmlcutDiagnostic>,
}

impl HtmlcutFailureDetails {
    pub(crate) fn new(
        reason: String,
        candidate_count: Option<usize>,
        plan_digest_sha256: String,
        diagnostics: Vec<HtmlcutDiagnostic>,
    ) -> Self {
        Self {
            reason,
            candidate_count,
            plan_digest_sha256,
            diagnostics,
        }
    }

    /// Converts one public HTMLCut failure into FFHN's stable report evidence.
    ///
    /// This is the sole boundary that derives a reason or candidate count from HTMLCut error
    /// details. Preflight validation and source acquisition both use it so their evidence cannot
    /// drift apart.
    pub(crate) fn from_interop_error(error: &InteropError) -> Self {
        let reason = error
            .details
            .get("core_diagnostic_code")
            .and_then(Value::as_str)
            .unwrap_or(match error.error_code {
                ErrorCode::PlanInvalid => "plan_invalid",
                ErrorCode::NoMatch => "no_match",
                ErrorCode::AmbiguousMatch => "ambiguous_match",
                ErrorCode::MissingAttribute => "missing_attribute",
                ErrorCode::InternalError => "internal_error",
            })
            .to_owned();
        let candidate_count = error.details.values().find_map(candidate_count_in_value);
        let diagnostics = error
            .diagnostics
            .clone()
            .into_iter()
            .map(HtmlcutDiagnostic::from_interop)
            .collect();
        Self::new(
            reason,
            candidate_count,
            error.plan_digest_sha256.clone(),
            diagnostics,
        )
    }

    /// Returns the stable primary HTMLCut diagnostic reason.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Returns HTMLCut's pre-selection candidate count when the diagnostic supplied one.
    pub const fn candidate_count(&self) -> Option<usize> {
        self.candidate_count
    }

    /// Returns the exact internally structured HTMLCut plan digest.
    pub fn plan_digest_sha256(&self) -> &str {
        &self.plan_digest_sha256
    }

    /// Returns all HTMLCut diagnostics retained for the failed acquisition.
    pub fn diagnostics(&self) -> &[HtmlcutDiagnostic] {
        &self.diagnostics
    }
}

fn candidate_count_in_value(value: &Value) -> Option<usize> {
    match value {
        Value::Object(values) => values
            .get("candidateCount")
            .or_else(|| values.get("candidate_count"))
            .and_then(Value::as_u64)
            .and_then(|count| usize::try_from(count).ok())
            .or_else(|| values.values().find_map(candidate_count_in_value)),
        Value::Array(values) => values.iter().find_map(candidate_count_in_value),
        _ => None,
    }
}

/// Stable diagnostic-category vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessErrorKind {
    /// Contract or configuration violation.
    Contract,
    /// Filesystem or network I/O failure.
    Io,
    /// JSON syntax failure.
    Json,
    /// HTMLCut selection or projection failure.
    Htmlcut,
    /// TOML syntax failure.
    Toml,
    /// Semantic value parsing failure.
    ValueUnparseable,
}

/// The result of one post-commit durable outbox delivery attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    /// The process accepted the stored payload and the pending record was removed.
    Delivered,
    /// The process failed and the record remains pending for its deterministic next retry.
    RetryScheduled,
    /// The process exhausted `max_attempts`; the record was removed as terminal evidence.
    DeadLettered,
    /// The process accepted the payload but FFHN could not persist the record removal.
    DeliveredUncommitted,
    /// The process failed but FFHN could not persist its deterministic retry state.
    RetryUncommitted,
    /// FFHN could not persist the required terminal-record removal.
    DeadLetterUncommitted,
}

/// Immutable evidence of one post-commit outbox delivery attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryOutcome {
    event_id: String,
    route_id: String,
    status: DeliveryStatus,
    attempt_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl DeliveryOutcome {
    pub(crate) fn delivered(event_id: String, route_id: String, attempt_count: u32) -> Self {
        Self {
            event_id,
            route_id,
            status: DeliveryStatus::Delivered,
            attempt_count,
            error: None,
        }
    }

    pub(crate) fn retry_scheduled(
        event_id: String,
        route_id: String,
        attempt_count: u32,
        error: String,
    ) -> Self {
        Self {
            event_id,
            route_id,
            status: DeliveryStatus::RetryScheduled,
            attempt_count,
            error: Some(error),
        }
    }

    pub(crate) fn dead_lettered(
        event_id: String,
        route_id: String,
        attempt_count: u32,
        error: String,
    ) -> Self {
        Self {
            event_id,
            route_id,
            status: DeliveryStatus::DeadLettered,
            attempt_count,
            error: Some(error),
        }
    }

    pub(crate) fn delivered_uncommitted(
        event_id: String,
        route_id: String,
        attempt_count: u32,
        outbox_error: impl std::fmt::Display,
    ) -> Self {
        Self {
            event_id,
            route_id,
            status: DeliveryStatus::DeliveredUncommitted,
            attempt_count,
            error: Some(format!(
                "delivery process succeeded, but FFHN could not persist the pending-record removal: {outbox_error}"
            )),
        }
    }

    pub(crate) fn retry_uncommitted(
        event_id: String,
        route_id: String,
        attempt_count: u32,
        delivery_error: &str,
        outbox_error: impl std::fmt::Display,
    ) -> Self {
        Self {
            event_id,
            route_id,
            status: DeliveryStatus::RetryUncommitted,
            attempt_count,
            error: Some(format!(
                "delivery process failed: {delivery_error}; FFHN could not persist deterministic retry state: {outbox_error}"
            )),
        }
    }

    pub(crate) fn dead_letter_uncommitted(
        event_id: String,
        route_id: String,
        attempt_count: u32,
        delivery_error: impl std::fmt::Display,
        outbox_error: impl std::fmt::Display,
    ) -> Self {
        Self {
            event_id,
            route_id,
            status: DeliveryStatus::DeadLetterUncommitted,
            attempt_count,
            error: Some(format!(
                "{delivery_error}; FFHN could not persist the terminal pending-record removal: {outbox_error}"
            )),
        }
    }

    /// Returns the deterministic event identity.
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    /// Returns the target-local delivery route identifier.
    pub fn route_id(&self) -> &str {
        &self.route_id
    }

    /// Returns the delivery result category.
    pub const fn status(&self) -> DeliveryStatus {
        self.status
    }

    /// Returns the completed attempt count for this record.
    pub const fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    /// Returns the delivery or outbox-persistence failure detail when delivery did not complete.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// Structured run result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunReport {
    schema_name: String,
    schema_version: u32,
    target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    run_mode: RunMode,
    outcome: RunOutcome,
    run_started_at: String,
    run_finished_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    contract_digest_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observation: Option<Observation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_canonical_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_detail: Option<ProcessErrorDetail>,
    state_persisted: bool,
    delivery_outcomes: Vec<DeliveryOutcome>,
    outbox_overflow: Vec<OutboxOverflow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outbox_error: Option<String>,
}

pub(crate) struct RunReportParts {
    pub(crate) target_id: String,
    pub(crate) display_name: Option<String>,
    pub(crate) run_mode: RunMode,
    pub(crate) outcome: RunOutcome,
    pub(crate) started: String,
    pub(crate) finished: String,
    pub(crate) digest: Option<String>,
    pub(crate) observation: Option<Observation>,
    pub(crate) previous: Option<String>,
    pub(crate) error: Option<ProcessErrorDetail>,
    pub(crate) state_persisted: bool,
    pub(crate) delivery_outcomes: Vec<DeliveryOutcome>,
    pub(crate) outbox_overflow: Vec<OutboxOverflow>,
    pub(crate) outbox_error: Option<String>,
}

/// Aggregate report for a multiple-target run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchRunReport {
    schema_name: String,
    schema_version: u32,
    run_mode: RunMode,
    requested_targets: Vec<String>,
    reports: Vec<RunReport>,
}

/// Status classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusKind {
    /// A valid target has no accepted observation yet.
    Pending,
    /// A valid target has accepted state.
    Ready,
    /// Target configuration is invalid.
    InvalidConfig,
    /// Target configuration is unavailable.
    UnavailableTarget,
    /// State is invalid.
    InvalidState,
}

/// Structured target status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusReport {
    schema_name: String,
    schema_version: u32,
    target_id: String,
    kind: StatusKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contract_digest_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    accepted_observation: Option<Observation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_detail: Option<ProcessErrorDetail>,
}

/// Structured outcome of a blind storage reset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetReport {
    schema_name: String,
    schema_version: u32,
    target_id: String,
    storage_cleared: bool,
    delivery_outcomes: Vec<DeliveryOutcome>,
    outbox_overflow: Vec<OutboxOverflow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outbox_error: Option<String>,
}

impl ProcessErrorDetail {
    /// Builds one structured diagnostic detail.
    pub(crate) fn new(
        kind: ProcessErrorKind,
        message: impl Into<String>,
        path: Option<String>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            path,
            htmlcut_failure: None,
        }
    }

    pub(crate) fn with_htmlcut_failure(mut self, failure: HtmlcutFailureDetails) -> Self {
        self.htmlcut_failure = Some(Box::new(failure));
        self
    }
    /// Returns the diagnostic category.
    pub const fn kind(&self) -> ProcessErrorKind {
        self.kind
    }
    /// Returns the message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns retained HTMLCut failure evidence for an HTML acquisition failure.
    pub fn htmlcut_failure(&self) -> Option<&HtmlcutFailureDetails> {
        self.htmlcut_failure.as_deref()
    }
}
impl RunReport {
    pub(crate) fn new(parts: RunReportParts) -> Self {
        Self {
            schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: RUN_REPORT_SCHEMA_VERSION,
            target_id: parts.target_id,
            display_name: parts.display_name,
            run_mode: parts.run_mode,
            outcome: parts.outcome,
            run_started_at: parts.started,
            run_finished_at: parts.finished,
            contract_digest_sha256: parts.digest,
            observation: parts.observation,
            previous_canonical_value: parts.previous,
            error_detail: parts.error,
            state_persisted: parts.state_persisted,
            delivery_outcomes: parts.delivery_outcomes,
            outbox_overflow: parts.outbox_overflow,
            outbox_error: parts.outbox_error,
        }
    }
    /// Returns the target id.
    pub fn target_id(&self) -> &str {
        &self.target_id
    }
    /// Returns the display name when target loading succeeded.
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }
    /// Returns the execution mode.
    pub const fn run_mode(&self) -> RunMode {
        self.run_mode
    }
    /// Returns the measurement outcome.
    pub const fn outcome(&self) -> RunOutcome {
        self.outcome
    }
    /// Returns the typed observation when acquisition and parsing succeeded.
    pub fn observation(&self) -> Option<&Observation> {
        self.observation.as_ref()
    }
    /// Returns any structured diagnostic.
    pub fn error_detail(&self) -> Option<&ProcessErrorDetail> {
        self.error_detail.as_ref()
    }
    /// Returns whether the state write committed.
    pub const fn state_persisted(&self) -> bool {
        self.state_persisted
    }

    /// Returns every post-commit outbox delivery attempt made during this run.
    pub fn delivery_outcomes(&self) -> &[DeliveryOutcome] {
        &self.delivery_outcomes
    }

    /// Returns newly staged records intentionally dropped because the pending queue was full.
    pub fn outbox_overflow(&self) -> &[OutboxOverflow] {
        &self.outbox_overflow
    }

    /// Returns the outbox error that halted post-commit delivery processing, when any.
    pub fn outbox_error(&self) -> Option<&str> {
        self.outbox_error.as_deref()
    }

    /// Returns whether a delivery attempt failed, including terminal dead-lettering.
    pub fn has_delivery_failure(&self) -> bool {
        self.delivery_outcomes
            .iter()
            .any(|outcome| outcome.status != DeliveryStatus::Delivered)
    }

    /// Returns whether delivery failed or a newly staged record was dropped by a full outbox.
    pub fn has_delivery_problem(&self) -> bool {
        self.has_delivery_failure()
            || !self.outbox_overflow.is_empty()
            || self.outbox_error.is_some()
    }
}
impl BatchRunReport {
    pub(crate) fn new(
        run_mode: RunMode,
        requested_targets: Vec<String>,
        reports: Vec<RunReport>,
    ) -> Self {
        Self {
            schema_name: BATCH_RUN_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: BATCH_RUN_REPORT_SCHEMA_VERSION,
            run_mode,
            requested_targets,
            reports,
        }
    }
    /// Returns the reports in requested-target order.
    pub fn reports(&self) -> &[RunReport] {
        &self.reports
    }
}
impl StatusReport {
    pub(crate) fn new(
        target_id: impl Into<String>,
        kind: StatusKind,
        display_name: Option<String>,
        enabled: Option<bool>,
        digest: Option<String>,
        observation: Option<Observation>,
        error: Option<ProcessErrorDetail>,
    ) -> Self {
        Self {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target_id.into(),
            kind,
            display_name,
            enabled,
            contract_digest_sha256: digest,
            accepted_observation: observation,
            error_detail: error,
        }
    }
    /// Returns the status classification.
    pub const fn kind(&self) -> StatusKind {
        self.kind
    }
    /// Returns the accepted observation when ready.
    pub fn accepted_observation(&self) -> Option<&Observation> {
        self.accepted_observation.as_ref()
    }
}
impl ResetReport {
    pub(crate) fn new(
        target_id: impl Into<String>,
        storage_cleared: bool,
        delivery_outcomes: Vec<DeliveryOutcome>,
        outbox_overflow: Vec<OutboxOverflow>,
        outbox_error: Option<String>,
    ) -> Self {
        Self {
            schema_name: RESET_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: RESET_REPORT_SCHEMA_VERSION,
            target_id: target_id.into(),
            storage_cleared,
            delivery_outcomes,
            outbox_overflow,
            outbox_error,
        }
    }

    /// Returns whether the target's v2 storage root was removed.
    pub const fn storage_cleared(&self) -> bool {
        self.storage_cleared
    }

    /// Returns every reset-event delivery attempt made after the blind delete committed.
    pub fn delivery_outcomes(&self) -> &[DeliveryOutcome] {
        &self.delivery_outcomes
    }

    /// Returns reset-event records intentionally dropped because the queue was full.
    pub fn outbox_overflow(&self) -> &[OutboxOverflow] {
        &self.outbox_overflow
    }

    /// Returns the outbox error that prevented reset-event delivery processing from completing.
    pub fn outbox_error(&self) -> Option<&str> {
        self.outbox_error.as_deref()
    }

    /// Returns whether reset-event delivery failed or the reset event overflowed the outbox.
    pub fn has_delivery_problem(&self) -> bool {
        self.delivery_outcomes
            .iter()
            .any(|outcome| outcome.status != DeliveryStatus::Delivered)
            || !self.outbox_overflow.is_empty()
            || self.outbox_error.is_some()
    }
}
impl RunMode {
    /// Returns the stable mode token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::DryRun => "dry_run",
        }
    }
}
impl RunOutcome {
    /// Returns the stable outcome token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initialized => "initialized",
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
            Self::SkippedDisabled => "skipped_disabled",
            Self::RefusedContractDigest => "refused_contract_digest",
            Self::AcquisitionFailed => "acquisition_failed",
            Self::ValueUnparseable => "value_unparseable",
            Self::ConfigInvalid => "config_invalid",
            Self::TargetUnavailable => "target_unavailable",
            Self::StateInvalid => "state_invalid",
            Self::LockUnavailable => "lock_unavailable",
            Self::FetchFailed => "fetch_failed",
            Self::PersistFailed => "persist_failed",
        }
    }
}

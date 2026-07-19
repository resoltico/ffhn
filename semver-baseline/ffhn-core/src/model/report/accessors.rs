//! Report construction and public accessors.

use crate::{CoreError, Observation, OutboxOverflow};

use super::delivery::{DeliveryOutcome, DeliveryStatus};
use super::diagnostic::DiagnosticDetail;
use super::lifecycle::{LifecycleFacet, LifecycleSnapshot};
use super::policy::PolicyEvaluation;
use super::records::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, BatchRunReport,
    RESET_REPORT_SCHEMA_NAME, RESET_REPORT_SCHEMA_VERSION, RUN_REPORT_SCHEMA_NAME,
    RUN_REPORT_SCHEMA_VERSION, ResetReport, RunMode, RunOutcome, RunReport, RunReportParts,
    STATUS_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_VERSION, StatusKind, StatusReport,
    StatusReportParts,
};

impl RunReport {
    pub(crate) fn new(parts: RunReportParts) -> Result<Self, crate::CoreError> {
        let report = Self {
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
            policy_evaluation: parts.policy_evaluation,
            lifecycle: LifecycleFacet::new(parts.lifecycle_before, parts.lifecycle_after),
            state_persisted: parts.state_persisted,
            delivery_outcomes: parts.delivery_outcomes,
            outbox_overflow: parts.outbox_overflow,
            outbox_error_detail: parts.outbox_error_detail,
        };
        report.validate_schema()?;
        Ok(report)
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
    /// Returns the canonical value from the accepted observation before this run.
    pub fn previous_canonical_value(&self) -> Option<&str> {
        self.previous_canonical_value.as_deref()
    }
    /// Returns the instant this run began.
    pub fn run_started_at(&self) -> &str {
        &self.run_started_at
    }
    /// Returns the instant this run finished.
    pub fn run_finished_at(&self) -> &str {
        &self.run_finished_at
    }
    /// Returns the target contract digest when target loading succeeded.
    pub fn contract_digest_sha256(&self) -> Option<&str> {
        self.contract_digest_sha256.as_deref()
    }
    /// Returns any structured diagnostic.
    pub fn error_detail(&self) -> Option<&DiagnosticDetail> {
        self.error_detail.as_ref()
    }

    /// Returns the policy staging result for this run.
    pub const fn policy_evaluation(&self) -> &PolicyEvaluation {
        &self.policy_evaluation
    }
    /// Returns the durable-before and staged-after lifecycle facts for this run.
    pub const fn lifecycle(&self) -> &LifecycleFacet {
        &self.lifecycle
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

    /// Returns the structured outbox failure that halted post-commit delivery processing.
    pub fn outbox_error_detail(&self) -> Option<&DiagnosticDetail> {
        self.outbox_error_detail.as_ref()
    }

    /// Returns whether a delivery attempt failed, including terminal dead-lettering.
    pub fn has_delivery_failure(&self) -> bool {
        self.delivery_outcomes.iter().any(|outcome| {
            matches!(
                outcome.status(),
                DeliveryStatus::RetryScheduled
                    | DeliveryStatus::DeadLettered
                    | DeliveryStatus::RetryUncommitted
                    | DeliveryStatus::DeadLetterUncommitted
            )
        })
    }

    /// Returns whether delivery failed or a newly staged record was dropped by a full outbox.
    pub fn has_delivery_problem(&self) -> bool {
        self.has_delivery_failure()
            || !self.outbox_overflow.is_empty()
            || self.outbox_error_detail.is_some()
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
    /// Returns the execution mode shared by all reports in this batch.
    pub const fn run_mode(&self) -> RunMode {
        self.run_mode
    }
    /// Returns target ids in the request's deterministic order.
    pub fn requested_targets(&self) -> &[String] {
        &self.requested_targets
    }
}
impl StatusReport {
    pub(crate) fn new(parts: StatusReportParts) -> Result<Self, CoreError> {
        let report = Self {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: parts.target_id,
            kind: parts.kind,
            display_name: parts.display_name,
            enabled: parts.enabled,
            contract_digest_sha256: parts.digest,
            accepted_observation: parts.observation,
            error_detail: parts.error,
            lifecycle: parts.lifecycle,
        };
        report.validate_schema()?;
        Ok(report)
    }
    /// Returns the status classification.
    pub const fn kind(&self) -> StatusKind {
        self.kind
    }
    /// Returns the requested target identifier.
    pub fn target_id(&self) -> &str {
        &self.target_id
    }
    /// Returns the display name when target loading succeeded.
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }
    /// Returns whether the target is enabled when target loading succeeded.
    pub const fn enabled(&self) -> Option<bool> {
        self.enabled
    }
    /// Returns the target contract digest when target loading succeeded.
    pub fn contract_digest_sha256(&self) -> Option<&str> {
        self.contract_digest_sha256.as_deref()
    }
    /// Returns the accepted observation when ready.
    pub fn accepted_observation(&self) -> Option<&Observation> {
        self.accepted_observation.as_ref()
    }
    /// Returns structured status failure evidence, when any.
    pub fn error_detail(&self) -> Option<&DiagnosticDetail> {
        self.error_detail.as_ref()
    }
    /// Returns the current durable lifecycle when a valid matching state was safely read.
    pub const fn lifecycle(&self) -> Option<&LifecycleSnapshot> {
        self.lifecycle.as_ref()
    }
}
impl ResetReport {
    pub(crate) fn new(
        target_id: impl Into<String>,
        storage_cleared: bool,
        delivery_outcomes: Vec<DeliveryOutcome>,
        outbox_overflow: Vec<OutboxOverflow>,
        outbox_error_detail: Option<DiagnosticDetail>,
    ) -> Self {
        Self {
            schema_name: RESET_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: RESET_REPORT_SCHEMA_VERSION,
            target_id: target_id.into(),
            storage_cleared,
            delivery_outcomes,
            outbox_overflow,
            outbox_error_detail,
        }
    }

    /// Returns whether the target's v2 storage root was removed.
    pub const fn storage_cleared(&self) -> bool {
        self.storage_cleared
    }
    /// Returns the reset target identifier.
    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    /// Returns every reset-event delivery attempt made after the blind delete committed.
    pub fn delivery_outcomes(&self) -> &[DeliveryOutcome] {
        &self.delivery_outcomes
    }

    /// Returns reset-event records intentionally dropped because the queue was full.
    pub fn outbox_overflow(&self) -> &[OutboxOverflow] {
        &self.outbox_overflow
    }

    /// Returns structured outbox evidence that halted reset-event delivery processing.
    pub fn outbox_error_detail(&self) -> Option<&DiagnosticDetail> {
        self.outbox_error_detail.as_ref()
    }

    /// Returns whether reset-event delivery failed or the reset event overflowed the outbox.
    pub fn has_delivery_problem(&self) -> bool {
        self.delivery_outcomes
            .iter()
            .any(|outcome| outcome.status() != DeliveryStatus::Delivered)
            || !self.outbox_overflow.is_empty()
            || self.outbox_error_detail.is_some()
    }
}
impl StatusKind {
    /// Returns the stable report-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::InvalidConfig => "invalid_config",
            Self::UnavailableTarget => "unavailable_target",
            Self::InvalidState => "invalid_state",
        }
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
            Self::IntegrationFault => "integration_fault",
        }
    }
}

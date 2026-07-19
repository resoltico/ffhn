//! Serializable run, batch, status, and reset reports.

use serde::{Deserialize, Deserializer, Serialize};

use crate::{CoreError, Observation, OutboxOverflow};

use super::delivery::DeliveryOutcome;
use super::diagnostic::DiagnosticDetail;
use super::lifecycle::{LifecycleFacet, LifecycleSnapshot};
use super::policy::PolicyEvaluation;

/// Canonical schema name for run reports.
pub const RUN_REPORT_SCHEMA_NAME: &str = "ffhn.run_report";
/// Canonical run-report-schema version.
pub const RUN_REPORT_SCHEMA_VERSION: u32 = 17;
/// Canonical schema name for batch reports.
pub const BATCH_RUN_REPORT_SCHEMA_NAME: &str = "ffhn.batch_run_report";
/// Canonical batch-report-schema version.
pub const BATCH_RUN_REPORT_SCHEMA_VERSION: u32 = 17;
/// Canonical schema name for status reports.
pub const STATUS_REPORT_SCHEMA_NAME: &str = "ffhn.status_report";
/// Canonical status-report-schema version.
pub const STATUS_REPORT_SCHEMA_VERSION: u32 = 13;
/// Canonical schema name for reset reports.
pub const RESET_REPORT_SCHEMA_NAME: &str = "ffhn.reset_report";
/// Canonical reset-report-schema version.
pub const RESET_REPORT_SCHEMA_VERSION: u32 = 7;

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
    /// FFHN or HTMLCut violated the closed adapter-boundary contract.
    IntegrationFault,
}

/// Whether a run-outcome family can carry a staged lifecycle successor.
///
/// This is deliberately private report-schema machinery: the public contract exposes the
/// `lifecycle.after` fact itself, not an additional metadata flag. Keeping the relationship here
/// makes direct report deserialization reject combinations that FFHN can never produce.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LifecycleAfterRule {
    /// This outcome ends before FFHN can stage a state transition.
    Forbidden,
    /// This outcome is produced only after FFHN has staged a state transition.
    Required,
    /// Target validation can either reject before staging or produce a permanent-error episode.
    Optional,
}

impl RunOutcome {
    const fn lifecycle_after_rule(self) -> LifecycleAfterRule {
        match self {
            Self::SkippedDisabled
            | Self::RefusedContractDigest
            | Self::TargetUnavailable
            | Self::StateInvalid
            | Self::LockUnavailable => LifecycleAfterRule::Forbidden,
            Self::Initialized
            | Self::Changed
            | Self::Unchanged
            | Self::AcquisitionFailed
            | Self::ValueUnparseable
            | Self::FetchFailed
            | Self::PersistFailed
            | Self::IntegrationFault => LifecycleAfterRule::Required,
            Self::ConfigInvalid => LifecycleAfterRule::Optional,
        }
    }
}

/// Structured run result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunReport {
    pub(super) schema_name: String,
    pub(super) schema_version: u32,
    pub(super) target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) display_name: Option<String>,
    pub(super) run_mode: RunMode,
    pub(super) outcome: RunOutcome,
    pub(super) run_started_at: String,
    pub(super) run_finished_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) contract_digest_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) observation: Option<Observation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) previous_canonical_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error_detail: Option<DiagnosticDetail>,
    pub(super) policy_evaluation: PolicyEvaluation,
    pub(super) lifecycle: LifecycleFacet,
    pub(super) state_persisted: bool,
    pub(super) delivery_outcomes: Vec<DeliveryOutcome>,
    pub(super) outbox_overflow: Vec<OutboxOverflow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) outbox_error_detail: Option<DiagnosticDetail>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunReportWire {
    schema_name: String,
    schema_version: u32,
    target_id: String,
    #[serde(default)]
    display_name: Option<String>,
    run_mode: RunMode,
    outcome: RunOutcome,
    run_started_at: String,
    run_finished_at: String,
    #[serde(default)]
    contract_digest_sha256: Option<String>,
    #[serde(default)]
    observation: Option<Observation>,
    #[serde(default)]
    previous_canonical_value: Option<String>,
    #[serde(default)]
    error_detail: Option<DiagnosticDetail>,
    policy_evaluation: PolicyEvaluation,
    lifecycle: LifecycleFacet,
    state_persisted: bool,
    delivery_outcomes: Vec<DeliveryOutcome>,
    outbox_overflow: Vec<OutboxOverflow>,
    #[serde(default)]
    outbox_error_detail: Option<DiagnosticDetail>,
}

impl<'de> Deserialize<'de> for RunReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RunReportWire::deserialize(deserializer)?;
        let report = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            target_id: wire.target_id,
            display_name: wire.display_name,
            run_mode: wire.run_mode,
            outcome: wire.outcome,
            run_started_at: wire.run_started_at,
            run_finished_at: wire.run_finished_at,
            contract_digest_sha256: wire.contract_digest_sha256,
            observation: wire.observation,
            previous_canonical_value: wire.previous_canonical_value,
            error_detail: wire.error_detail,
            policy_evaluation: wire.policy_evaluation,
            lifecycle: wire.lifecycle,
            state_persisted: wire.state_persisted,
            delivery_outcomes: wire.delivery_outcomes,
            outbox_overflow: wire.outbox_overflow,
            outbox_error_detail: wire.outbox_error_detail,
        };
        report.validate_schema().map_err(serde::de::Error::custom)?;
        Ok(report)
    }
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
    pub(crate) error: Option<DiagnosticDetail>,
    pub(crate) policy_evaluation: PolicyEvaluation,
    pub(crate) lifecycle_before: Option<LifecycleSnapshot>,
    pub(crate) lifecycle_after: Option<LifecycleSnapshot>,
    pub(crate) state_persisted: bool,
    pub(crate) delivery_outcomes: Vec<DeliveryOutcome>,
    pub(crate) outbox_overflow: Vec<OutboxOverflow>,
    pub(crate) outbox_error_detail: Option<DiagnosticDetail>,
}

/// Aggregate report for a multiple-target run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BatchRunReport {
    pub(super) schema_name: String,
    pub(super) schema_version: u32,
    pub(super) run_mode: RunMode,
    pub(super) requested_targets: Vec<String>,
    pub(super) reports: Vec<RunReport>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchRunReportWire {
    schema_name: String,
    schema_version: u32,
    run_mode: RunMode,
    requested_targets: Vec<String>,
    reports: Vec<RunReport>,
}

impl<'de> Deserialize<'de> for BatchRunReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = BatchRunReportWire::deserialize(deserializer)?;
        let report = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            run_mode: wire.run_mode,
            requested_targets: wire.requested_targets,
            reports: wire.reports,
        };
        report.validate_schema().map_err(serde::de::Error::custom)?;
        Ok(report)
    }
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StatusReport {
    pub(super) schema_name: String,
    pub(super) schema_version: u32,
    pub(super) target_id: String,
    pub(super) kind: StatusKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) contract_digest_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) accepted_observation: Option<Observation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error_detail: Option<DiagnosticDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) lifecycle: Option<LifecycleSnapshot>,
}

/// Complete input for one status report.
///
/// Construction remains crate-private because callers must obtain lifecycle facts through the
/// verified status path. Named fields make every report fact explicit at that boundary.
pub(crate) struct StatusReportParts {
    pub(crate) target_id: String,
    pub(crate) kind: StatusKind,
    pub(crate) display_name: Option<String>,
    pub(crate) enabled: Option<bool>,
    pub(crate) digest: Option<String>,
    pub(crate) observation: Option<Observation>,
    pub(crate) error: Option<DiagnosticDetail>,
    pub(crate) lifecycle: Option<LifecycleSnapshot>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusReportWire {
    schema_name: String,
    schema_version: u32,
    target_id: String,
    kind: StatusKind,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    contract_digest_sha256: Option<String>,
    #[serde(default)]
    accepted_observation: Option<Observation>,
    #[serde(default)]
    error_detail: Option<DiagnosticDetail>,
    #[serde(default)]
    lifecycle: Option<LifecycleSnapshot>,
}

impl<'de> Deserialize<'de> for StatusReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = StatusReportWire::deserialize(deserializer)?;
        let report = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            target_id: wire.target_id,
            kind: wire.kind,
            display_name: wire.display_name,
            enabled: wire.enabled,
            contract_digest_sha256: wire.contract_digest_sha256,
            accepted_observation: wire.accepted_observation,
            error_detail: wire.error_detail,
            lifecycle: wire.lifecycle,
        };
        report.validate_schema().map_err(serde::de::Error::custom)?;
        Ok(report)
    }
}

/// Structured outcome of a blind storage reset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResetReport {
    pub(super) schema_name: String,
    pub(super) schema_version: u32,
    pub(super) target_id: String,
    pub(super) storage_cleared: bool,
    pub(super) delivery_outcomes: Vec<DeliveryOutcome>,
    pub(super) outbox_overflow: Vec<OutboxOverflow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) outbox_error_detail: Option<DiagnosticDetail>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResetReportWire {
    schema_name: String,
    schema_version: u32,
    target_id: String,
    storage_cleared: bool,
    delivery_outcomes: Vec<DeliveryOutcome>,
    outbox_overflow: Vec<OutboxOverflow>,
    #[serde(default)]
    outbox_error_detail: Option<DiagnosticDetail>,
}

impl<'de> Deserialize<'de> for ResetReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ResetReportWire::deserialize(deserializer)?;
        let report = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            target_id: wire.target_id,
            storage_cleared: wire.storage_cleared,
            delivery_outcomes: wire.delivery_outcomes,
            outbox_overflow: wire.outbox_overflow,
            outbox_error_detail: wire.outbox_error_detail,
        };
        report.validate_schema().map_err(serde::de::Error::custom)?;
        Ok(report)
    }
}

impl RunReport {
    pub(super) fn validate_schema(&self) -> Result<(), CoreError> {
        if self.schema_name != RUN_REPORT_SCHEMA_NAME
            || self.schema_version != RUN_REPORT_SCHEMA_VERSION
        {
            return Err(CoreError::contract(
                "run report is not a current FFHN run report",
            ));
        }
        self.lifecycle.validate()?;
        let after = self.lifecycle.after();
        if self.state_persisted && after.is_none() {
            return Err(CoreError::contract(
                "a persisted run report must carry its staged lifecycle snapshot",
            ));
        }
        match self.outcome.lifecycle_after_rule() {
            LifecycleAfterRule::Forbidden if after.is_some() => {
                return Err(CoreError::contract(
                    "this run outcome cannot carry a staged lifecycle transition",
                ));
            }
            LifecycleAfterRule::Required if after.is_none() => {
                return Err(CoreError::contract(
                    "this run outcome must carry its staged lifecycle snapshot",
                ));
            }
            LifecycleAfterRule::Forbidden
            | LifecycleAfterRule::Required
            | LifecycleAfterRule::Optional => {}
        }
        Ok(())
    }
}

impl BatchRunReport {
    fn validate_schema(&self) -> Result<(), CoreError> {
        if self.schema_name != BATCH_RUN_REPORT_SCHEMA_NAME
            || self.schema_version != BATCH_RUN_REPORT_SCHEMA_VERSION
        {
            return Err(CoreError::contract(
                "batch report is not a current FFHN batch report",
            ));
        }
        Ok(())
    }
}

impl StatusReport {
    pub(super) fn validate_schema(&self) -> Result<(), CoreError> {
        if self.schema_name != STATUS_REPORT_SCHEMA_NAME
            || self.schema_version != STATUS_REPORT_SCHEMA_VERSION
        {
            return Err(CoreError::contract(
                "status report is not a current FFHN status report",
            ));
        }
        match (self.kind, self.accepted_observation.is_some()) {
            (StatusKind::Ready, false) => {
                return Err(CoreError::contract(
                    "a ready status report must carry its accepted observation",
                ));
            }
            (
                StatusKind::Pending
                | StatusKind::InvalidConfig
                | StatusKind::UnavailableTarget
                | StatusKind::InvalidState,
                true,
            ) => {
                return Err(CoreError::contract(
                    "only a ready status report can carry an accepted observation",
                ));
            }
            (StatusKind::Ready, true)
            | (
                StatusKind::Pending
                | StatusKind::InvalidConfig
                | StatusKind::UnavailableTarget
                | StatusKind::InvalidState,
                false,
            ) => {}
        }
        match (self.kind, self.lifecycle.as_ref()) {
            (StatusKind::UnavailableTarget | StatusKind::InvalidState, Some(_)) => {
                return Err(CoreError::contract(
                    "this status kind cannot expose unverified lifecycle state",
                ));
            }
            (StatusKind::Ready, None) => {
                return Err(CoreError::contract(
                    "a ready status report must carry its verified durable lifecycle state",
                ));
            }
            (_, Some(lifecycle)) => {
                lifecycle.validate()?;
                if self.display_name.is_none()
                    || self.enabled.is_none()
                    || self.contract_digest_sha256.is_none()
                {
                    return Err(CoreError::contract(
                        "a lifecycle-bearing status report must retain verified target identity facts",
                    ));
                }
            }
            (_, None) => {}
        }
        Ok(())
    }
}

impl ResetReport {
    fn validate_schema(&self) -> Result<(), CoreError> {
        if self.schema_name != RESET_REPORT_SCHEMA_NAME
            || self.schema_version != RESET_REPORT_SCHEMA_VERSION
        {
            return Err(CoreError::contract(
                "reset report is not a current FFHN reset report",
            ));
        }
        Ok(())
    }
}

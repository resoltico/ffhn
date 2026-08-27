//! Offline graph inspection operations that never create lineage or execute a delivery.

use std::{thread, time::Duration};

use crate::CoreError;

use super::{
    IntegrationFaultEpisode, MeasurementExtractionHealth, MeasurementId, SourceAcquisitionHealth,
    SourceId, SourceLineage, TrustedGraphRoot,
};

#[path = "inspection/status_measurements.rs"]
mod status_measurements;
use status_measurements::status_measurements;

const STATUS_LOCK_ATTEMPTS: usize = 5;
const STATUS_LOCK_RETRY: Duration = Duration::from_millis(10);

const fn status_lock_retry_remaining(attempt: usize) -> bool {
    attempt + 1 < STATUS_LOCK_ATTEMPTS
}

/// Selects the stable graph listing view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphListScope {
    /// List source identifiers.
    Sources,
    /// List every configured measurement, paired with its source identifier.
    Measurements,
}

/// One graph-list item, always carrying its containing source identifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphListItem {
    source_id: SourceId,
    measurement_id: Option<MeasurementId>,
}

/// Result of a graph listing operation in stable identifier order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphListResult {
    scope: GraphListScope,
    items: Vec<GraphListItem>,
}

/// Source-level status observed under the shared source reader lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphSourceStatusKind {
    /// A manifest is present and a writer must complete or reset it before status can be exact.
    Pending,
    /// Source authority or source state violates the lineage contract.
    LineageRefused,
    /// The configured source has not yet initialized source lineage.
    Uninitialized,
    /// Source lineage and state agree.
    Ready,
    /// Source configuration is invalid even if durable lineage remains readable.
    ConfigInvalid,
}

/// Measurement-level lineage status observed under the shared source reader lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphMeasurementStatusKind {
    /// A configured measurement has no lineage or artifact yet.
    NeverInitialized,
    /// Measurement state and lineage agree.
    Ready,
    /// Only this measurement is held pending an explicit measurement reset.
    LineageHeld,
    /// An authoritative or artifact-backed measurement is not currently configured.
    NotConfigured,
    /// Measurement configuration or projection preflight is invalid.
    ConfigInvalid,
    /// Stored state belongs to a different current measurement value digest.
    Quarantined,
}

/// Stable source facts for the public status operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphSourceStatusResult {
    source_id: SourceId,
    kind: GraphSourceStatusKind,
    config_error: Option<String>,
    pending_manifest: Option<super::UnresolvableManifest>,
    lineage_refusal: Option<super::SourceLineageRefusal>,
    generation: Option<u64>,
    next_due_utc: Option<String>,
    source_health: Option<SourceAcquisitionHealth>,
    integration_fault_episode: Option<IntegrationFaultEpisode>,
    outbox_overflow: Vec<super::OutboxOverflowFact>,
}

/// Stable measurement facts for the public status operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphMeasurementStatusResult {
    measurement_id: MeasurementId,
    kind: GraphMeasurementStatusKind,
    config_error: Option<String>,
    lineage_hold: Option<super::MeasurementLineageHold>,
    stored_measurement_value_digest: Option<String>,
    current_measurement_value_digest: Option<String>,
    observation_seq: Option<u64>,
    extraction_health: Option<MeasurementExtractionHealth>,
    integration_fault_episode: Option<IntegrationFaultEpisode>,
    outbox_overflow: Vec<super::OutboxOverflowFact>,
}

/// One immutable status snapshot of a source and every observed measurement lineage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphStatusResult {
    source: GraphSourceStatusResult,
    measurements: Vec<GraphMeasurementStatusResult>,
}

/// Offline validation scope for a graph configuration document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphValidationScope {
    /// Source-owned acquisition and delivery configuration.
    Source,
    /// Measurement-owned projection, type, policy, and delivery configuration.
    Measurement,
}

/// One Stage-A or Stage-B validation result. Invalid configuration never mutates lineage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphValidationIssue {
    source_id: SourceId,
    measurement_id: Option<MeasurementId>,
    scope: GraphValidationScope,
    message: Option<String>,
}

/// Full offline configuration validation result in stable source/measurement order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphValidationResult {
    issues: Vec<GraphValidationIssue>,
}

impl GraphListScope {
    /// Returns the stable serialized spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sources => "sources",
            Self::Measurements => "measurements",
        }
    }
}

impl GraphListItem {
    /// Returns the containing source identifier.
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    /// Returns the measurement identifier for a measurement listing item.
    pub fn measurement_id(&self) -> Option<&MeasurementId> {
        self.measurement_id.as_ref()
    }
}

impl GraphListResult {
    /// Returns the selected listing scope.
    pub const fn scope(&self) -> GraphListScope {
        self.scope
    }

    /// Returns entries in stable identifier order.
    pub fn items(&self) -> &[GraphListItem] {
        &self.items
    }
}

impl GraphSourceStatusKind {
    /// Returns the stable report spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::LineageRefused => "lineage_refused",
            Self::Uninitialized => "uninitialized",
            Self::Ready => "ready",
            Self::ConfigInvalid => "config_invalid",
        }
    }
}

impl GraphMeasurementStatusKind {
    /// Returns the stable report spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeverInitialized => "never_initialized",
            Self::Ready => "ready",
            Self::LineageHeld => "lineage_held",
            Self::NotConfigured => "not_configured",
            Self::ConfigInvalid => "config_invalid",
            Self::Quarantined => "quarantined",
        }
    }
}

impl GraphSourceStatusResult {
    /// Returns the source identifier.
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }
    /// Returns the lineage status.
    pub const fn kind(&self) -> GraphSourceStatusKind {
        self.kind
    }
    /// Returns the source configuration diagnostic, if offline decoding or validation failed.
    pub fn config_error(&self) -> Option<&str> {
        self.config_error.as_deref()
    }
    /// Returns the manifest class that makes a pending status inexact.
    pub const fn pending_manifest(&self) -> Option<super::UnresolvableManifest> {
        self.pending_manifest
    }
    /// Returns the authoritative source-lineage refusal reason.
    pub const fn lineage_refusal(&self) -> Option<super::SourceLineageRefusal> {
        self.lineage_refusal
    }
    /// Returns the fully-installed source generation when lineage is ready.
    pub const fn generation(&self) -> Option<u64> {
        self.generation
    }
    /// Returns the durable next acquisition due time when present.
    pub fn next_due_utc(&self) -> Option<&str> {
        self.next_due_utc.as_deref()
    }
    /// Returns the source health episode when source lineage is ready.
    pub fn source_health(&self) -> Option<&SourceAcquisitionHealth> {
        self.source_health.as_ref()
    }
    /// Returns the source integration-fault episode when active.
    pub fn integration_fault_episode(&self) -> Option<&IntegrationFaultEpisode> {
        self.integration_fault_episode.as_ref()
    }
    /// Returns the latest committed source-owned overflow facts.
    pub fn outbox_overflow(&self) -> &[super::OutboxOverflowFact] {
        &self.outbox_overflow
    }
}

impl GraphMeasurementStatusResult {
    /// Returns the measurement identifier.
    pub fn measurement_id(&self) -> &MeasurementId {
        &self.measurement_id
    }
    /// Returns the measurement lineage status.
    pub const fn kind(&self) -> GraphMeasurementStatusKind {
        self.kind
    }
    /// Returns the measurement configuration diagnostic, if any.
    pub fn config_error(&self) -> Option<&str> {
        self.config_error.as_deref()
    }
    /// Returns the measurement-scoped lineage-hold reason.
    pub const fn lineage_hold(&self) -> Option<super::MeasurementLineageHold> {
        self.lineage_hold
    }
    /// Returns the stored measurement value digest that caused quarantine.
    pub fn stored_measurement_value_digest(&self) -> Option<&str> {
        self.stored_measurement_value_digest.as_deref()
    }
    /// Returns the current configured measurement value digest.
    pub fn current_measurement_value_digest(&self) -> Option<&str> {
        self.current_measurement_value_digest.as_deref()
    }
    /// Returns the accepted-observation sequence when state is ready.
    pub const fn observation_seq(&self) -> Option<u64> {
        self.observation_seq
    }
    /// Returns the extraction-health episode when measurement lineage is ready.
    pub fn extraction_health(&self) -> Option<&MeasurementExtractionHealth> {
        self.extraction_health.as_ref()
    }
    /// Returns the measurement integration-fault episode when active.
    pub fn integration_fault_episode(&self) -> Option<&IntegrationFaultEpisode> {
        self.integration_fault_episode.as_ref()
    }
    /// Returns the latest committed measurement-owned overflow facts.
    pub fn outbox_overflow(&self) -> &[super::OutboxOverflowFact] {
        &self.outbox_overflow
    }
}

impl GraphStatusResult {
    /// Returns source-level status facts.
    pub fn source(&self) -> &GraphSourceStatusResult {
        &self.source
    }
    /// Returns every configured, authoritative, or artifact-backed measurement status.
    pub fn measurements(&self) -> &[GraphMeasurementStatusResult] {
        &self.measurements
    }
}

impl GraphValidationScope {
    /// Returns the stable report spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Measurement => "measurement",
        }
    }
}

impl GraphValidationIssue {
    /// Returns the containing source identifier.
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }
    /// Returns the measurement identifier for a Stage-B issue.
    pub fn measurement_id(&self) -> Option<&MeasurementId> {
        self.measurement_id.as_ref()
    }
    /// Returns the validation stage owner.
    pub const fn scope(&self) -> GraphValidationScope {
        self.scope
    }
    /// Returns the diagnostic when this item is invalid; `None` means valid.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
    /// Returns whether this scope is valid.
    pub const fn is_valid(&self) -> bool {
        self.message.is_none()
    }
}

impl GraphValidationResult {
    /// Returns validation facts in stable source/measurement order.
    pub fn issues(&self) -> &[GraphValidationIssue] {
        &self.issues
    }
    /// Returns whether all checked configuration scopes are valid.
    pub fn is_valid(&self) -> bool {
        self.issues.iter().all(GraphValidationIssue::is_valid)
    }
}

/// Lists graph sources or their configured measurement documents without touching lineage state.
pub fn list_graph(
    graph: &TrustedGraphRoot,
    scope: GraphListScope,
) -> Result<GraphListResult, CoreError> {
    graph.validate_graph_documents()?;
    let mut sources = graph.source_ids()?;
    sources.sort_unstable();
    let mut items = Vec::new();
    for source_id in sources {
        match scope {
            GraphListScope::Sources => items.push(GraphListItem {
                source_id,
                measurement_id: None,
            }),
            GraphListScope::Measurements => {
                let source = graph.open_source(source_id.clone())?;
                let mut measurements = source.measurement_ids()?;
                measurements.sort_unstable();
                items.extend(
                    measurements
                        .into_iter()
                        .map(|measurement_id| GraphListItem {
                            source_id: source_id.clone(),
                            measurement_id: Some(measurement_id),
                        }),
                );
            }
        }
    }
    Ok(GraphListResult { scope, items })
}

/// Reads one status snapshot under a bounded shared-lock retry without recovering manifests.
pub fn status_source(
    graph: &TrustedGraphRoot,
    source_id: SourceId,
    measurement_id: Option<MeasurementId>,
) -> Result<GraphStatusResult, CoreError> {
    graph.validate_graph_documents()?;
    let source = graph.open_source(source_id)?;
    let mut lease = None;
    for attempt in 0..STATUS_LOCK_ATTEMPTS {
        if let Some(acquired) = source.try_acquire_read_lease()? {
            lease = Some(acquired);
            break;
        }
        if status_lock_retry_remaining(attempt) {
            thread::sleep(STATUS_LOCK_RETRY);
        }
    }
    let Some(_lease) = lease else {
        return Err(CoreError::contract("source is busy"));
    };
    let pending_manifest = if !matches!(source.read_lineage_manifest(), Ok(None)) {
        Some(super::UnresolvableManifest::Lineage)
    } else if source
        .open_storage()
        .is_ok_and(|storage| !matches!(storage.read_commit_manifest(), Ok(None)))
    {
        Some(super::UnresolvableManifest::Commit)
    } else {
        None
    };
    if let Some(pending_manifest) = pending_manifest {
        return Ok(GraphStatusResult {
            source: GraphSourceStatusResult {
                source_id: source.paths().source_id().clone(),
                kind: GraphSourceStatusKind::Pending,
                config_error: source
                    .read_source_document()
                    .err()
                    .map(|error| error.to_string()),
                pending_manifest: Some(pending_manifest),
                lineage_refusal: None,
                generation: None,
                next_due_utc: None,
                source_health: None,
                integration_fault_episode: None,
                outbox_overflow: Vec::new(),
            },
            measurements: Vec::new(),
        });
    }
    let config_ids = source.measurement_ids().unwrap_or_default();
    let source_document = source.read_source_document();
    let source_config_error = source_document.as_ref().err().map(ToString::to_string);
    let inspection = source.inspect_lineage(config_ids.iter().cloned())?;
    let state = match inspection.source() {
        SourceLineage::NeedsInitialization => GraphSourceStatusResult {
            source_id: source.paths().source_id().clone(),
            kind: if source_config_error.is_some() {
                GraphSourceStatusKind::ConfigInvalid
            } else {
                GraphSourceStatusKind::Uninitialized
            },
            config_error: source_config_error,
            pending_manifest: None,
            lineage_refusal: None,
            generation: None,
            next_due_utc: None,
            source_health: None,
            integration_fault_episode: None,
            outbox_overflow: Vec::new(),
        },
        SourceLineage::Refused(reason) => GraphSourceStatusResult {
            source_id: source.paths().source_id().clone(),
            kind: GraphSourceStatusKind::LineageRefused,
            config_error: source_config_error,
            pending_manifest: None,
            lineage_refusal: Some(*reason),
            generation: None,
            next_due_utc: None,
            source_health: None,
            integration_fault_episode: None,
            outbox_overflow: Vec::new(),
        },
        SourceLineage::Ready(ready) => GraphSourceStatusResult {
            source_id: source.paths().source_id().clone(),
            kind: if source_config_error.is_some() {
                GraphSourceStatusKind::ConfigInvalid
            } else {
                GraphSourceStatusKind::Ready
            },
            config_error: source_config_error,
            pending_manifest: None,
            lineage_refusal: None,
            generation: Some(ready.state().generation()),
            next_due_utc: ready.state().next_due_utc().map(ToOwned::to_owned),
            source_health: Some(ready.state().source_health().clone()),
            integration_fault_episode: ready.state().integration_fault_episode().cloned(),
            outbox_overflow: ready.state().outbox_overflow().to_vec(),
        },
    };
    Ok(GraphStatusResult {
        source: state,
        measurements: status_measurements(
            &source,
            &inspection,
            &config_ids,
            measurement_id,
            source_document.as_ref().ok(),
        ),
    })
}

/// Validates Stage A then every discoverable Stage-B document without fetching or mutating state.
pub fn validate_graph(
    graph: &TrustedGraphRoot,
    selected_source: Option<SourceId>,
) -> Result<GraphValidationResult, CoreError> {
    graph.validate_graph_documents()?;
    let mut ids = match selected_source {
        Some(id) => vec![id],
        None => graph.source_ids()?,
    };
    ids.sort_unstable();
    let mut issues = Vec::new();
    for source_id in ids {
        let source = graph.open_source(source_id.clone())?;
        let source_document = source.read_source_document();
        let source_error = source_document.as_ref().err().map(ToString::to_string);
        issues.push(GraphValidationIssue {
            source_id: source_id.clone(),
            measurement_id: None,
            scope: GraphValidationScope::Source,
            message: source_error,
        });
        let measurement_ids = match source.measurement_ids() {
            Ok(mut ids) => {
                ids.sort_unstable();
                ids
            }
            Err(error) => {
                issues.push(GraphValidationIssue {
                    source_id,
                    measurement_id: None,
                    scope: GraphValidationScope::Measurement,
                    message: Some(error.to_string()),
                });
                continue;
            }
        };
        for measurement_id in measurement_ids {
            let document = source.read_measurement_document(&measurement_id);
            let message = match (&source_document, document) {
                (_, Err(error)) => Some(error.to_string()),
                (Err(_), Ok(_)) => Some(
                    "measurement value validation is withheld until source configuration is valid"
                        .to_owned(),
                ),
                (Ok(source_document), Ok(document)) => document
                    .measurement_value_digest(source_document)
                    .and_then(|_| {
                        super::PreparedMeasurementProjection::prepare(&document).map(|_| ())
                    })
                    .err()
                    .map(|error| error.to_string()),
            };
            issues.push(GraphValidationIssue {
                source_id: source_id.clone(),
                measurement_id: Some(measurement_id),
                scope: GraphValidationScope::Measurement,
                message,
            });
        }
    }
    Ok(GraphValidationResult { issues })
}

#[cfg(test)]
#[path = "inspection/tests.rs"]
mod tests;

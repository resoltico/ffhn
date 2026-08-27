//! v11 observation-graph domain boundary.

mod acquire;
mod agent;
mod commit;
mod config_io;
mod cycle;
mod delivery_commit;
mod delivery_config;
mod delivery_drain;
mod delivery_execute;
mod engine;
mod event;
mod event_materialize;
mod health;
mod identity;
mod inspection;
mod lineage_gate;
mod lineage_manifest;
mod locks;
mod manifest;
mod measurement;
mod outbox;
mod outbox_storage;
mod paths;
mod projection;
mod reports;
mod reset;
mod retry;
mod source;
mod state;
mod storage;
mod transition;
mod turn_entry;

#[cfg(test)]
mod test_support;

pub use acquire::{
    HttpValidators, SourceAcquireError, SourceAcquisition, SourceDocumentBytes, SourceFetchFailure,
    SourceFetchFailureKind, SourceFetchFailureReasonClass, acquire_source,
    acquire_source_with_validators,
};
pub use cycle::{SourceCycleDecision, decide_source_cycle};
pub use delivery_commit::{OutboxOwner, commit_delivery_result};
pub use delivery_config::{
    DeliveryHeaderSecret, DeliveryPolicy, GraphDeliveryAdapter, GraphRoute, GraphRouteFamily,
    GraphRouteId,
};
pub use health::{
    ExtractionFailureReason, GraphIntegrationFaultCode, HealthState, IntegrationFaultEpisode,
    MeasurementExtractionHealth, SourceAcquisitionHealth,
};
pub use identity::{
    GRAPH_IDENTITY_SCHEMA_NAME, GRAPH_IDENTITY_SCHEMA_VERSION, GraphId, GraphIdentity,
    MeasurementId, MeasurementIdentity, MeasurementInstanceId, SOURCE_IDENTITY_SCHEMA_NAME,
    SOURCE_IDENTITY_SCHEMA_VERSION, SourceId, SourceIdentity, SourceInstanceId,
};
pub use lineage_gate::{
    LineageInspection, MeasurementLineage, MeasurementLineageHold, ReadySourceLineage,
    SourceLineage, SourceLineageRefusal,
};
pub use lineage_manifest::{
    LINEAGE_MANIFEST_SCHEMA_NAME, LINEAGE_MANIFEST_SCHEMA_VERSION, LineageManifest,
    LineageManifestParts, LineageRecovery, LineageScope,
};
pub use manifest::{
    COMMIT_MANIFEST_SCHEMA_NAME, COMMIT_MANIFEST_SCHEMA_VERSION, CommitCause, CommitManifest,
    CommitManifestParts, CommitOperation, CommitRecovery, ManifestPath,
};
pub use measurement::{
    MEASUREMENT_POLICY_SEMANTICS_VERSION, MEASUREMENT_SCHEMA_NAME, MEASUREMENT_SCHEMA_VERSION,
    MEASUREMENT_VALUE_SEMANTICS_VERSION, MeasurementDocument,
};
pub use outbox::{
    DEAD_LETTER_SCHEMA_NAME, DEAD_LETTER_SCHEMA_VERSION, DELIVERY_RECORD_SCHEMA_NAME,
    DELIVERY_RECORD_SCHEMA_VERSION, DeadLetter, DeliveryAttempt, DeliveryAttemptFailure,
    DeliveryRecord, OutboxAdmission, OutboxOverflowFact,
};
pub use paths::{GraphPaths, SourcePaths};
pub use projection::{MeasurementProjectionFailure, PreparedMeasurementProjection};
pub use source::{
    AGENT_SCHEMA_NAME, AGENT_SCHEMA_VERSION, AgentDocument, ConditionalRequests, FetchHeaderSecret,
    HttpTimeouts, SOURCE_SCHEMA_NAME, SOURCE_SCHEMA_VERSION, SourceDocument, SourceFetch,
    SourceSchedule,
};
pub use state::{
    MEASUREMENT_STATE_SCHEMA_NAME, MEASUREMENT_STATE_SCHEMA_VERSION, MeasurementState,
    SOURCE_STATE_SCHEMA_NAME, SOURCE_STATE_SCHEMA_VERSION, SourceState,
};
pub use storage::{TrustedGraphRoot, TrustedSourceDir, TrustedStorageDir};
pub use turn_entry::{SourceTurnEntry, UnresolvableManifest};
pub use {
    agent::{AgentSourceTurn, AgentTickResult, AgentWorker, agent_tick, agent_tick_with_jobs},
    engine::{
        GraphMeasureResult, GraphMeasurementResult, GraphMeasurementStatus, GraphSourceStatus,
        measure_selected_source_dry_run, measure_selected_source_once, measure_source_dry_run,
        measure_source_once,
    },
    inspection::{
        GraphListItem, GraphListResult, GraphListScope, GraphMeasurementStatusKind,
        GraphMeasurementStatusResult, GraphSourceStatusKind, GraphSourceStatusResult,
        GraphStatusResult, GraphValidationIssue, GraphValidationResult, GraphValidationScope,
        list_graph, status_source, validate_graph,
    },
    reports::{
        AGENT_STATUS_REPORT_SCHEMA_NAME, AGENT_STATUS_REPORT_SCHEMA_VERSION,
        AGENT_TICK_REPORT_SCHEMA_NAME, AGENT_TICK_REPORT_SCHEMA_VERSION, AgentStatusReport,
        AgentTickReport, GRAPH_LIST_REPORT_SCHEMA_NAME, GRAPH_LIST_REPORT_SCHEMA_VERSION,
        GRAPH_MEASUREMENT_STATUS_REPORT_SCHEMA_NAME,
        GRAPH_MEASUREMENT_STATUS_REPORT_SCHEMA_VERSION, GRAPH_NEW_REPORT_SCHEMA_NAME,
        GRAPH_NEW_REPORT_SCHEMA_VERSION, GRAPH_RESET_REPORT_SCHEMA_NAME,
        GRAPH_RESET_REPORT_SCHEMA_VERSION, GRAPH_SOURCE_STATUS_REPORT_SCHEMA_NAME,
        GRAPH_SOURCE_STATUS_REPORT_SCHEMA_VERSION, GRAPH_VALIDATE_REPORT_SCHEMA_NAME,
        GRAPH_VALIDATE_REPORT_SCHEMA_VERSION, GraphListReport, GraphMeasureReport,
        GraphMeasurementStatusReport, GraphNewReport, GraphResetReport, GraphSourceStatusReport,
        GraphValidateReport, MEASURE_REPORT_SCHEMA_NAME, MEASURE_REPORT_SCHEMA_VERSION,
    },
    reset::{DiscardedManifestEvidence, GraphResetResult, reset_measurement, reset_source},
};
pub use {
    delivery_drain::{DrainResult, drain_measurement_outbox_once, drain_source_outbox_once},
    delivery_execute::{DeliveryExecution, execute_delivery_attempt},
};
pub use {
    event::{
        EVENT_ENVELOPE_SCHEMA_NAME, EVENT_ENVELOPE_SCHEMA_VERSION, EventEmitter, EventEnvelope,
        EventEnvelopeParts, EventKey, EventKind, EventObservation,
    },
    event_materialize::materialize_condition_events,
};

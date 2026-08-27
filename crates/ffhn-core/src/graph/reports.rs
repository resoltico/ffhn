//! Public observation-graph operation documents.

use base64::Engine as _;
use serde::Serialize;

use crate::ConditionEvaluation;

use super::{AgentTickResult, EventEnvelope, GraphMeasureResult, GraphResetResult};

#[path = "reports/inspection.rs"]
mod inspection;

pub use inspection::{
    GRAPH_LIST_REPORT_SCHEMA_NAME, GRAPH_LIST_REPORT_SCHEMA_VERSION,
    GRAPH_MEASUREMENT_STATUS_REPORT_SCHEMA_NAME, GRAPH_MEASUREMENT_STATUS_REPORT_SCHEMA_VERSION,
    GRAPH_SOURCE_STATUS_REPORT_SCHEMA_NAME, GRAPH_SOURCE_STATUS_REPORT_SCHEMA_VERSION,
    GRAPH_VALIDATE_REPORT_SCHEMA_NAME, GRAPH_VALIDATE_REPORT_SCHEMA_VERSION, GraphListReport,
    GraphMeasurementStatusReport, GraphSourceStatusReport, GraphValidateReport,
};
#[path = "reports/agent_status.rs"]
mod agent_status;
pub use agent_status::{
    AGENT_STATUS_REPORT_SCHEMA_NAME, AGENT_STATUS_REPORT_SCHEMA_VERSION, AgentStatusReport,
};

/// Schema name for one v11 measurement report.
pub const MEASURE_REPORT_SCHEMA_NAME: &str = "ffhn.measure_report";
/// Schema version for one v11 measurement report.
pub const MEASURE_REPORT_SCHEMA_VERSION: u32 = 1;
/// Schema name for one v11 agent tick report.
pub const AGENT_TICK_REPORT_SCHEMA_NAME: &str = "ffhn.agent_tick_report";
/// Schema version for one v11 agent tick report.
pub const AGENT_TICK_REPORT_SCHEMA_VERSION: u32 = 1;
/// Schema name for one v11 reset report.
pub const GRAPH_RESET_REPORT_SCHEMA_NAME: &str = "ffhn.reset_report";
/// Schema version for one v11 reset report.
pub const GRAPH_RESET_REPORT_SCHEMA_VERSION: u32 = 1;
/// Schema name for one graph configuration-creation report.
pub const GRAPH_NEW_REPORT_SCHEMA_NAME: &str = "ffhn.new_report";
/// Schema version for one graph configuration-creation report.
pub const GRAPH_NEW_REPORT_SCHEMA_VERSION: u32 = 1;

/// Public result of creating one source or measurement configuration document.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphNewReport {
    schema_name: String,
    schema_version: u32,
    kind: String,
    source_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    measurement_id: Option<String>,
}

impl GraphNewReport {
    /// Reports one newly created source configuration.
    pub fn source(source_id: &super::SourceId) -> Self {
        Self {
            schema_name: GRAPH_NEW_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: GRAPH_NEW_REPORT_SCHEMA_VERSION,
            kind: "source".to_owned(),
            source_id: source_id.as_str().to_owned(),
            measurement_id: None,
        }
    }

    /// Reports one newly created measurement configuration.
    pub fn measurement(source_id: &super::SourceId, measurement_id: &super::MeasurementId) -> Self {
        Self {
            schema_name: GRAPH_NEW_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: GRAPH_NEW_REPORT_SCHEMA_VERSION,
            kind: "measurement".to_owned(),
            source_id: source_id.as_str().to_owned(),
            measurement_id: Some(measurement_id.as_str().to_owned()),
        }
    }
}

/// Route-independent public result of one graph source measurement command.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphMeasureReport {
    schema_name: String,
    schema_version: u32,
    source_id: String,
    source_status: String,
    source_event_envelopes: Vec<EventEnvelope>,
    source_outbox_overflow: Vec<super::OutboxOverflowFact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_config_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unresolvable_manifest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_lineage_refusal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_health: Option<super::SourceAcquisitionHealth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_integration_fault_episode: Option<super::IntegrationFaultEpisode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_failure: Option<GraphSourceFailureReport>,
    measurements: Vec<GraphMeasurementReport>,
}

/// Public measurement-local facts from one shared source cycle.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphMeasurementReport {
    measurement_id: String,
    status: String,
    policy_evaluations: Vec<ConditionEvaluation>,
    condition_event_envelopes: Vec<EventEnvelope>,
    outbox_overflow: Vec<super::OutboxOverflowFact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    config_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lineage_hold: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stored_measurement_value_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_measurement_value_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extraction_health: Option<super::MeasurementExtractionHealth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_fault_episode: Option<super::IntegrationFaultEpisode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observation: Option<crate::Observation>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct GraphSourceFailureReport {
    kind: super::SourceFetchFailureKind,
    reason_class: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    raw_platform_error: Option<String>,
}

/// Public finite agent-tick result.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentTickReport {
    schema_name: String,
    schema_version: u32,
    source_turns: Vec<AgentSourceTurnReport>,
}

/// Public source turn facts including independent acquisition and drain evidence.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSourceTurnReport {
    source_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    acquisition: Option<GraphMeasureReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    acquisition_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_drain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    drain_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    acquisition_deferred_until: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    acquisition_deferred_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    drain_deferred_until: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    drain_deferred_reason: Option<String>,
    measurement_drains: Vec<MeasurementDrainReport>,
    measurement_drain_deferrals: Vec<MeasurementDrainDeferralReport>,
}

/// Public per-measurement drain disposition.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MeasurementDrainReport {
    measurement_id: String,
    status: String,
}

/// Public pacing boundary for one measurement-owned drain capability.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MeasurementDrainDeferralReport {
    measurement_id: String,
    deferred_until: String,
    reason: String,
}

/// Public reset scope result.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphResetReport {
    schema_name: String,
    schema_version: u32,
    source_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    measurement_id: Option<String>,
    discarded_manifests: Vec<DiscardedManifestReport>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct DiscardedManifestReport {
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes_base64: Option<String>,
    bytes_unavailable: bool,
}

impl From<&GraphMeasureResult> for GraphMeasureReport {
    fn from(result: &GraphMeasureResult) -> Self {
        Self {
            schema_name: MEASURE_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: MEASURE_REPORT_SCHEMA_VERSION,
            source_id: result.source_id().as_str().to_owned(),
            source_status: result.status().as_str().to_owned(),
            source_event_envelopes: result.source_event_envelopes().to_vec(),
            source_outbox_overflow: result.source_outbox_overflow().to_vec(),
            source_config_error: result.source_config_error().map(ToOwned::to_owned),
            unresolvable_manifest: result
                .unresolvable_manifest()
                .map(|kind| kind.as_str().to_owned()),
            source_lineage_refusal: result
                .source_lineage_refusal()
                .map(|reason| reason.as_str().to_owned()),
            source_health: result.source_health().cloned(),
            source_integration_fault_episode: result.source_integration_fault_episode().cloned(),
            source_failure: result
                .source_failure()
                .map(|failure| GraphSourceFailureReport {
                    kind: failure.kind,
                    reason_class: failure.reason_class().as_str().to_owned(),
                    status: failure.status,
                    raw_platform_error: failure.raw_platform_error.clone(),
                }),
            measurements: result
                .measurements()
                .iter()
                .map(|measurement| GraphMeasurementReport {
                    measurement_id: measurement.measurement_id().as_str().to_owned(),
                    status: measurement.status().as_str().to_owned(),
                    policy_evaluations: measurement.policy_evaluations().to_vec(),
                    condition_event_envelopes: measurement.event_envelopes().to_vec(),
                    outbox_overflow: measurement.outbox_overflow().to_vec(),
                    config_error: measurement.config_error().map(ToOwned::to_owned),
                    lineage_hold: measurement
                        .lineage_hold()
                        .map(|hold| hold.as_str().to_owned()),
                    stored_measurement_value_digest: measurement
                        .stored_measurement_value_digest()
                        .map(ToOwned::to_owned),
                    current_measurement_value_digest: measurement
                        .current_measurement_value_digest()
                        .map(ToOwned::to_owned),
                    extraction_health: measurement.extraction_health().cloned(),
                    integration_fault_episode: measurement.integration_fault_episode().cloned(),
                    observation: measurement.observation().cloned(),
                })
                .collect(),
        }
    }
}

impl From<&AgentTickResult> for AgentTickReport {
    fn from(result: &AgentTickResult) -> Self {
        Self {
            schema_name: AGENT_TICK_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: AGENT_TICK_REPORT_SCHEMA_VERSION,
            source_turns: result
                .sources()
                .iter()
                .map(|turn| AgentSourceTurnReport {
                    source_id: turn.source_id().as_str().to_owned(),
                    acquisition: turn.measurement().map(GraphMeasureReport::from),
                    acquisition_error: turn.acquisition_error().map(ToOwned::to_owned),
                    source_drain: turn.source_drain().map(|drain| drain.as_str().to_owned()),
                    drain_error: turn.drain_error().map(ToOwned::to_owned),
                    acquisition_deferred_until: turn
                        .acquisition_deferred_until()
                        .map(ToOwned::to_owned),
                    acquisition_deferred_reason: turn
                        .acquisition_deferred_reason()
                        .map(ToOwned::to_owned),
                    drain_deferred_until: turn.drain_deferred_until().map(ToOwned::to_owned),
                    drain_deferred_reason: turn.drain_deferred_reason().map(ToOwned::to_owned),
                    measurement_drains: turn
                        .measurement_drains()
                        .iter()
                        .map(|(id, status)| MeasurementDrainReport {
                            measurement_id: id.clone(),
                            status: status.as_str().to_owned(),
                        })
                        .collect(),
                    measurement_drain_deferrals: turn
                        .measurement_drain_deferrals()
                        .iter()
                        .map(|(id, until, reason)| MeasurementDrainDeferralReport {
                            measurement_id: id.clone(),
                            deferred_until: until.clone(),
                            reason: reason.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

impl From<&GraphResetResult> for GraphResetReport {
    fn from(result: &GraphResetResult) -> Self {
        Self {
            schema_name: GRAPH_RESET_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: GRAPH_RESET_REPORT_SCHEMA_VERSION,
            source_id: result.source_id().as_str().to_owned(),
            measurement_id: result.measurement_id().map(|id| id.as_str().to_owned()),
            discarded_manifests: result
                .discarded_manifests()
                .iter()
                .map(|evidence| DiscardedManifestReport {
                    kind: evidence.kind().as_str().to_owned(),
                    bytes_base64: evidence
                        .bytes()
                        .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes)),
                    bytes_unavailable: evidence.bytes().is_none(),
                })
                .collect(),
        }
    }
}

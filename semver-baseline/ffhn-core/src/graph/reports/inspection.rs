//! Public documents for read-only graph inspection operations.

use serde::Serialize;

use crate::CoreError;
use crate::graph::{GraphListResult, GraphStatusResult, GraphValidationResult};

/// Schema name for one v11 source status report.
pub const GRAPH_SOURCE_STATUS_REPORT_SCHEMA_NAME: &str = "ffhn.source_status_report";
/// Schema version for one v11 source status report.
pub const GRAPH_SOURCE_STATUS_REPORT_SCHEMA_VERSION: u32 = 1;
/// Schema name for one v11 measurement status report.
pub const GRAPH_MEASUREMENT_STATUS_REPORT_SCHEMA_NAME: &str = "ffhn.measurement_status_report";
/// Schema version for one v11 measurement status report.
pub const GRAPH_MEASUREMENT_STATUS_REPORT_SCHEMA_VERSION: u32 = 1;
/// Schema name for one v11 configuration validation report.
pub const GRAPH_VALIDATE_REPORT_SCHEMA_NAME: &str = "ffhn.validate_report";
/// Schema version for one v11 configuration validation report.
pub const GRAPH_VALIDATE_REPORT_SCHEMA_VERSION: u32 = 1;
/// Schema name for one v11 graph listing report.
pub const GRAPH_LIST_REPORT_SCHEMA_NAME: &str = "ffhn.list_report";
/// Schema version for one v11 graph listing report.
pub const GRAPH_LIST_REPORT_SCHEMA_VERSION: u32 = 1;

/// Public stable snapshot of source and measurement lineage state.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSourceStatusReport {
    schema_name: String,
    schema_version: u32,
    source_id: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    config_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pending_manifest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lineage_refusal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_due_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_health: Option<crate::graph::SourceAcquisitionHealth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_fault_episode: Option<crate::graph::IntegrationFaultEpisode>,
    outbox_overflow: Vec<crate::graph::OutboxOverflowFact>,
    measurements: Vec<MeasurementStatusFact>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct MeasurementStatusFact {
    measurement_id: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    config_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lineage_hold: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stored_measurement_value_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_measurement_value_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observation_seq: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extraction_health: Option<crate::graph::MeasurementExtractionHealth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_fault_episode: Option<crate::graph::IntegrationFaultEpisode>,
    outbox_overflow: Vec<crate::graph::OutboxOverflowFact>,
}

/// Public status document for one configured, authoritative, or artifact-backed measurement.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphMeasurementStatusReport {
    schema_name: String,
    schema_version: u32,
    source_id: String,
    measurement: MeasurementStatusFact,
}

/// Public offline validation result with one Stage-A or Stage-B fact per checked scope.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphValidateReport {
    schema_name: String,
    schema_version: u32,
    valid: bool,
    checks: Vec<GraphValidationReportItem>,
}

/// One public validation fact.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphValidationReportItem {
    source_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    measurement_id: Option<String>,
    scope: String,
    valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

/// Public stable graph configuration listing.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphListReport {
    schema_name: String,
    schema_version: u32,
    scope: String,
    entries: Vec<GraphListReportItem>,
}

/// One public list entry.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphListReportItem {
    source_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    measurement_id: Option<String>,
}

impl From<&GraphStatusResult> for GraphSourceStatusReport {
    fn from(result: &GraphStatusResult) -> Self {
        Self {
            schema_name: GRAPH_SOURCE_STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: GRAPH_SOURCE_STATUS_REPORT_SCHEMA_VERSION,
            source_id: result.source().source_id().as_str().to_owned(),
            status: result.source().kind().as_str().to_owned(),
            config_error: result.source().config_error().map(ToOwned::to_owned),
            pending_manifest: result
                .source()
                .pending_manifest()
                .map(|kind| kind.as_str().to_owned()),
            lineage_refusal: result
                .source()
                .lineage_refusal()
                .map(|reason| reason.as_str().to_owned()),
            generation: result.source().generation(),
            next_due_utc: result.source().next_due_utc().map(ToOwned::to_owned),
            source_health: result.source().source_health().cloned(),
            integration_fault_episode: result.source().integration_fault_episode().cloned(),
            outbox_overflow: result.source().outbox_overflow().to_vec(),
            measurements: result.measurements().iter().map(measurement_fact).collect(),
        }
    }
}

impl TryFrom<&GraphStatusResult> for GraphMeasurementStatusReport {
    type Error = CoreError;

    fn try_from(result: &GraphStatusResult) -> Result<Self, Self::Error> {
        let measurement = result.measurements().first().ok_or_else(|| {
            CoreError::contract("selected measurement has no configured or durable status")
        })?;
        if result.measurements().len() != 1 {
            return Err(CoreError::internal(
                "measurement status selection returned more than one measurement",
            ));
        }
        Ok(Self {
            schema_name: GRAPH_MEASUREMENT_STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: GRAPH_MEASUREMENT_STATUS_REPORT_SCHEMA_VERSION,
            source_id: result.source().source_id().as_str().to_owned(),
            measurement: measurement_fact(measurement),
        })
    }
}

fn measurement_fact(
    measurement: &crate::graph::GraphMeasurementStatusResult,
) -> MeasurementStatusFact {
    MeasurementStatusFact {
        measurement_id: measurement.measurement_id().as_str().to_owned(),
        status: measurement.kind().as_str().to_owned(),
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
        observation_seq: measurement.observation_seq(),
        extraction_health: measurement.extraction_health().cloned(),
        integration_fault_episode: measurement.integration_fault_episode().cloned(),
        outbox_overflow: measurement.outbox_overflow().to_vec(),
    }
}

impl From<&GraphValidationResult> for GraphValidateReport {
    fn from(result: &GraphValidationResult) -> Self {
        Self {
            schema_name: GRAPH_VALIDATE_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: GRAPH_VALIDATE_REPORT_SCHEMA_VERSION,
            valid: result.is_valid(),
            checks: result
                .issues()
                .iter()
                .map(|issue| GraphValidationReportItem {
                    source_id: issue.source_id().as_str().to_owned(),
                    measurement_id: issue.measurement_id().map(|id| id.as_str().to_owned()),
                    scope: issue.scope().as_str().to_owned(),
                    valid: issue.is_valid(),
                    message: issue.message().map(ToOwned::to_owned),
                })
                .collect(),
        }
    }
}

impl From<&GraphListResult> for GraphListReport {
    fn from(result: &GraphListResult) -> Self {
        Self {
            schema_name: GRAPH_LIST_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: GRAPH_LIST_REPORT_SCHEMA_VERSION,
            scope: result.scope().as_str().to_owned(),
            entries: result
                .items()
                .iter()
                .map(|item| GraphListReportItem {
                    source_id: item.source_id().as_str().to_owned(),
                    measurement_id: item.measurement_id().map(|id| id.as_str().to_owned()),
                })
                .collect(),
        }
    }
}

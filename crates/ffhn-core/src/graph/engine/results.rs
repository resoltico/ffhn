//! Construction of the compact route-independent source-cycle result model.

use crate::ConditionEvaluation;

use super::super::{EventEnvelope, MeasurementId, OutboxOverflowFact, SourceId};
use super::{
    GraphMeasureResult, GraphMeasurementResult, GraphMeasurementStatus, GraphSourceStatus,
};

impl GraphSourceStatus {
    /// Returns the stable report spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Locked => "skipped_locked",
            Self::UnresolvableManifest => "unresolvable_manifest",
            Self::ConfigInvalid => "config_invalid",
            Self::LineageRefused => "lineage_refused",
            Self::AcquisitionHold => "acquisition_hold",
            Self::Document => "document",
            Self::NotModified => "not_modified",
            Self::FetchFailed => "fetch_failed",
            Self::IntegrationFault => "integration_fault",
        }
    }
}

impl GraphMeasurementStatus {
    /// Returns the stable report spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initialized => "initialized",
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
            Self::ExtractionFailed => "extraction_failed",
            Self::IntegrationFault => "integration_fault",
            Self::Quarantined => "quarantined",
            Self::LineageHeld => "lineage_held",
            Self::ConfigInvalid => "config_invalid",
            Self::Disabled => "disabled",
            Self::NotModified => "not_modified",
        }
    }
}

pub(super) fn result(
    source_id: SourceId,
    status: GraphSourceStatus,
    measurements: Vec<GraphMeasurementResult>,
) -> GraphMeasureResult {
    GraphMeasureResult {
        source_id,
        status,
        measurements,
        source_event_envelopes: Vec::new(),
        source_outbox_overflow: Vec::new(),
        source_failure: None,
        source_config_error: None,
        unresolvable_manifest: None,
        source_lineage_refusal: None,
        source_health: None,
        source_integration_fault_episode: None,
    }
}

pub(super) fn result_with_source_events(
    source_id: SourceId,
    status: GraphSourceStatus,
    measurements: Vec<GraphMeasurementResult>,
    source_event_envelopes: Vec<EventEnvelope>,
    source_outbox_overflow: Vec<OutboxOverflowFact>,
    source_failure: Option<super::super::SourceFetchFailure>,
) -> GraphMeasureResult {
    GraphMeasureResult {
        source_id,
        status,
        measurements,
        source_event_envelopes,
        source_outbox_overflow,
        source_failure,
        source_config_error: None,
        unresolvable_manifest: None,
        source_lineage_refusal: None,
        source_health: None,
        source_integration_fault_episode: None,
    }
}

pub(super) fn measurement(
    id: MeasurementId,
    status: GraphMeasurementStatus,
    policy_evaluations: Vec<ConditionEvaluation>,
) -> GraphMeasurementResult {
    GraphMeasurementResult {
        measurement_id: id,
        status,
        policy_evaluations,
        event_envelopes: Vec::new(),
        outbox_overflow: Vec::new(),
        config_error: None,
        lineage_hold: None,
        stored_measurement_value_digest: None,
        current_measurement_value_digest: None,
        extraction_health: None,
        integration_fault_episode: None,
        observation: None,
    }
}

pub(super) fn measurement_with_events(
    id: MeasurementId,
    status: GraphMeasurementStatus,
    policy_evaluations: Vec<ConditionEvaluation>,
    event_envelopes: Vec<EventEnvelope>,
    outbox_overflow: Vec<OutboxOverflowFact>,
) -> GraphMeasurementResult {
    GraphMeasurementResult {
        measurement_id: id,
        status,
        policy_evaluations,
        event_envelopes,
        outbox_overflow,
        config_error: None,
        lineage_hold: None,
        stored_measurement_value_digest: None,
        current_measurement_value_digest: None,
        extraction_health: None,
        integration_fault_episode: None,
        observation: None,
    }
}

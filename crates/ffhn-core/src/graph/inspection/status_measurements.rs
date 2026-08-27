//! Measurement-local status classification over configuration, MVD, and lineage evidence.

use crate::CoreError;

use super::super::{
    LineageInspection, MeasurementId, MeasurementLineage, PreparedMeasurementProjection,
    SourceDocument, TrustedSourceDir,
};
use super::{GraphMeasurementStatusKind, GraphMeasurementStatusResult};

pub(super) fn status_measurements(
    source: &TrustedSourceDir,
    inspection: &LineageInspection,
    configured: &[MeasurementId],
    selected: Option<MeasurementId>,
    source_document: Option<&SourceDocument>,
) -> Vec<GraphMeasurementStatusResult> {
    let mut results = inspection
        .measurements()
        .iter()
        .filter(|(id, _)| selected.as_ref().is_none_or(|selected| selected == *id))
        .map(|(id, lineage)| {
            let configured = configured.contains(id);
            let (config_error, current_mvd) =
                configuration_evidence(source, id, configured, source_document);
            let stored_mvd = match lineage {
                MeasurementLineage::Ready(state) => {
                    state.measurement_value_digest().map(ToOwned::to_owned)
                }
                MeasurementLineage::NeverInitialized | MeasurementLineage::Held(_) => None,
            };
            let (kind, observation_seq, lineage_hold) = classify(
                lineage,
                configured,
                config_error.is_some(),
                stored_mvd.as_deref(),
                current_mvd.as_deref(),
            );
            let (extraction_health, integration_fault_episode) = match lineage {
                MeasurementLineage::Ready(state) => (
                    Some(state.extraction_health().clone()),
                    state.integration_fault_episode().cloned(),
                ),
                MeasurementLineage::NeverInitialized | MeasurementLineage::Held(_) => (None, None),
            };
            GraphMeasurementStatusResult {
                measurement_id: id.clone(),
                kind,
                config_error,
                lineage_hold,
                stored_measurement_value_digest: stored_mvd,
                current_measurement_value_digest: current_mvd,
                observation_seq,
                extraction_health,
                integration_fault_episode,
                outbox_overflow: match lineage {
                    MeasurementLineage::Ready(state) => state.outbox_overflow().to_vec(),
                    MeasurementLineage::NeverInitialized | MeasurementLineage::Held(_) => {
                        Vec::new()
                    }
                },
            }
        })
        .collect::<Vec<_>>();
    if results.is_empty()
        && let Some(measurement_id) = selected
    {
        results.push(GraphMeasurementStatusResult {
            measurement_id,
            kind: GraphMeasurementStatusKind::NotConfigured,
            config_error: None,
            lineage_hold: None,
            stored_measurement_value_digest: None,
            current_measurement_value_digest: None,
            observation_seq: None,
            extraction_health: None,
            integration_fault_episode: None,
            outbox_overflow: Vec::new(),
        });
    }
    results
}

pub(super) fn configuration_evidence(
    source: &TrustedSourceDir,
    id: &MeasurementId,
    configured: bool,
    source_document: Option<&SourceDocument>,
) -> (Option<String>, Option<String>) {
    if !configured {
        return (None, None);
    }
    match source.read_measurement_document(id).and_then(|document| {
        PreparedMeasurementProjection::prepare(&document)?;
        Ok(document)
    }) {
        Err(error) => (Some(error.to_string()), None),
        Ok(document) => match source_document {
            Some(source_document) => {
                digest_evidence(document.measurement_value_digest(source_document))
            }
            None => (None, None),
        },
    }
}

pub(super) fn digest_evidence(
    digest: Result<String, CoreError>,
) -> (Option<String>, Option<String>) {
    match digest {
        Ok(digest) => (None, Some(digest)),
        Err(error) => (Some(error.to_string()), None),
    }
}

pub(super) fn classify(
    lineage: &MeasurementLineage,
    configured: bool,
    config_invalid: bool,
    stored_mvd: Option<&str>,
    current_mvd: Option<&str>,
) -> (
    GraphMeasurementStatusKind,
    Option<u64>,
    Option<super::super::MeasurementLineageHold>,
) {
    match lineage {
        MeasurementLineage::Ready(state) if !configured => (
            GraphMeasurementStatusKind::NotConfigured,
            (state.observation_seq() > 0).then_some(state.observation_seq()),
            None,
        ),
        MeasurementLineage::Ready(state) if config_invalid => (
            GraphMeasurementStatusKind::ConfigInvalid,
            (state.observation_seq() > 0).then_some(state.observation_seq()),
            None,
        ),
        MeasurementLineage::Ready(state) if state.observation_seq() == 0 => {
            (GraphMeasurementStatusKind::NeverInitialized, None, None)
        }
        MeasurementLineage::Ready(state) if stored_mvd.is_some() && stored_mvd != current_mvd => (
            GraphMeasurementStatusKind::Quarantined,
            Some(state.observation_seq()),
            None,
        ),
        MeasurementLineage::Ready(state) => (
            GraphMeasurementStatusKind::Ready,
            Some(state.observation_seq()),
            None,
        ),
        MeasurementLineage::NeverInitialized if config_invalid => {
            (GraphMeasurementStatusKind::ConfigInvalid, None, None)
        }
        MeasurementLineage::NeverInitialized => {
            (GraphMeasurementStatusKind::NeverInitialized, None, None)
        }
        MeasurementLineage::Held(hold) => {
            (GraphMeasurementStatusKind::LineageHeld, None, Some(*hold))
        }
    }
}

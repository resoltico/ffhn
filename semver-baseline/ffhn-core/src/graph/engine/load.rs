//! Configuration loading and disabled-result construction for one source cycle.

use super::super::{MeasurementId, PreparedMeasurementProjection, TrustedSourceDir};
use super::{GraphMeasurementResult, GraphMeasurementStatus, measurement};

pub(super) fn load(source: &TrustedSourceDir, id: &MeasurementId) -> super::LoadedMeasurement {
    (
        id.clone(),
        source
            .read_measurement_document(id)
            .and_then(|doc| PreparedMeasurementProjection::prepare(&doc).map(|plan| (doc, plan))),
    )
}

pub(super) fn disabled(loaded: Vec<super::LoadedMeasurement>) -> Vec<GraphMeasurementResult> {
    loaded
        .into_iter()
        .map(|(id, loaded)| {
            let mut result = measurement(id, GraphMeasurementStatus::Disabled, Vec::new());
            if let Err(error) = loaded {
                result.config_error = Some(error.to_string());
            }
            result
        })
        .collect()
}

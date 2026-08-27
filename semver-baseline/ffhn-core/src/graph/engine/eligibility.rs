//! Measurement eligibility, source initialization, and route-independent state evidence.

use crate::CoreError;

use super::super::{
    LineageManifest, LineageManifestParts, LineageScope, MeasurementDocument, MeasurementId,
    MeasurementLineage, MeasurementState, PreparedMeasurementProjection, SourceDocument,
    SourceLineage, TrustedSourceDir,
};
use super::{GraphMeasureResult, GraphMeasurementResult, GraphMeasurementStatus, measurement};

pub(super) struct Eligible {
    pub(super) id: MeasurementId,
    pub(super) document: MeasurementDocument,
    pub(super) projection: PreparedMeasurementProjection,
    pub(super) state: Option<MeasurementState>,
    pub(super) mvd: String,
}

pub(super) enum SourceContext {
    Ready {
        identity: Box<super::super::SourceIdentity>,
        state: Box<super::super::SourceState>,
        authoritative_lineage: bool,
    },
    Refused(super::super::SourceLineageRefusal),
}

pub(super) fn source_context(
    inspection: &super::super::LineageInspection,
    persist: bool,
) -> Result<SourceContext, CoreError> {
    match inspection.source() {
        SourceLineage::Ready(ready) => Ok(SourceContext::Ready {
            identity: Box::new(ready.identity().clone()),
            state: Box::new(ready.state().clone()),
            authoritative_lineage: true,
        }),
        SourceLineage::Refused(reason) => Ok(SourceContext::Refused(*reason)),
        SourceLineage::NeedsInitialization if !persist => {
            let identity = super::super::SourceIdentity::fresh();
            let state = super::super::SourceState::fresh(identity.source_instance_id().clone());
            Ok(SourceContext::Ready {
                identity: Box::new(identity),
                state: Box::new(state),
                authoritative_lineage: false,
            })
        }
        SourceLineage::NeedsInitialization => Err(CoreError::internal(
            "source initialization did not establish lineage",
        )),
    }
}

pub(super) fn initialize(
    source: &TrustedSourceDir,
    ids: &[MeasurementId],
) -> Result<super::super::LineageInspection, CoreError> {
    let inspection = source.inspect_lineage(ids.iter().cloned())?;
    if !matches!(inspection.source(), SourceLineage::NeedsInitialization) {
        return Ok(inspection);
    }
    let manifest = LineageManifest::new(LineageManifestParts {
        source_id: source.paths().source_id().clone(),
        scope: LineageScope::Init,
        from: None,
        target: super::super::SourceIdentity::fresh(),
    })?;
    source.apply_source_transition(&manifest)?;
    source.inspect_lineage(ids.iter().cloned())
}

pub(super) fn with_source_state(
    mut result: GraphMeasureResult,
    state: &super::super::SourceState,
) -> GraphMeasureResult {
    result.source_health = Some(state.source_health().clone());
    result.source_integration_fault_episode = state.integration_fault_episode().cloned();
    result
}

pub(super) fn eligible(
    source: &SourceDocument,
    inspection: &super::super::LineageInspection,
    loaded: Vec<super::LoadedMeasurement>,
) -> Result<(Vec<Eligible>, Vec<GraphMeasurementResult>), CoreError> {
    let mut eligible = Vec::new();
    let mut outcomes = Vec::new();
    for (id, loaded) in loaded {
        let (document, projection) = match loaded {
            Ok(loaded) => loaded,
            Err(error) => {
                let mut outcome =
                    measurement(id, GraphMeasurementStatus::ConfigInvalid, Vec::new());
                outcome.config_error = Some(error.to_string());
                outcomes.push(outcome);
                continue;
            }
        };
        if !document.enabled() {
            outcomes.push(measurement(
                id,
                GraphMeasurementStatus::Disabled,
                Vec::new(),
            ));
            continue;
        }
        let mvd = document.measurement_value_digest(source)?;
        match inspection.measurement(&id) {
            Some(MeasurementLineage::Held(hold)) => {
                let mut outcome = measurement(id, GraphMeasurementStatus::LineageHeld, Vec::new());
                outcome.lineage_hold = Some(*hold);
                outcome.current_measurement_value_digest = Some(mvd);
                outcomes.push(outcome);
            }
            Some(MeasurementLineage::Ready(state))
                if state
                    .measurement_value_digest()
                    .is_some_and(|stored| stored != mvd) =>
            {
                let mut outcome = measurement(id, GraphMeasurementStatus::Quarantined, Vec::new());
                outcome.stored_measurement_value_digest =
                    state.measurement_value_digest().map(ToOwned::to_owned);
                outcome.current_measurement_value_digest = Some(mvd);
                outcome.extraction_health = Some(state.extraction_health().clone());
                outcome.integration_fault_episode = state.integration_fault_episode().cloned();
                outcomes.push(outcome);
            }
            Some(MeasurementLineage::Ready(state)) => eligible.push(Eligible {
                id,
                document,
                projection,
                state: Some((**state).clone()),
                mvd,
            }),
            Some(MeasurementLineage::NeverInitialized) | None => eligible.push(Eligible {
                id,
                document,
                projection,
                state: None,
                mvd,
            }),
        }
    }
    Ok((eligible, outcomes))
}

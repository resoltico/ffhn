use super::super::*;
use crate::graph::{MeasurementLineageHold, SourceLineageRefusal, UnresolvableManifest};

#[test]
fn result_accessors_preserve_nondefault_lineage_and_measurement_facts() {
    let source_id = SourceId::new("source-contract").expect("source id");
    let mut result = results::result(source_id, GraphSourceStatus::LineageRefused, Vec::new());
    result.unresolvable_manifest = Some(UnresolvableManifest::Commit);
    result.source_lineage_refusal = Some(SourceLineageRefusal::SourceStateUnreadable);
    assert_eq!(
        result.unresolvable_manifest(),
        Some(UnresolvableManifest::Commit)
    );
    assert_eq!(
        result.source_lineage_refusal(),
        Some(SourceLineageRefusal::SourceStateUnreadable)
    );

    let measurement_id = MeasurementId::new("measurement-contract").expect("measurement id");
    let mut measurement =
        results::measurement(measurement_id, GraphMeasurementStatus::Disabled, Vec::new());
    measurement.lineage_hold = Some(MeasurementLineageHold::ArtifactUnreadable);
    measurement.stored_measurement_value_digest = Some("a".repeat(64));
    measurement.integration_fault_episode = Some(
        crate::graph::IntegrationFaultEpisode::new(
            crate::graph::GraphIntegrationFaultCode::HtmlcutInternalError,
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("integration fault episode"),
    );
    assert_eq!(measurement.status(), GraphMeasurementStatus::Disabled);
    assert_eq!(
        measurement.lineage_hold(),
        Some(MeasurementLineageHold::ArtifactUnreadable)
    );
    assert_eq!(
        measurement.stored_measurement_value_digest(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert_eq!(
        measurement
            .integration_fault_episode()
            .expect("integration fault")
            .code(),
        crate::graph::GraphIntegrationFaultCode::HtmlcutInternalError
    );
}

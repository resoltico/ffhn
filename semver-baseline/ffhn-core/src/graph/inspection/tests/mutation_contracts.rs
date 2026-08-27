use super::super::*;

fn overflow() -> super::super::super::OutboxOverflowFact {
    serde_json::from_value(serde_json::json!({
        "event_id": "a".repeat(64),
        "event_kind": "source_escalation",
        "route_id": "alert",
        "route_family": "on_source"
    }))
    .expect("valid overflow fact")
}

#[test]
fn result_accessors_preserve_nondefault_status_and_validation_facts() {
    let source_id = SourceId::new("source-contract").expect("source id");
    let measurement_id = MeasurementId::new("measurement-contract").expect("measurement id");
    let source = GraphSourceStatusResult {
        source_id: source_id.clone(),
        kind: GraphSourceStatusKind::Ready,
        config_error: Some("source diagnostic".to_owned()),
        pending_manifest: None,
        lineage_refusal: None,
        generation: Some(7),
        next_due_utc: Some("2026-08-25T00:00:07Z".to_owned()),
        source_health: Some(super::super::super::SourceAcquisitionHealth::healthy()),
        integration_fault_episode: Some(
            super::super::super::IntegrationFaultEpisode::new(
                super::super::super::GraphIntegrationFaultCode::HtmlcutInternalError,
                "2026-08-25T00:00:00Z".to_owned(),
            )
            .expect("integration fault"),
        ),
        outbox_overflow: vec![overflow()],
    };
    let mut extraction_health = super::super::super::MeasurementExtractionHealth::healthy();
    extraction_health
        .observe(
            super::super::super::ExtractionFailureReason::JsonMalformed,
            "2026-08-25T00:00:00Z",
            2,
        )
        .expect("extraction failure");
    let measurement = GraphMeasurementStatusResult {
        measurement_id: measurement_id.clone(),
        kind: GraphMeasurementStatusKind::LineageHeld,
        config_error: Some("measurement diagnostic".to_owned()),
        lineage_hold: Some(super::super::super::MeasurementLineageHold::ArtifactUnreadable),
        stored_measurement_value_digest: Some("a".repeat(64)),
        current_measurement_value_digest: Some("b".repeat(64)),
        observation_seq: Some(9),
        extraction_health: Some(extraction_health),
        integration_fault_episode: Some(
            super::super::super::IntegrationFaultEpisode::new(
                super::super::super::GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation,
                "2026-08-25T00:00:01Z".to_owned(),
            )
            .expect("measurement integration fault"),
        ),
        outbox_overflow: vec![overflow()],
    };
    let result = GraphStatusResult {
        source,
        measurements: vec![measurement],
    };

    assert_eq!(result.source().source_id(), &source_id);
    assert_eq!(
        result
            .source()
            .integration_fault_episode()
            .expect("source integration fault")
            .code(),
        super::super::super::GraphIntegrationFaultCode::HtmlcutInternalError
    );
    assert_eq!(result.source().outbox_overflow().len(), 1);
    assert_eq!(result.measurements().len(), 1);
    let measurement = &result.measurements()[0];
    assert_eq!(measurement.measurement_id(), &measurement_id);
    assert_eq!(measurement.kind(), GraphMeasurementStatusKind::LineageHeld);
    assert_eq!(measurement.config_error(), Some("measurement diagnostic"));
    assert_eq!(
        measurement.lineage_hold(),
        Some(super::super::super::MeasurementLineageHold::ArtifactUnreadable)
    );
    assert_eq!(measurement.observation_seq(), Some(9));
    assert_eq!(
        measurement
            .extraction_health()
            .expect("extraction health")
            .reason(),
        Some(super::super::super::ExtractionFailureReason::JsonMalformed)
    );
    assert_eq!(
        measurement
            .integration_fault_episode()
            .expect("integration fault")
            .code(),
        super::super::super::GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation
    );
    assert_eq!(measurement.outbox_overflow().len(), 1);

    let issue = GraphValidationIssue {
        source_id: source_id.clone(),
        measurement_id: Some(measurement_id.clone()),
        scope: GraphValidationScope::Measurement,
        message: Some("invalid projection".to_owned()),
    };
    assert_eq!(GraphValidationScope::Source.as_str(), "source");
    assert_eq!(GraphValidationScope::Measurement.as_str(), "measurement");
    assert_eq!(issue.source_id(), &source_id);
    assert_eq!(issue.measurement_id(), Some(&measurement_id));
}

#[test]
fn status_lock_retry_boundary_honors_every_attempt_exactly() {
    assert!(status_lock_retry_remaining(0));
    assert!(status_lock_retry_remaining(3));
    assert!(!status_lock_retry_remaining(4));
}

use std::fs;

use super::*;
use crate::graph::{GraphPaths, MeasurementDocument, SourceDocument, TrustedGraphRoot};

const SOURCE: &str = r#"
schema_name = "ffhn.source"
schema_version = 1
source_id = "shop"
display_name = "Shop"
enabled = true
escalate_after = 2
[fetch]
engine = "file"
file_path = "/tmp/shop.json"
max_bytes = 1024
[conditional]
enabled = false
[schedule]
interval_ms = 1000
min_interval_ms = 1000
"#;

const MEASUREMENT: &str = r#"
schema_name = "ffhn.measurement"
schema_version = 1
measurement_id = "price"
display_name = "Price"
enabled = true
escalate_after = 2
declared_type = "integer"
conditions = []
[projection]
kind = "json_pointer"
pointer = "/price"
"#;

fn source_toml(path: &str) -> String {
    SOURCE.replace("/tmp/shop.json", &path.replace('\\', "\\\\"))
}

fn graph() -> TrustedGraphRoot {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.keep();
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(root.join("graph")),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    let source: SourceDocument = toml::from_str(&source_toml(
        &crate::graph::test_support::absolute_file_path("shop.json"),
    ))
    .expect("source");
    let source = graph.create_source_document(&source).expect("source");
    let measurement: MeasurementDocument = toml::from_str(MEASUREMENT).expect("measurement");
    source
        .create_measurement_document(&measurement)
        .expect("measurement");
    graph
}

#[test]
fn offline_listing_validation_and_status_do_not_mint_lineage() {
    let graph = graph();
    let source_id = SourceId::new("shop").expect("source id");
    let source = graph.open_source(source_id.clone()).expect("source");
    assert_eq!(
        list_graph(&graph, GraphListScope::Sources)
            .expect("source listing")
            .items()
            .len(),
        1
    );
    assert_eq!(
        list_graph(&graph, GraphListScope::Measurements)
            .expect("measurement listing")
            .items()[0]
            .measurement_id()
            .expect("measurement id")
            .as_str(),
        "price"
    );
    assert!(validate_graph(&graph, None).expect("validation").is_valid());
    let status = status_source(&graph, source_id, None).expect("status");
    assert_eq!(status.source().kind(), GraphSourceStatusKind::Uninitialized);
    assert_eq!(
        status.measurements()[0].kind(),
        GraphMeasurementStatusKind::NeverInitialized
    );
    assert!(source.read_identity().expect("identity read").is_none());
}

#[test]
fn validation_reports_measurement_failure_without_touching_state() {
    let graph = graph();
    let source_id = SourceId::new("shop").expect("source id");
    let source = graph.open_source(source_id.clone()).expect("source");
    fs::write(
        source
            .paths()
            .measurement_file(&MeasurementId::new("price").expect("measurement")),
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\n",
    )
    .expect("invalid measurement");
    let result = validate_graph(&graph, Some(source_id)).expect("validation");
    assert!(!result.is_valid());
    assert!(
        result
            .issues()
            .iter()
            .any(|issue| issue.scope() == GraphValidationScope::Measurement && !issue.is_valid())
    );
    assert!(source.read_identity().expect("identity read").is_none());
}

#[test]
fn status_treats_an_unreadable_manifest_as_pending_without_attempting_recovery() {
    let graph = graph();
    let source_id = SourceId::new("shop").expect("source id");
    let source = graph.open_source(source_id.clone()).expect("source");
    fs::write(source.paths().lineage_manifest_file(), "not json").expect("broken manifest");
    let status = status_source(&graph, source_id, None).expect("pending status");
    assert_eq!(status.source().kind(), GraphSourceStatusKind::Pending);
    assert_eq!(
        status.source().pending_manifest(),
        Some(super::super::UnresolvableManifest::Lineage)
    );
    assert!(source.read_identity().expect("identity").is_none());
}

#[test]
fn status_reports_measurement_value_quarantine_with_both_digests() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let source_file = temporary.path().join("source.json");
    fs::write(&source_file, r#"{"price":7,"other":8}"#).expect("source body");
    let graph =
        TrustedGraphRoot::initialize(GraphPaths::new(root), "2026-08-25T00:00:00Z".to_owned())
            .expect("graph");
    let source: SourceDocument =
        toml::from_str(&source_toml(&source_file.to_string_lossy())).expect("source");
    let source_id = source.source_id().clone();
    let source = graph.create_source_document(&source).expect("source");
    let measurement: MeasurementDocument = toml::from_str(MEASUREMENT).expect("measurement");
    source
        .create_measurement_document(&measurement)
        .expect("measurement");
    super::super::measure_source_once(&graph, source_id.clone()).expect("initial measurement");
    fs::write(
        source
            .paths()
            .measurement_file(&MeasurementId::new("price").expect("measurement id")),
        MEASUREMENT.replace("/price", "/other"),
    )
    .expect("changed measurement");
    let status = status_source(&graph, source_id, None).expect("status");
    let measurement = &status.measurements()[0];
    assert_eq!(measurement.kind(), GraphMeasurementStatusKind::Quarantined);
    assert!(measurement.stored_measurement_value_digest().is_some());
    assert!(measurement.current_measurement_value_digest().is_some());
    assert_ne!(
        measurement.stored_measurement_value_digest(),
        measurement.current_measurement_value_digest()
    );
}

#[test]
fn inspection_value_objects_reports_and_closed_status_vocabularies_are_complete() {
    for (scope, spelling) in [
        (GraphListScope::Sources, "sources"),
        (GraphListScope::Measurements, "measurements"),
    ] {
        assert_eq!(scope.as_str(), spelling);
    }
    for (kind, spelling) in [
        (GraphSourceStatusKind::Pending, "pending"),
        (GraphSourceStatusKind::LineageRefused, "lineage_refused"),
        (GraphSourceStatusKind::Uninitialized, "uninitialized"),
        (GraphSourceStatusKind::Ready, "ready"),
        (GraphSourceStatusKind::ConfigInvalid, "config_invalid"),
    ] {
        assert_eq!(kind.as_str(), spelling);
    }
    for (kind, spelling) in [
        (
            GraphMeasurementStatusKind::NeverInitialized,
            "never_initialized",
        ),
        (GraphMeasurementStatusKind::Ready, "ready"),
        (GraphMeasurementStatusKind::LineageHeld, "lineage_held"),
        (GraphMeasurementStatusKind::NotConfigured, "not_configured"),
        (GraphMeasurementStatusKind::ConfigInvalid, "config_invalid"),
        (GraphMeasurementStatusKind::Quarantined, "quarantined"),
    ] {
        assert_eq!(kind.as_str(), spelling);
    }
    assert_eq!(GraphValidationScope::Source.as_str(), "source");
    assert_eq!(GraphValidationScope::Measurement.as_str(), "measurement");

    let source_id = SourceId::new("source").expect("source");
    let measurement_id = MeasurementId::new("measurement").expect("measurement");
    let item = GraphListItem {
        source_id: source_id.clone(),
        measurement_id: Some(measurement_id.clone()),
    };
    assert_eq!(item.source_id(), &source_id);
    assert_eq!(item.measurement_id(), Some(&measurement_id));
    let list = GraphListResult {
        scope: GraphListScope::Measurements,
        items: vec![item],
    };
    assert_eq!(list.scope(), GraphListScope::Measurements);
    assert_eq!(list.items().len(), 1);
    let list_report = crate::graph::GraphListReport::from(&list);
    assert_eq!(
        serde_json::to_value(list_report).expect("list report")["entries"]
            .as_array()
            .expect("entries")
            .len(),
        1
    );

    let source = GraphSourceStatusResult {
        source_id: source_id.clone(),
        kind: GraphSourceStatusKind::Ready,
        config_error: None,
        pending_manifest: None,
        lineage_refusal: None,
        generation: Some(1),
        next_due_utc: Some("2026-08-25T00:00:01Z".to_owned()),
        source_health: Some(super::super::SourceAcquisitionHealth::healthy()),
        integration_fault_episode: None,
        outbox_overflow: Vec::new(),
    };
    assert_eq!(source.source_id(), &source_id);
    assert_eq!(source.kind(), GraphSourceStatusKind::Ready);
    assert!(source.config_error().is_none());
    assert!(source.pending_manifest().is_none());
    assert!(source.lineage_refusal().is_none());
    assert_eq!(source.generation(), Some(1));
    assert_eq!(source.next_due_utc(), Some("2026-08-25T00:00:01Z"));
    assert!(source.source_health().is_some());
    assert!(source.integration_fault_episode().is_none());
    assert!(source.outbox_overflow().is_empty());

    let measurement = GraphMeasurementStatusResult {
        measurement_id: measurement_id.clone(),
        kind: GraphMeasurementStatusKind::NeverInitialized,
        config_error: None,
        lineage_hold: None,
        stored_measurement_value_digest: None,
        current_measurement_value_digest: None,
        observation_seq: None,
        extraction_health: None,
        integration_fault_episode: None,
        outbox_overflow: Vec::new(),
    };
    assert_eq!(measurement.measurement_id(), &measurement_id);
    assert_eq!(
        measurement.kind(),
        GraphMeasurementStatusKind::NeverInitialized
    );
    assert!(measurement.config_error().is_none());
    assert!(measurement.lineage_hold().is_none());
    assert!(measurement.stored_measurement_value_digest().is_none());
    assert!(measurement.current_measurement_value_digest().is_none());
    assert!(measurement.observation_seq().is_none());
    assert!(measurement.extraction_health().is_none());
    assert!(measurement.integration_fault_episode().is_none());
    assert!(measurement.outbox_overflow().is_empty());

    let status = GraphStatusResult {
        source,
        measurements: vec![measurement.clone()],
    };
    assert_eq!(status.source().source_id(), &source_id);
    assert_eq!(status.measurements().len(), 1);
    assert!(crate::graph::GraphMeasurementStatusReport::try_from(&status).is_ok());
    assert!(serde_json::to_value(crate::graph::GraphSourceStatusReport::from(&status)).is_ok());
    let empty = GraphStatusResult {
        source: status.source().clone(),
        measurements: Vec::new(),
    };
    assert!(crate::graph::GraphMeasurementStatusReport::try_from(&empty).is_err());
    let multiple = GraphStatusResult {
        source: status.source().clone(),
        measurements: vec![measurement.clone(), measurement],
    };
    assert!(crate::graph::GraphMeasurementStatusReport::try_from(&multiple).is_err());

    let valid_issue = GraphValidationIssue {
        source_id: source_id.clone(),
        measurement_id: None,
        scope: GraphValidationScope::Source,
        message: None,
    };
    assert_eq!(valid_issue.source_id(), &source_id);
    assert!(valid_issue.measurement_id().is_none());
    assert_eq!(valid_issue.scope(), GraphValidationScope::Source);
    assert!(valid_issue.message().is_none());
    assert!(valid_issue.is_valid());
    let invalid_issue = GraphValidationIssue {
        source_id,
        measurement_id: Some(measurement_id),
        scope: GraphValidationScope::Measurement,
        message: Some("invalid".to_owned()),
    };
    let validation = GraphValidationResult {
        issues: vec![valid_issue, invalid_issue],
    };
    assert!(!validation.is_valid());
    assert_eq!(validation.issues().len(), 2);
    assert!(serde_json::to_value(crate::graph::GraphValidateReport::from(&validation)).is_ok());
}

#[test]
fn measurement_status_classifier_covers_ready_unconfigured_invalid_quarantined_and_held_rows() {
    let source_instance = super::super::SourceInstanceId::mint();
    let instance = super::super::MeasurementInstanceId::mint();
    let fresh = super::super::MeasurementState::fresh(source_instance.clone(), instance.clone());
    for (lineage, configured, invalid, stored, current, expected) in [
        (
            super::super::MeasurementLineage::Ready(Box::new(fresh.clone())),
            false,
            false,
            None,
            None,
            GraphMeasurementStatusKind::NotConfigured,
        ),
        (
            super::super::MeasurementLineage::Ready(Box::new(fresh.clone())),
            true,
            true,
            None,
            None,
            GraphMeasurementStatusKind::ConfigInvalid,
        ),
        (
            super::super::MeasurementLineage::Ready(Box::new(fresh)),
            true,
            false,
            None,
            None,
            GraphMeasurementStatusKind::NeverInitialized,
        ),
        (
            super::super::MeasurementLineage::NeverInitialized,
            true,
            true,
            None,
            None,
            GraphMeasurementStatusKind::ConfigInvalid,
        ),
        (
            super::super::MeasurementLineage::NeverInitialized,
            true,
            false,
            None,
            None,
            GraphMeasurementStatusKind::NeverInitialized,
        ),
        (
            super::super::MeasurementLineage::Held(
                super::super::MeasurementLineageHold::StateMissing,
            ),
            true,
            false,
            None,
            None,
            GraphMeasurementStatusKind::LineageHeld,
        ),
    ] {
        let classified =
            super::status_measurements::classify(&lineage, configured, invalid, stored, current);
        assert_eq!(classified.0, expected);
        assert_eq!(classified.1, None);
    }

    let measurement: MeasurementDocument = toml::from_str(MEASUREMENT).expect("measurement");
    let mut initialized = super::super::MeasurementState::fresh(source_instance, instance);
    initialized
        .apply_accepted_observation(
            &measurement,
            crate::model::parse_json_scalar_token_for_contract(
                crate::DeclaredType::Integer,
                &crate::TypeParams::default(),
                "1".to_owned(),
            )
            .expect("observation"),
            "a".repeat(64),
        )
        .expect("accepted");
    let lineage = super::super::MeasurementLineage::Ready(Box::new(initialized));
    assert_eq!(
        super::status_measurements::classify(&lineage, false, false, Some("a"), None).1,
        Some(1)
    );
    assert_eq!(
        super::status_measurements::classify(&lineage, true, true, Some("a"), Some("a")).1,
        Some(1)
    );
    assert_eq!(
        super::status_measurements::classify(&lineage, true, false, None, None).0,
        GraphMeasurementStatusKind::Ready
    );
    assert_eq!(
        super::status_measurements::classify(&lineage, true, false, None, Some("b")).0,
        GraphMeasurementStatusKind::Ready
    );
    assert_eq!(
        super::status_measurements::classify(&lineage, true, false, Some("a"), Some("b"),).0,
        GraphMeasurementStatusKind::Quarantined
    );
    assert_eq!(
        super::status_measurements::classify(&lineage, true, false, Some("a"), Some("a"),).0,
        GraphMeasurementStatusKind::Ready
    );
}

#[test]
fn status_busy_commit_pending_unconfigured_and_refused_source_rows_are_complete() {
    let graph = graph();
    let source_id = SourceId::new("shop").expect("source");
    let source = graph.open_source(source_id.clone()).expect("source");
    let lease = source
        .try_acquire_write_lease()
        .expect("lock")
        .expect("lease");
    assert!(status_source(&graph, source_id.clone(), None).is_err());
    drop(lease);

    let missing = status_source(
        &graph,
        source_id.clone(),
        Some(MeasurementId::new("not-configured").expect("measurement")),
    )
    .expect("selected status");
    assert_eq!(
        missing.measurements()[0].kind(),
        GraphMeasurementStatusKind::NotConfigured
    );

    fs::create_dir(source.paths().storage_dir()).expect("storage");
    fs::write(source.paths().commit_manifest_file(), "not-json").expect("commit");
    let pending = status_source(&graph, source_id.clone(), None).expect("pending");
    assert_eq!(pending.source().kind(), GraphSourceStatusKind::Pending);
    assert_eq!(
        pending.source().pending_manifest(),
        Some(super::super::UnresolvableManifest::Commit)
    );
    fs::remove_file(source.paths().commit_manifest_file()).expect("remove commit");
    let refused = status_source(&graph, source_id, None).expect("refused");
    assert_eq!(
        refused.source().kind(),
        GraphSourceStatusKind::LineageRefused
    );
    assert_eq!(
        refused.source().lineage_refusal(),
        Some(super::super::SourceLineageRefusal::StorageWithoutIdentity)
    );
}

#[test]
fn status_and_validation_preserve_invalid_source_and_measurement_inventory_evidence() {
    let graph = graph();
    let source_id = SourceId::new("shop").expect("source");
    let source = graph.open_source(source_id.clone()).expect("source");
    assert_eq!(
        super::status_measurements::configuration_evidence(
            &source,
            &MeasurementId::new("missing").expect("measurement"),
            false,
            None,
        ),
        (None, None)
    );
    assert!(
        super::status_measurements::digest_evidence(Err(CoreError::contract("invalid")))
            .0
            .is_some()
    );
    super::super::reset_source(&graph, source_id.clone()).expect("initialize lineage");
    fs::write(source.paths().source_file(), "schema_name = \"broken\"").expect("bad source");
    let invalid = status_source(&graph, source_id.clone(), None).expect("invalid status");
    assert_eq!(
        invalid.source().kind(),
        GraphSourceStatusKind::ConfigInvalid
    );
    assert!(invalid.source().config_error().is_some());
    let validation = validate_graph(&graph, Some(source_id.clone())).expect("validation");
    assert!(!validation.is_valid());
    assert!(validation.issues().iter().any(|issue| {
        issue.scope() == GraphValidationScope::Measurement
            && issue
                .message()
                .is_some_and(|message| message.contains("withheld"))
    }));

    let price = MeasurementId::new("price").expect("measurement");
    fs::write(
        source.paths().measurement_file(&price),
        "schema_name = \"broken\"",
    )
    .expect("bad measurement");
    assert!(
        super::status_measurements::configuration_evidence(&source, &price, true, None,)
            .0
            .is_some()
    );

    fs::remove_dir_all(source.paths().measurements_dir()).expect("remove measurements");
    fs::write(source.paths().measurements_dir(), "not a directory").expect("bad root");
    let validation = validate_graph(&graph, Some(source_id)).expect("inventory validation");
    assert!(validation.issues().iter().any(|issue| {
        issue.scope() == GraphValidationScope::Measurement && issue.measurement_id().is_none()
    }));
}

#[test]
fn uninitialized_source_with_invalid_configuration_is_config_invalid_not_ready() {
    let graph = graph();
    let source_id = SourceId::new("shop").expect("source");
    let source = graph.open_source(source_id.clone()).expect("source");
    fs::write(source.paths().source_file(), "schema_name = \"broken\"").expect("bad source");
    let status = status_source(&graph, source_id, None).expect("status");
    assert_eq!(status.source().kind(), GraphSourceStatusKind::ConfigInvalid);
    assert!(status.source().generation().is_none());
}

#[path = "tests/mutation_contracts.rs"]
mod mutation_contracts;

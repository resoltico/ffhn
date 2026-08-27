use std::fs;

use tempfile::tempdir;

use super::*;
use crate::graph::{
    GraphPaths, MeasurementDocument, MeasurementId, SourceDocument, TrustedGraphRoot,
};

fn platform_processes(document: &str) -> String {
    let program = crate::graph::test_support::successful_process().program;
    document.replace("/usr/bin/true", &program.replace('\\', "\\\\"))
}

#[test]
fn exact_policy_errors_are_measurement_scoped_integration_faults() {
    assert_eq!(
        super::process::policy_error_code(&CoreError::policy_invariant("proof")),
        crate::graph::GraphIntegrationFaultCode::FfhnPolicyInvariantViolation
    );
    assert_eq!(
        super::process::policy_error_code(&CoreError::internal("policy state")),
        crate::graph::GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation
    );
}

#[test]
fn first_document_cycle_mints_measurement_lineage_only_with_its_state_and_projects_once() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let source_file = temporary.path().join("source.json");
    fs::write(&source_file, r#"{"price":7}"#).expect("source body");
    let graph =
        TrustedGraphRoot::initialize(GraphPaths::new(root), "2026-08-25T00:00:00Z".to_owned())
            .expect("graph");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
        source_file.to_string_lossy(),
    ))
    .expect("source config");
    let source = graph.create_source_document(&source).expect("source");
    let measurement: MeasurementDocument = toml::from_str(&platform_processes(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"price\"\ndisplay_name = \"Price\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\n[[conditions]]\ncondition_id = \"changed\"\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"\n[projection]\nkind = \"json_pointer\"\npointer = \"/price\"\n[outbox]\nmax_pending = 1\nmax_attempts = 2\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n[[routes]]\nroute_id = \"alert\"\nroute_family = \"on_condition\"\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = \"/usr/bin/true\"\ntimeout_ms = 1000\n[[routes]]\nroute_id = \"lifecycle\"\nroute_family = \"on_measurement\"\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = \"/usr/bin/true\"\ntimeout_ms = 1000\n",
    ))
    .expect("measurement");
    source
        .create_measurement_document(&measurement)
        .expect("measurement config");
    let dry = measure_source_dry_run(&graph, SourceId::new("shop").expect("source id"))
        .expect("dry cycle");
    assert_eq!(dry.status(), GraphSourceStatus::Document);
    assert_eq!(
        dry.measurements()[0].status(),
        GraphMeasurementStatus::Initialized
    );
    assert!(dry.source_event_envelopes().is_empty());
    assert!(dry.measurements()[0].event_envelopes().is_empty());
    assert!(source.read_identity().expect("identity read").is_none());
    assert!(source.open_storage().is_err());
    let result =
        measure_source_once(&graph, SourceId::new("shop").expect("source id")).expect("cycle");
    assert_eq!(result.status(), GraphSourceStatus::Document);
    assert_eq!(
        result.source_event_envelopes()[0].event_kind(),
        crate::graph::EventKind::SourceInitialized
    );
    assert_eq!(
        result.measurements()[0].status(),
        GraphMeasurementStatus::Initialized
    );
    assert_eq!(
        result.measurements()[0]
            .observation()
            .expect("reported observation")
            .canonical_value(),
        "7"
    );
    assert_eq!(
        result.measurements()[0].event_envelopes()[0].event_kind(),
        crate::graph::EventKind::MeasurementInitialized
    );
    fs::write(&source_file, r#"{"price": 7}"#).expect("same value new representation");
    let unchanged = measure_source_once(&graph, SourceId::new("shop").expect("source id"))
        .expect("unchanged cycle");
    assert_eq!(
        unchanged.measurements()[0].status(),
        GraphMeasurementStatus::Unchanged
    );
    let not_modified = measure_source_once(&graph, SourceId::new("shop").expect("source id"))
        .expect("not-modified cycle");
    assert_eq!(not_modified.status(), GraphSourceStatus::NotModified);
    let dry_not_modified =
        measure_source_dry_run(&graph, SourceId::new("shop").expect("source id"))
            .expect("dry not-modified cycle");
    assert_eq!(dry_not_modified.status(), GraphSourceStatus::NotModified);
    let identity = source
        .read_identity()
        .expect("identity")
        .expect("identity present");
    let price = MeasurementId::new("price").expect("measurement id");
    assert!(identity.measurements().contains_key(&price));
    assert!(
        source
            .open_storage()
            .expect("storage")
            .read_measurement_state(&price)
            .expect("state read")
            .is_some()
    );
    fs::write(&source_file, r#"{"price":8}"#).expect("changed source body");
    let changed = measure_source_once(&graph, SourceId::new("shop").expect("source id"))
        .expect("changed cycle");
    assert_eq!(
        changed.measurements()[0].status(),
        GraphMeasurementStatus::Changed
    );
    assert_eq!(changed.measurements()[0].event_envelopes().len(), 1);
    assert_eq!(changed.measurements()[0].policy_evaluations().len(), 1);
    assert_eq!(
        changed.measurements()[0].policy_evaluations()[0].condition_id(),
        "changed"
    );
    assert_eq!(changed.measurements()[0].outbox_overflow().len(), 1);
    assert_eq!(
        changed.measurements()[0].event_envelopes()[0].event_kind(),
        crate::graph::EventKind::ConditionSatisfied
    );
    assert_eq!(
        source
            .open_storage()
            .expect("storage")
            .read_measurement_delivery_records(&price)
            .expect("outbox")
            .len(),
        1
    );
    let before_dry = source
        .open_storage()
        .expect("storage")
        .read_measurement_state(&price)
        .expect("state read")
        .expect("state");
    assert_eq!(before_dry.outbox_overflow().len(), 1);
    fs::write(&source_file, r#"{"price":9}"#).expect("second changed source body");
    let dry_changed = measure_source_dry_run(&graph, SourceId::new("shop").expect("source id"))
        .expect("dry changed cycle");
    assert_eq!(dry_changed.measurements()[0].event_envelopes().len(), 1);
    assert_eq!(
        source
            .open_storage()
            .expect("storage")
            .read_measurement_state(&price)
            .expect("state read")
            .expect("state"),
        before_dry
    );
    assert_eq!(
        source
            .open_storage()
            .expect("storage")
            .read_measurement_delivery_records(&price)
            .expect("outbox")
            .len(),
        1
    );
    fs::write(
        source.paths().measurement_file(&price),
        toml::to_string(&measurement)
            .expect("measurement TOML")
            .replace("pointer = \"/price\"", "pointer = \"/other\""),
    )
    .expect("changed measurement");
    let quarantined = measure_source_once(&graph, SourceId::new("shop").expect("source id"))
        .expect("quarantined cycle");
    assert_eq!(quarantined.status(), GraphSourceStatus::AcquisitionHold);
    assert_eq!(
        quarantined.measurements()[0].status(),
        GraphMeasurementStatus::Quarantined
    );
    assert!(
        quarantined.measurements()[0]
            .stored_measurement_value_digest()
            .is_some_and(|digest| digest.len() == 64 && digest.bytes().any(|byte| byte != b'0'))
    );
    fs::write(
        source.paths().measurement_file(&price),
        toml::to_string(&measurement).expect("measurement TOML"),
    )
    .expect("restore measurement");
    let state_path = source
        .paths()
        .measurement_storage_dir(&price)
        .join("state.json");
    let mut held_state: serde_json::Value =
        serde_json::from_slice(&fs::read(&state_path).expect("state bytes")).expect("state JSON");
    held_state["measurement_instance_id"] =
        serde_json::to_value(crate::graph::MeasurementInstanceId::mint()).expect("instance");
    fs::write(
        &state_path,
        format!(
            "{}\n",
            crate::stable_json::stable_json(&held_state).expect("state JSON")
        ),
    )
    .expect("held state");
    let held =
        measure_source_once(&graph, SourceId::new("shop").expect("source id")).expect("held cycle");
    assert_eq!(held.status(), GraphSourceStatus::AcquisitionHold);
    assert_eq!(
        held.measurements()[0].status(),
        GraphMeasurementStatus::LineageHeld
    );
    assert_eq!(
        held.measurements()[0].lineage_hold(),
        Some(crate::graph::MeasurementLineageHold::MeasurementInstanceMismatch)
    );
    super::super::reset_measurement(
        &graph,
        SourceId::new("shop").expect("source id"),
        price.clone(),
    )
    .expect("measurement reset");
    fs::write(&source_file, r#"{"price":10}"#).expect("reinitialized source body");
    let reinitialized = measure_source_once(&graph, SourceId::new("shop").expect("source id"))
        .expect("reinitialized cycle");
    assert_eq!(
        reinitialized.measurements()[0].status(),
        GraphMeasurementStatus::Initialized
    );
    assert_eq!(
        reinitialized.measurements()[0].event_envelopes()[0].event_kind(),
        crate::graph::EventKind::MeasurementInitialized
    );
}

#[test]
fn measurement_reports_skipped_locked_without_converting_contention_into_a_fatal_error() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph =
        TrustedGraphRoot::initialize(GraphPaths::new(root), "2026-08-25T00:00:00Z".to_owned())
            .expect("graph");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
        temporary.path().join("source.json").to_string_lossy(),
    ))
    .expect("source config");
    let source_id = source.source_id().clone();
    let source = graph.create_source_document(&source).expect("source");
    let _lease = source
        .try_acquire_write_lease()
        .expect("source lock")
        .expect("available source lock");
    assert_eq!(
        measure_source_once(&graph, source_id)
            .expect("contention result")
            .status(),
        GraphSourceStatus::Locked
    );
}

#[test]
fn dry_measurement_never_recovers_or_removes_a_staged_artifact() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph =
        TrustedGraphRoot::initialize(GraphPaths::new(root), "2026-08-25T00:00:00Z".to_owned())
            .expect("graph");
    let source_file = temporary.path().join("source.json");
    fs::write(&source_file, r#"{"price":7}"#).expect("source body");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
        source_file.to_string_lossy(),
    ))
    .expect("source config");
    let source_id = source.source_id().clone();
    let source = graph.create_source_document(&source).expect("source");
    let stale = source.paths().storage_dir().join("staged/abandoned.json");
    fs::create_dir_all(stale.parent().expect("staged parent")).expect("staged parent");
    fs::write(&stale, "orphaned").expect("staged bytes");
    assert_eq!(
        measure_source_dry_run(&graph, source_id)
            .expect("dry result")
            .status(),
        GraphSourceStatus::LineageRefused
    );
    assert!(stale.exists());
}

#[test]
fn invalid_source_or_measurement_configuration_is_a_structured_non_mutating_cycle_result() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph =
        TrustedGraphRoot::initialize(GraphPaths::new(root), "2026-08-25T00:00:00Z".to_owned())
            .expect("graph");
    let source_directory = graph
        .paths()
        .source(SourceId::new("shop").expect("source id"));
    fs::create_dir_all(source_directory.source_dir()).expect("source directory");
    fs::write(source_directory.source_file(), "schema_name = \"broken\"").expect("bad source");
    let invalid =
        measure_source_once(&graph, SourceId::new("shop").expect("source id")).expect("result");
    assert_eq!(invalid.status(), GraphSourceStatus::ConfigInvalid);
    let report = serde_json::to_value(crate::graph::GraphMeasureReport::from(&invalid))
        .expect("invalid report");
    assert!(report["source_config_error"].as_str().is_some());
    assert!(!source_directory.identity_file().exists());
}

#[test]
fn missing_source_secret_admits_one_source_scoped_integration_fault_event() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph =
        TrustedGraphRoot::initialize(GraphPaths::new(root), "2026-08-25T00:00:00Z".to_owned())
            .expect("graph");
    let source: SourceDocument = toml::from_str(&platform_processes(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"http\"\nsource_url = \"https://example.com/value\"\nuser_agent = \"ffhn-test\"\naccept = \"application/json\"\nmax_bytes = 1024\nfollow_redirects = true\nmax_redirects = 3\n[fetch.timeouts]\nconnect_ms = 1000\nread_idle_ms = 1000\ntotal_ms = 2000\n[fetch.header_secrets.authorization]\nenv = \"FFHN_TEST_MISSING_SECRET\"\nformat = \"Bearer {value}\"\nrevision = 1\n[conditional]\nenabled = true\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n[outbox]\nmax_pending = 3\nmax_attempts = 2\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n[[routes]]\nroute_id = \"source-alert\"\nroute_family = \"on_source\"\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = \"/usr/bin/true\"\ntimeout_ms = 1000\n",
    ))
    .expect("source configuration");
    let source_id = source.source_id().clone();
    let source = graph.create_source_document(&source).expect("source");
    let measurement: MeasurementDocument = toml::from_str(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"price\"\ndisplay_name = \"Price\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = \"/price\"\n",
    )
    .expect("measurement configuration");
    source
        .create_measurement_document(&measurement)
        .expect("measurement");
    let dry = measure_source_dry_run(&graph, source_id.clone()).expect("dry integration cycle");
    assert_eq!(dry.status(), GraphSourceStatus::IntegrationFault);
    assert!(dry.source_event_envelopes().is_empty());
    let first = measure_source_once(&graph, source_id.clone()).expect("first cycle");
    assert_eq!(first.status(), GraphSourceStatus::IntegrationFault);
    let report = serde_json::to_value(crate::graph::GraphMeasureReport::from(&first))
        .expect("integration report");
    assert_eq!(
        report["source_integration_fault_episode"]["code"],
        "secret_unavailable"
    );
    assert_eq!(
        first.source_event_envelopes()[0].event_kind(),
        crate::graph::EventKind::SourceIntegrationFault
    );
    let storage = source.open_storage().expect("storage");
    let records = storage
        .read_source_delivery_records()
        .expect("source records");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].envelope().event_kind(),
        crate::graph::EventKind::SourceIntegrationFault
    );
    measure_source_once(&graph, source_id).expect("second cycle");
    assert_eq!(
        storage
            .read_source_delivery_records()
            .expect("source records")
            .len(),
        1
    );
}

#[test]
fn source_acquisition_escalation_admits_one_source_scoped_event() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph =
        TrustedGraphRoot::initialize(GraphPaths::new(root), "2026-08-25T00:00:00Z".to_owned())
            .expect("graph");
    let source_file = temporary.path().join("missing.json");
    let source: SourceDocument = toml::from_str(&platform_processes(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 1\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n[outbox]\nmax_pending = 1\nmax_attempts = 2\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n[[routes]]\nroute_id = \"source-alert\"\nroute_family = \"on_source\"\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = \"/usr/bin/true\"\ntimeout_ms = 1000\n",
        source_file.to_string_lossy(),
    )))
    .expect("source configuration");
    let source_id = source.source_id().clone();
    let source = graph.create_source_document(&source).expect("source");
    let measurement: MeasurementDocument = toml::from_str(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"price\"\ndisplay_name = \"Price\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = \"/price\"\n",
    )
    .expect("measurement configuration");
    source
        .create_measurement_document(&measurement)
        .expect("measurement");
    let first = measure_source_once(&graph, source_id.clone()).expect("failure cycle");
    assert_eq!(first.status(), GraphSourceStatus::FetchFailed);
    let first_report = serde_json::to_value(crate::graph::GraphMeasureReport::from(&first))
        .expect("measure report");
    assert_eq!(first_report["source_failure"]["kind"], "file_not_found");
    assert_eq!(first_report["source_failure"]["reason_class"], "filesystem");
    assert_eq!(first_report["source_health"]["state"], "suspect");
    assert_eq!(
        first.source_event_envelopes()[0].event_kind(),
        crate::graph::EventKind::SourceEscalation
    );
    let records = source
        .open_storage()
        .expect("storage")
        .read_source_delivery_records()
        .expect("source records");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].envelope().event_kind(),
        crate::graph::EventKind::SourceEscalation
    );
    fs::write(&source_file, [0xff, 0xfe]).expect("invalid UTF-8 source");
    let second = measure_source_once(&graph, source_id).expect("second failure cycle");
    assert_eq!(second.source_outbox_overflow().len(), 1);
    let second_report = serde_json::to_value(crate::graph::GraphMeasureReport::from(&second))
        .expect("measure report");
    assert_eq!(second_report["source_failure"]["kind"], "invalid_utf8");
    assert_eq!(second_report["source_failure"]["reason_class"], "decode");
    assert_eq!(
        second_report["source_outbox_overflow"][0]["event_kind"],
        "source_escalation"
    );
    assert_eq!(
        source
            .open_storage()
            .expect("storage")
            .read_source_delivery_records()
            .expect("source records")
            .len(),
        1
    );
    assert_eq!(
        source
            .open_storage()
            .expect("storage")
            .read_source_state()
            .expect("source state")
            .expect("state")
            .outbox_overflow()
            .len(),
        1
    );
    fs::write(&source_file, r#"{"price":9}"#).expect("valid source");
    let recovered = measure_source_once(&graph, SourceId::new("shop").expect("source id"))
        .expect("recovered cycle");
    assert_eq!(recovered.status(), GraphSourceStatus::Document);
    assert_eq!(
        recovered.source_event_envelopes()[0].event_kind(),
        crate::graph::EventKind::SourceInitialized
    );
}

#[path = "tests/coverage.rs"]
mod coverage;

#[path = "tests/mutation_contracts.rs"]
mod mutation_contracts;

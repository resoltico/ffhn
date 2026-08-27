use super::*;

#[test]
fn projection_failure_staging_covers_integration_and_extraction_episode_lifecycles() {
    let temporary = tempdir().expect("temporary");
    let source_file = temporary.path().join("source.json");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
        source_file.to_string_lossy(),
    ))
    .expect("source");
    assert!(
        super::process::source_provenance(
            &source,
            &crate::graph::SourceDocumentBytes {
                body: "{}".to_owned(),
                effective_http_url: Some(url::Url::parse("https://example.test").expect("URL")),
                file_content_sha256: None,
                validators: None,
            },
        )
        .is_err()
    );
    let document: MeasurementDocument = toml::from_str(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"price\"\ndisplay_name = \"Price\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = \"/price\"\n",
    )
    .expect("measurement");
    let projection =
        crate::graph::PreparedMeasurementProjection::prepare(&document).expect("projection");
    let identity = crate::graph::SourceIdentity::fresh();
    let item = super::eligibility::Eligible {
        id: MeasurementId::new("price").expect("measurement"),
        document,
        projection,
        state: None,
        mvd: "a".repeat(64),
    };
    let mut integration_state = crate::graph::MeasurementState::fresh(
        identity.source_instance_id().clone(),
        crate::graph::MeasurementInstanceId::mint(),
    );
    let (status, evaluations, events) = super::process::stage_projection_failure(
        crate::graph::MeasurementProjectionFailure::Integration(
            crate::graph::GraphIntegrationFaultCode::HtmlcutInternalError,
        ),
        true,
        &crate::graph::GraphId::mint(),
        &source,
        &identity,
        &item.id,
        &item.document,
        &item.mvd,
        &mut integration_state,
        "2026-08-25T00:00:00Z",
    )
    .expect("integration staging");
    assert_eq!(status, GraphMeasurementStatus::IntegrationFault);
    assert!(evaluations.is_empty());
    assert_eq!(events.len(), 1);
    let (_, _, repeated) = super::process::stage_projection_failure(
        crate::graph::MeasurementProjectionFailure::Integration(
            crate::graph::GraphIntegrationFaultCode::HtmlcutInternalError,
        ),
        true,
        &crate::graph::GraphId::mint(),
        &source,
        &identity,
        &item.id,
        &item.document,
        &item.mvd,
        &mut integration_state,
        "2026-08-25T00:01:00Z",
    )
    .expect("repeated integration");
    assert!(repeated.is_empty());

    let mut extraction_state = crate::graph::MeasurementState::fresh(
        identity.source_instance_id().clone(),
        crate::graph::MeasurementInstanceId::mint(),
    );
    let (_, _, first) = super::process::stage_projection_failure(
        crate::graph::MeasurementProjectionFailure::Extraction(
            crate::graph::ExtractionFailureReason::JsonMalformed,
        ),
        true,
        &crate::graph::GraphId::mint(),
        &source,
        &identity,
        &item.id,
        &item.document,
        &item.mvd,
        &mut extraction_state,
        "2026-08-25T00:00:00Z",
    )
    .expect("first extraction");
    assert!(first.is_empty());
    let (status, _, escalated) = super::process::stage_projection_failure(
        crate::graph::MeasurementProjectionFailure::Extraction(
            crate::graph::ExtractionFailureReason::JsonMalformed,
        ),
        true,
        &crate::graph::GraphId::mint(),
        &source,
        &identity,
        &item.id,
        &item.document,
        &item.mvd,
        &mut extraction_state,
        "2026-08-25T00:01:00Z",
    )
    .expect("escalated extraction");
    assert_eq!(status, GraphMeasurementStatus::ExtractionFailed);
    assert_eq!(escalated.len(), 1);

    let mut policy_state = crate::graph::MeasurementState::fresh(
        identity.source_instance_id().clone(),
        crate::graph::MeasurementInstanceId::mint(),
    );
    let (status, _, events) = super::process::stage_policy_failure(
        &CoreError::policy_invariant("proof"),
        true,
        &crate::graph::GraphId::mint(),
        &source,
        &identity,
        &item.id,
        &item.document,
        &item.mvd,
        &mut policy_state,
        "2026-08-25T00:02:00Z",
    )
    .expect("policy failure");
    assert_eq!(status, GraphMeasurementStatus::IntegrationFault);
    assert_eq!(events.len(), 1);
    let with_state = super::not_modified_measurement(super::eligibility::Eligible {
        id: item.id.clone(),
        document: item.document.clone(),
        projection: item.projection.clone(),
        state: Some(policy_state),
        mvd: item.mvd.clone(),
    });
    assert!(with_state.extraction_health().is_some());
    let without_state = super::not_modified_measurement(item);
    assert!(without_state.extraction_health().is_none());
}

#[test]
fn direct_document_processing_converts_an_apply_contract_error_into_integration_fault_evidence() {
    let temporary = tempdir().expect("temporary");
    let root = temporary.path().join("graph");
    let graph =
        TrustedGraphRoot::initialize(GraphPaths::new(root), "2026-08-25T00:00:00Z".to_owned())
            .expect("graph");
    let source_path = temporary.path().join("source.json");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
        source_path.to_string_lossy(),
    ))
    .expect("source");
    let source_dir = graph.create_source_document(&source).expect("source");
    let measurement: MeasurementDocument = toml::from_str(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"price\"\ndisplay_name = \"Price\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = \"/price\"\n",
    )
    .expect("measurement");
    let identity = crate::graph::SourceIdentity::fresh();
    let instance = crate::graph::MeasurementInstanceId::mint();
    let mut measurement_state =
        crate::graph::MeasurementState::fresh(identity.source_instance_id().clone(), instance);
    measurement_state
        .apply_accepted_observation(
            &measurement,
            crate::model::parse_json_scalar_token_for_contract(
                crate::DeclaredType::Integer,
                &crate::TypeParams::default(),
                "1".to_owned(),
            )
            .expect("observation"),
            "b".repeat(64),
        )
        .expect("seed state");
    let eligible = super::eligibility::Eligible {
        id: MeasurementId::new("price").expect("measurement"),
        projection: crate::graph::PreparedMeasurementProjection::prepare(&measurement)
            .expect("projection"),
        document: measurement,
        state: Some(measurement_state),
        mvd: "a".repeat(64),
    };
    let result = super::process::document_commit(
        &source_dir,
        None,
        &source,
        identity.clone(),
        crate::graph::SourceState::fresh(identity.source_instance_id().clone()),
        "2026-08-25T00:00:00Z",
        Box::new(crate::graph::SourceDocumentBytes {
            body: "{\"price\":2}".to_owned(),
            effective_http_url: None,
            file_content_sha256: Some("c".repeat(64)),
            validators: None,
        }),
        vec![eligible],
        Vec::new(),
        crate::graph::GraphId::mint(),
        false,
    )
    .expect("document result");
    assert_eq!(
        result.measurements()[0].status(),
        GraphMeasurementStatus::IntegrationFault
    );
}

#[test]
fn result_vocabulary_selection_and_accessors_are_closed_and_complete() {
    for (status, spelling) in [
        (GraphSourceStatus::Disabled, "disabled"),
        (GraphSourceStatus::Locked, "skipped_locked"),
        (
            GraphSourceStatus::UnresolvableManifest,
            "unresolvable_manifest",
        ),
        (GraphSourceStatus::ConfigInvalid, "config_invalid"),
        (GraphSourceStatus::LineageRefused, "lineage_refused"),
        (GraphSourceStatus::AcquisitionHold, "acquisition_hold"),
        (GraphSourceStatus::Document, "document"),
        (GraphSourceStatus::NotModified, "not_modified"),
        (GraphSourceStatus::FetchFailed, "fetch_failed"),
        (GraphSourceStatus::IntegrationFault, "integration_fault"),
    ] {
        assert_eq!(status.as_str(), spelling);
    }
    for (status, spelling) in [
        (GraphMeasurementStatus::Initialized, "initialized"),
        (GraphMeasurementStatus::Changed, "changed"),
        (GraphMeasurementStatus::Unchanged, "unchanged"),
        (
            GraphMeasurementStatus::ExtractionFailed,
            "extraction_failed",
        ),
        (
            GraphMeasurementStatus::IntegrationFault,
            "integration_fault",
        ),
        (GraphMeasurementStatus::Quarantined, "quarantined"),
        (GraphMeasurementStatus::LineageHeld, "lineage_held"),
        (GraphMeasurementStatus::ConfigInvalid, "config_invalid"),
        (GraphMeasurementStatus::Disabled, "disabled"),
        (GraphMeasurementStatus::NotModified, "not_modified"),
    ] {
        assert_eq!(status.as_str(), spelling);
    }

    let source_id = SourceId::new("source").expect("source id");
    let measurement_id = MeasurementId::new("measurement").expect("measurement id");
    let measurement = super::results::measurement(
        measurement_id.clone(),
        GraphMeasurementStatus::Disabled,
        Vec::new(),
    );
    assert_eq!(measurement.measurement_id(), &measurement_id);
    assert_eq!(measurement.status(), GraphMeasurementStatus::Disabled);
    assert!(measurement.policy_evaluations().is_empty());
    assert!(measurement.event_envelopes().is_empty());
    assert!(measurement.outbox_overflow().is_empty());
    assert!(measurement.config_error().is_none());
    assert!(measurement.lineage_hold().is_none());
    assert!(measurement.stored_measurement_value_digest().is_none());
    assert!(measurement.current_measurement_value_digest().is_none());
    assert!(measurement.extraction_health().is_none());
    assert!(measurement.integration_fault_episode().is_none());
    assert!(measurement.observation().is_none());

    let result = super::results::result(
        source_id.clone(),
        GraphSourceStatus::Disabled,
        vec![measurement],
    );
    assert_eq!(result.source_id(), &source_id);
    assert_eq!(result.status(), GraphSourceStatus::Disabled);
    assert_eq!(result.measurements().len(), 1);
    assert!(result.source_event_envelopes().is_empty());
    assert!(result.source_outbox_overflow().is_empty());
    assert!(result.source_failure().is_none());
    assert!(!result.has_handled_failure());
    assert!(result.source_config_error().is_none());
    assert!(result.unresolvable_manifest().is_none());
    assert!(result.source_lineage_refusal().is_none());
    assert!(result.source_health().is_none());
    assert!(result.source_integration_fault_episode().is_none());

    for status in [
        GraphSourceStatus::UnresolvableManifest,
        GraphSourceStatus::ConfigInvalid,
        GraphSourceStatus::LineageRefused,
        GraphSourceStatus::AcquisitionHold,
        GraphSourceStatus::FetchFailed,
        GraphSourceStatus::IntegrationFault,
    ] {
        assert!(
            super::results::result(source_id.clone(), status, Vec::new()).has_handled_failure()
        );
    }
    for status in [
        GraphMeasurementStatus::ExtractionFailed,
        GraphMeasurementStatus::IntegrationFault,
        GraphMeasurementStatus::Quarantined,
        GraphMeasurementStatus::LineageHeld,
        GraphMeasurementStatus::ConfigInvalid,
    ] {
        let measurement = super::results::measurement(measurement_id.clone(), status, Vec::new());
        assert!(
            super::results::result(
                source_id.clone(),
                GraphSourceStatus::Document,
                vec![measurement]
            )
            .has_handled_failure()
        );
    }

    let configured = vec![
        MeasurementId::new("a").expect("a"),
        MeasurementId::new("b").expect("b"),
    ];
    assert_eq!(
        super::selection::selected_measurements(configured.clone(), None).expect("all"),
        configured
    );
    assert_eq!(
        super::selection::selected_measurements(
            configured.clone(),
            Some(vec![MeasurementId::new("b").expect("b")]),
        )
        .expect("selected")[0]
            .as_str(),
        "b"
    );
    assert!(
        super::selection::selected_measurements(
            configured.clone(),
            Some(vec![MeasurementId::new("missing").expect("missing")]),
        )
        .is_err()
    );
    assert!(
        super::selection::selected_measurements(
            configured,
            Some(vec![
                MeasurementId::new("a").expect("a"),
                MeasurementId::new("a").expect("a"),
            ]),
        )
        .is_err()
    );
}

#[test]
fn disabled_and_invalid_measurements_withhold_acquisition_as_structured_results() {
    let temporary = tempdir().expect("temporary graph");
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(temporary.path().join("graph")),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    let source_file = temporary.path().join("source.json");
    fs::write(&source_file, r#"{"value":1}"#).expect("source body");
    let source_body = |id: &str, enabled: bool| {
        format!(
            "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"{id}\"\ndisplay_name = \"Source\"\nenabled = {enabled}\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
            source_file.to_string_lossy(),
        )
    };
    let measurement_body = |id: &str, enabled: bool| {
        format!(
            "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"{id}\"\ndisplay_name = \"Measurement\"\nenabled = {enabled}\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n"
        )
    };

    let disabled_source: SourceDocument =
        toml::from_str(&source_body("disabled", false)).expect("disabled source");
    let disabled_dir = graph
        .create_source_document(&disabled_source)
        .expect("disabled source directory");
    let invalid_id = MeasurementId::new("invalid").expect("measurement id");
    let invalid_path = disabled_dir.paths().measurement_file(&invalid_id);
    fs::create_dir_all(invalid_path.parent().expect("measurement directory"))
        .expect("measurement directory");
    fs::write(&invalid_path, "schema_name = \"broken\"").expect("invalid measurement");
    let disabled = measure_source_once(&graph, SourceId::new("disabled").expect("source id"))
        .expect("disabled result");
    assert_eq!(disabled.status(), GraphSourceStatus::Disabled);
    assert_eq!(
        disabled.measurements()[0].status(),
        GraphMeasurementStatus::Disabled
    );
    assert!(disabled.measurements()[0].config_error().is_some());
    assert!(
        disabled_dir
            .read_identity()
            .expect("identity read")
            .is_none()
    );

    let enabled_source: SourceDocument =
        toml::from_str(&source_body("enabled", true)).expect("enabled source");
    let enabled_dir = graph
        .create_source_document(&enabled_source)
        .expect("enabled source directory");
    let disabled_measurement: MeasurementDocument =
        toml::from_str(&measurement_body("disabled", false)).expect("disabled measurement");
    enabled_dir
        .create_measurement_document(&disabled_measurement)
        .expect("disabled measurement");
    let hold = measure_source_once(&graph, SourceId::new("enabled").expect("source id"))
        .expect("hold result");
    assert_eq!(hold.status(), GraphSourceStatus::AcquisitionHold);
    assert_eq!(
        hold.measurements()[0].status(),
        GraphMeasurementStatus::Disabled
    );

    let invalid_id = MeasurementId::new("invalid").expect("measurement id");
    let invalid_path = enabled_dir.paths().measurement_file(&invalid_id);
    fs::create_dir_all(invalid_path.parent().expect("measurement directory"))
        .expect("measurement directory");
    fs::write(&invalid_path, "schema_name = \"broken\"").expect("invalid measurement");
    let hold = measure_source_once(&graph, SourceId::new("enabled").expect("source id"))
        .expect("invalid result");
    assert_eq!(hold.status(), GraphSourceStatus::AcquisitionHold);
    assert!(hold.measurements().iter().any(|item| {
        item.status() == GraphMeasurementStatus::ConfigInvalid && item.config_error().is_some()
    }));
}

#[test]
fn invalid_measurement_inventory_is_a_structured_source_config_failure() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(root.clone()),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
        temporary.path().join("source.json").to_string_lossy(),
    ))
    .expect("source");
    let source = graph.create_source_document(&source).expect("source");
    fs::write(source.paths().measurements_dir(), "not a directory").expect("bad inventory");
    let result = measure_source_once(&graph, SourceId::new("shop").expect("source"))
        .expect("structured result");
    assert_eq!(result.status(), GraphSourceStatus::ConfigInvalid);
    assert!(result.source_config_error().is_some());
}

#[test]
fn source_context_is_total_for_dry_uninitialized_persisted_ready_and_refused_lineage() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(root.clone()),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    fs::create_dir(root.join("sources/shop")).expect("source");
    let source = graph
        .open_source(SourceId::new("shop").expect("source"))
        .expect("source");
    let uninitialized = source.inspect_lineage([]).expect("inspection");
    assert!(matches!(
        super::source_context(&uninitialized, false).expect("dry context"),
        super::SourceContext::Ready {
            authoritative_lineage: false,
            ..
        }
    ));
    assert!(super::source_context(&uninitialized, true).is_err());
    fs::create_dir(source.paths().storage_dir()).expect("storage");
    let refused = source.inspect_lineage([]).expect("inspection");
    assert!(matches!(
        super::source_context(&refused, false).expect("refused context"),
        super::SourceContext::Refused(crate::graph::SourceLineageRefusal::StorageWithoutIdentity)
    ));
    fs::remove_dir(source.paths().storage_dir()).expect("remove storage");
    crate::graph::reset_source(&graph, SourceId::new("shop").expect("source")).expect("initialize");
    let ready = source.inspect_lineage([]).expect("inspection");
    assert!(matches!(
        super::source_context(&ready, true).expect("ready context"),
        super::SourceContext::Ready {
            authoritative_lineage: true,
            ..
        }
    ));
}

#[path = "coverage/operational.rs"]
mod operational;

use super::*;

#[test]
fn direct_conditional_304_commits_not_modified_source_and_measurement_evidence() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let worker = thread::spawn(move || {
        let mut requests = Vec::new();
        for response in [
            b"HTTP/1.1 200 OK\r\nETag: \"v1\"\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"price\":7}".as_slice(),
            b"HTTP/1.1 304 Not Modified\r\nETag: \"v1\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice(),
        ] {
            let (mut stream, _) = listener.accept().expect("request");
            let mut bytes = [0_u8; 2048];
            let count = stream.read(&mut bytes).expect("read request");
            requests.push(String::from_utf8_lossy(&bytes[..count]).into_owned());
            stream.write_all(response).expect("response");
        }
        requests
    });
    let temporary = tempdir().expect("temporary graph");
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(temporary.path().join("graph")),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"http\"\nsource_url = \"http://{address}/value\"\nuser_agent = \"ffhn-test\"\naccept = \"application/json\"\nmax_bytes = 1024\nfollow_redirects = true\nmax_redirects = 2\n[fetch.timeouts]\nconnect_ms = 1000\nread_idle_ms = 1000\ntotal_ms = 1000\n[conditional]\nenabled = true\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n"
    ))
    .expect("source");
    let source_dir = graph.create_source_document(&source).expect("source");
    let measurement: MeasurementDocument = toml::from_str(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"price\"\ndisplay_name = \"Price\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = \"/price\"\n",
    )
    .expect("measurement");
    source_dir
        .create_measurement_document(&measurement)
        .expect("measurement");
    let source_id = SourceId::new("shop").expect("source id");
    assert_eq!(
        measure_source_once(&graph, source_id.clone())
            .expect("first")
            .status(),
        GraphSourceStatus::Document
    );
    let second = measure_source_once(&graph, source_id).expect("second");
    assert_eq!(second.status(), GraphSourceStatus::NotModified);
    assert_eq!(
        second.measurements()[0].status(),
        GraphMeasurementStatus::NotModified
    );
    assert!(
        second.measurements()[0]
            .current_measurement_value_digest()
            .is_some()
    );
    assert!(second.source_health().is_some());
    let requests = worker.join().expect("worker");
    assert!(
        requests[1]
            .to_ascii_lowercase()
            .contains("if-none-match: \"v1\"")
    );
}

#[test]
fn repeated_extraction_failure_escalates_and_dry_lock_contention_is_structured() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let source_file = temporary.path().join("source.json");
    fs::write(&source_file, r#"{"other":7}"#).expect("source");
    let graph =
        TrustedGraphRoot::initialize(GraphPaths::new(root), "2026-08-25T00:00:00Z".to_owned())
            .expect("graph");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
        source_file.to_string_lossy(),
    ))
    .expect("source");
    let source_id = source.source_id().clone();
    let source_dir = graph.create_source_document(&source).expect("source");
    let measurement: MeasurementDocument = toml::from_str(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"price\"\ndisplay_name = \"Price\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = \"/price\"\n",
    )
    .expect("measurement");
    source_dir
        .create_measurement_document(&measurement)
        .expect("measurement");
    let first = measure_source_once(&graph, source_id.clone()).expect("first");
    assert_eq!(
        first.measurements()[0].status(),
        GraphMeasurementStatus::ExtractionFailed
    );
    let second = measure_source_once(&graph, source_id.clone()).expect("second");
    assert_eq!(
        second.measurements()[0].event_envelopes()[0].event_kind(),
        crate::graph::EventKind::ExtractionEscalation
    );
    assert!(second.measurements()[0].extraction_health().is_some());

    let lease = source_dir
        .try_acquire_write_lease()
        .expect("lock")
        .expect("lease");
    assert_eq!(
        measure_source_dry_run(&graph, source_id)
            .expect("dry lock")
            .status(),
        GraphSourceStatus::Locked
    );
    drop(lease);
}

#[test]
fn first_source_failure_persists_health_without_premature_escalation_event() {
    let temporary = tempdir().expect("temporary graph");
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(temporary.path().join("graph")),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
        temporary.path().join("missing.json").to_string_lossy(),
    ))
    .expect("source");
    let source_dir = graph.create_source_document(&source).expect("source");
    let measurement: MeasurementDocument = toml::from_str(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"price\"\ndisplay_name = \"Price\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = \"/price\"\n",
    )
    .expect("measurement");
    source_dir
        .create_measurement_document(&measurement)
        .expect("measurement");
    let dry = measure_source_dry_run(&graph, SourceId::new("shop").expect("source"))
        .expect("dry failure");
    assert_eq!(dry.status(), GraphSourceStatus::FetchFailed);
    assert!(dry.source_event_envelopes().is_empty());
    let result =
        measure_source_once(&graph, SourceId::new("shop").expect("source")).expect("failure");
    assert_eq!(result.status(), GraphSourceStatus::FetchFailed);
    assert!(result.source_event_envelopes().is_empty());
    assert!(result.source_outbox_overflow().is_empty());
    assert!(result.source_health().is_some());
}

#[test]
fn dry_cycles_report_lineage_and_commit_manifests_without_recovery() {
    for (manifest_path, expected) in [
        (
            ".ffhn-lineage.manifest",
            crate::graph::UnresolvableManifest::Lineage,
        ),
        (
            ".ffhn/commit.manifest",
            crate::graph::UnresolvableManifest::Commit,
        ),
    ] {
        let temporary = tempdir().expect("temporary graph");
        let root = temporary.path().join("graph");
        let graph = TrustedGraphRoot::initialize(
            GraphPaths::new(root.clone()),
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("graph");
        fs::create_dir_all(root.join("sources/shop/.ffhn")).expect("storage");
        fs::write(
            root.join("sources/shop/source.toml"),
            "schema_name = \"broken\"",
        )
        .expect("source");
        fs::write(root.join("sources/shop").join(manifest_path), "not-json").expect("manifest");
        let result = measure_source_dry_run(&graph, SourceId::new("shop").expect("source"))
            .expect("dry result");
        assert_eq!(result.status(), GraphSourceStatus::UnresolvableManifest);
        assert_eq!(result.unresolvable_manifest(), Some(expected));
        assert!(root.join("sources/shop").join(manifest_path).exists());
    }
}

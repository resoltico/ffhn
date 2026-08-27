use super::*;

#[test]
fn isolated_source_workers_defer_missing_source_and_unavailable_graph_reopen() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(root.clone()),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    let now = OffsetDateTime::parse("2026-08-25T00:00:00Z", &Rfc3339).expect("now");
    let mut deferrals = SourceDeferrals::default();
    let missing = AgentWorker::tick_source(
        &graph,
        SourceId::new("missing").expect("source"),
        now,
        "2026-08-25T00:00:00Z",
        &mut deferrals,
    );
    assert!(missing.acquisition_error().is_some());
    assert_eq!(missing.acquisition_deferred_reason(), Some("unreadable"));
    assert_eq!(missing.drain_deferred_reason(), Some("unreadable"));

    let source_file = temporary.path().join("source.json");
    fs::write(&source_file, "{}").expect("source");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
        source_file.to_string_lossy(),
    ))
    .expect("source config");
    graph.create_source_document(&source).expect("source");
    let graph_identity = graph
        .read_graph_identity()
        .expect("identity")
        .expect("identity");
    fs::remove_file(graph.paths().identity_file()).expect("remove identity");
    let mut failed_deferrals = SourceDeferrals::default();
    let failed = AgentWorker::tick_source(
        &graph,
        SourceId::new("shop").expect("source"),
        now,
        "2026-08-25T00:00:00Z",
        &mut failed_deferrals,
    );
    assert!(failed.acquisition_error().is_some());
    graph
        .write_graph_identity(&graph_identity)
        .expect("restore identity");
    let worker = AgentWorker::try_start(&graph)
        .expect("agent")
        .expect("lease");
    assert_eq!(
        super::super::wake::source_wake_candidates(
            &worker,
            &graph,
            SourceId::new("missing-again").expect("source"),
            now,
        ),
        [now + Duration::seconds(1)]
    );
    assert_eq!(
        super::super::wake::source_wake_candidates_with(
            &worker,
            &graph,
            SourceId::new("shop").expect("source"),
            now,
            |_| Err(CoreError::internal("inspection failed")),
            |_| Err(CoreError::internal("storage must not open")),
        ),
        [now + Duration::seconds(1)]
    );
    #[cfg(unix)]
    {
        drop(worker);
        let mut worker = AgentWorker::try_start(&graph)
            .expect("agent")
            .expect("lease");
        // Unix permits moving a tree while its advisory-lock inode remains open. Windows
        // correctly prevents that move, so its contention semantics are covered separately.
        let moved = temporary.path().join("moved-graph");
        fs::rename(&root, &moved).expect("move graph path");
        let result = worker
            .tick_with_jobs(&graph, "2026-08-25T00:00:00Z".to_owned(), 1)
            .expect("isolated tick");
        assert_eq!(
            result.sources()[0].acquisition_deferred_reason(),
            Some("unreadable")
        );
        assert!(result.sources()[0].acquisition_error().is_some());
        assert!(result.sources()[0].drain_error().is_some());
    }
}

#[test]
fn source_drain_unreachable_and_error_paths_set_independent_deferrals() {
    let temporary = tempfile::tempdir().expect("temporary graph");
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
    let source_id = source.source_id().clone();
    let source_dir = graph.create_source_document(&source).expect("source");
    fs::write(temporary.path().join("source.json"), "\"value\"").expect("source body");
    let measurement: MeasurementDocument = toml::from_str(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"value\"\ndisplay_name = \"Value\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"text\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = \"\"\n",
    )
    .expect("measurement");
    source_dir
        .create_measurement_document(&measurement)
        .expect("measurement");
    let now = OffsetDateTime::parse("2026-08-25T00:00:00Z", &Rfc3339).expect("now");
    let mut successful_deferrals = SourceDeferrals::default();
    let successful = AgentWorker::tick_source(
        &graph,
        source_id.clone(),
        now,
        "2026-08-25T00:00:00Z",
        &mut successful_deferrals,
    );
    assert_eq!(
        successful.measurement().expect("measurement").status(),
        GraphSourceStatus::Document
    );
    fs::write(source_dir.paths().lineage_manifest_file(), "not-json").expect("manifest");
    let mut deferrals = SourceDeferrals::default();
    let (drain, error, _) = AgentWorker::drain_source(
        &graph,
        &source_dir,
        &source_id,
        now,
        "2026-08-25T00:00:00Z",
        &mut deferrals,
    );
    assert_eq!(drain, Some(DrainResult::Unreachable));
    assert!(error.is_none());
    assert_eq!(
        deferrals.drain_reason,
        Some(DeferralReason::DeliveryUnreachable)
    );

    deferrals.drain_until = None;
    fs::remove_file(source_dir.paths().lineage_manifest_file()).expect("remove manifest");
    fs::remove_file(graph.paths().identity_file()).expect("remove graph identity");
    let (drain, error, _) = AgentWorker::drain_source(
        &graph,
        &source_dir,
        &source_id,
        now,
        "2026-08-25T00:00:00Z",
        &mut deferrals,
    );
    assert!(drain.is_none());
    assert!(error.is_some());
    assert_eq!(deferrals.drain_reason, Some(DeferralReason::Unreadable));
}

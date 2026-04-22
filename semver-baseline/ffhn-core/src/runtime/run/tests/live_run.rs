use super::support::*;

#[test]
fn run_once_covers_config_invalid_lock_state_and_disabled_paths() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let source_url = Url::parse("https://example.com").expect("url");

    write_target(
        &paths,
        &target_document(
            "other",
            true,
            source_url.clone(),
            "main",
            SelectionMatch::Single,
        ),
    );
    std::fs::write(paths.state_file(), [0xff]).expect("write unreadable state");
    let report = run_once(&paths).expect("config invalid run");
    assert_eq!(report.reason_code, ReasonCode::ConfigInvalid);
    assert_eq!(report.run_outcome, RunOutcome::FailedPermanent);
    assert!(!paths.run_lock_file().exists());

    write_target(
        &paths,
        &target_document(
            "demo",
            true,
            source_url.clone(),
            "main",
            SelectionMatch::Single,
        ),
    );
    let report = run_once(&paths).expect("state unreadable run");
    assert_eq!(report.reason_code, ReasonCode::StateInvalid);
    assert_eq!(report.run_outcome, RunOutcome::FailedPermanent);
    std::fs::remove_file(paths.state_file()).expect("remove unreadable state");

    let _lock = try_lock_exclusive(&paths).expect("lock");
    let report = run_once(&paths).expect("lock unavailable run");
    assert_eq!(report.reason_code, ReasonCode::LockUnavailable);

    drop(_lock);
    write_exact_text(paths.state_file(), "{not json").expect("write malformed state");
    let report = run_once(&paths).expect("malformed state run");
    assert_eq!(report.reason_code, ReasonCode::StateInvalid);

    write_json(
        paths.state_file(),
        &crate::StateDocument {
            schema_name: "wrong".to_owned(),
            schema_version: crate::STATE_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            state_phase: StatePhase::HasBaseline,
            last_run_at: None,
            last_run_outcome: None,
            last_reason_code: None,
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        },
    )
    .expect("write invalid state");
    let report = run_once(&paths).expect("state invalid run");
    assert_eq!(report.reason_code, ReasonCode::StateInvalid);

    write_snapshot_state(&paths, "hello", "<main>Hello</main>");
    write_exact_text(
        paths.target_dir().join("snapshots/current/outer.html"),
        "<main>Tampered</main>",
    )
    .expect("tamper state");
    let report = run_once(&paths).expect("integrity mismatch run");
    assert_eq!(report.reason_code, ReasonCode::IntegrityMismatch);

    std::fs::remove_file(paths.state_file()).expect("remove state");
    write_target(
        &paths,
        &target_document("demo", false, source_url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("disabled run");
    assert_eq!(report.run_outcome, RunOutcome::SkippedDisabled);
    assert_eq!(report.reason_code, ReasonCode::Disabled);
    assert!(report.persist.wrote_state);
    assert!(paths.last_run_file().is_file());
}

#[test]
fn live_run_holds_the_exclusive_lock_until_the_run_finishes() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let (source_url, accepted_rx, response_handle) = serve_once_with_accept_signal(
        TestResponse {
            status_line: "200 OK",
            content_type: "text/html",
            body: "<main>Hello</main>",
        },
        250,
    );
    write_target(
        &paths,
        &target_document("demo", true, source_url, "main", SelectionMatch::Single),
    );

    let first_paths = paths.clone();
    let first_run = thread::spawn(move || run_once(&first_paths).expect("first run"));
    accepted_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("first run reached the fetch server");

    let second_report = run_once(&paths).expect("second run");
    let first_report = first_run.join().expect("join first run");
    response_handle.join().expect("join response server");

    assert_eq!(second_report.reason_code, ReasonCode::LockUnavailable);
    assert_eq!(second_report.run_outcome, RunOutcome::FailedTransient);
    assert_eq!(first_report.run_outcome, RunOutcome::Initialized);
}

#[test]
fn run_once_surfaces_non_contention_lock_errors_as_fatal_core_errors() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    write_target(
        &paths,
        &target_document(
            "demo",
            true,
            Url::parse("https://example.com").expect("url"),
            "main",
            SelectionMatch::Single,
        ),
    );

    with_exclusive_lock_error_injected(std::io::ErrorKind::Other, || {
        let error = run_once(&paths).expect_err("lock io error");
        assert!(matches!(error, CoreError::Io { .. }));
    });
}

#[test]
fn run_once_reports_structured_persist_failures_for_live_disabled_fetch_extraction_and_success_paths()
 {
    let disabled_temp = tempdir().expect("disabled tempdir");
    let disabled_paths = TargetPaths::new(disabled_temp.path(), "demo");
    write_target(
        &disabled_paths,
        &target_document(
            "demo",
            false,
            Url::parse("https://example.com").expect("url"),
            "main",
            SelectionMatch::Single,
        ),
    );
    std::fs::create_dir_all(disabled_paths.state_file()).expect("disabled state dir conflict");
    let disabled_report = run_once(&disabled_paths).expect("disabled persist failure");
    assert_eq!(disabled_report.reason_code, ReasonCode::PersistError);
    assert_eq!(disabled_report.run_outcome, RunOutcome::FailedTransient);
    assert!(disabled_report.fetch.is_none());
    assert!(!disabled_report.persist.wrote_state);
    assert!(disabled_report.persist.wrote_last_run);
    assert!(disabled_paths.last_run_file().is_file());

    let fetch_temp = tempdir().expect("fetch tempdir");
    let fetch_paths = TargetPaths::new(fetch_temp.path(), "demo");
    write_snapshot_state(&fetch_paths, "before", "<main>Before</main>");
    let (url, fetch_handle) = serve_once_with_delay(
        TestResponse {
            status_line: "500 Internal Server Error",
            content_type: "text/html",
            body: "boom",
        },
        100,
    );
    write_target(
        &fetch_paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let fetch_state_path = fetch_paths.state_file();
    let fetch_state_conflict = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        std::fs::remove_file(&fetch_state_path).expect("remove fetch state file");
        std::fs::create_dir(&fetch_state_path).expect("fetch state dir conflict");
    });
    let fetch_report = run_once(&fetch_paths).expect("fetch persist failure");
    fetch_handle.join().expect("fetch join");
    fetch_state_conflict
        .join()
        .expect("fetch state conflict join");
    assert_eq!(fetch_report.reason_code, ReasonCode::PersistError);
    assert_eq!(fetch_report.run_outcome, RunOutcome::FailedTransient);
    assert!(fetch_report.fetch.is_some());
    assert!(fetch_report.extraction.is_none());
    assert!(!fetch_report.persist.wrote_state);

    let extraction_temp = tempdir().expect("extraction tempdir");
    let extraction_paths = TargetPaths::new(extraction_temp.path(), "demo");
    write_snapshot_state(&extraction_paths, "before", "<main>Before</main>");
    let (url, extraction_handle) = serve_once_with_delay(
        TestResponse {
            status_line: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: "<html><body><aside>No match</aside></body></html>",
        },
        100,
    );
    write_target(
        &extraction_paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let extraction_state_path = extraction_paths.state_file();
    let extraction_state_conflict = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        std::fs::remove_file(&extraction_state_path).expect("remove extraction state file");
        std::fs::create_dir(&extraction_state_path).expect("extraction state dir conflict");
    });
    let extraction_report = run_once(&extraction_paths).expect("extraction persist failure");
    extraction_handle.join().expect("extraction join");
    extraction_state_conflict
        .join()
        .expect("extraction state conflict join");
    assert_eq!(extraction_report.reason_code, ReasonCode::PersistError);
    assert_eq!(extraction_report.run_outcome, RunOutcome::FailedTransient);
    assert!(extraction_report.fetch.is_some());
    assert!(extraction_report.extraction.is_none());
    assert!(!extraction_report.persist.wrote_state);

    let success_temp = tempdir().expect("success tempdir");
    let success_paths = TargetPaths::new(success_temp.path(), "demo");
    let (url, success_handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main></body></html>",
    });
    write_target(
        &success_paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    std::fs::create_dir_all(success_paths.state_file()).expect("success state dir conflict");
    let success_report = run_once(&success_paths).expect("success persist failure");
    success_handle.join().expect("success join");
    assert_eq!(success_report.reason_code, ReasonCode::PersistError);
    assert_eq!(success_report.run_outcome, RunOutcome::FailedTransient);
    assert!(success_report.fetch.is_some());
    assert!(success_report.extraction.is_some());
    assert!(success_report.compare.is_some());
    assert!(success_report.change.is_some());
    assert_eq!(success_report.current_compare_digest_sha256, None);
    assert!(!success_report.persist.wrote_state);
    assert!(success_report.persist.wrote_last_run);
    assert!(success_paths.last_run_file().is_file());
}

#[test]
fn run_once_covers_fetch_and_extraction_failures() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");

    let (url, handle) = serve_once(TestResponse {
        status_line: "500 Internal Server Error",
        content_type: "text/html",
        body: "boom",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("fetch failure run");
    handle.join().expect("server join");
    assert_eq!(report.reason_code, ReasonCode::FetchHttpServerError);
    assert_eq!(report.run_outcome, RunOutcome::FailedTransient);

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main><main>Again</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("ambiguous extraction run");
    handle.join().expect("server join");
    assert_eq!(report.reason_code, ReasonCode::ExtractionAmbiguousMatch);

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><aside>No match</aside></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("no-match extraction run");
    handle.join().expect("server join");
    assert_eq!(report.reason_code, ReasonCode::ExtractionNoMatch);
    assert!(paths.last_run_file().is_file());
}

#[test]
fn run_once_initializes_then_detects_unchanged_and_changed_content() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("initialized run");
    handle.join().expect("server join");
    assert_eq!(report.run_outcome, RunOutcome::Initialized);
    assert_eq!(report.reason_code, ReasonCode::Ok);
    assert_eq!(report.target_status_after_run, TargetStatus::Ready);

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("unchanged run");
    handle.join().expect("server join");
    assert_eq!(report.run_outcome, RunOutcome::Unchanged);

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Changed</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("changed run");
    handle.join().expect("server join");
    assert_eq!(report.run_outcome, RunOutcome::Changed);
    assert_eq!(report.reason_code, ReasonCode::Ok);

    let state: crate::StateDocument = read_json(&paths.state_file()).expect("read state");
    assert_eq!(state.state_phase, StatePhase::HasBaseline);
    assert_eq!(state.snapshot_history.len(), 1);
    assert_eq!(state.snapshot_history[0].slot, SnapshotSlot::History);
    assert!(
        state.snapshot_history[0]
            .canonical_text_path
            .starts_with("snapshots/history/")
    );
    assert!(paths.last_run_file().is_file());
    let last_run: crate::RunReport = read_json(&paths.last_run_file()).expect("last run");
    assert!(report.persist.wrote_last_run);
    assert!(last_run.persist.wrote_last_run);
    assert_eq!(last_run, report);
}

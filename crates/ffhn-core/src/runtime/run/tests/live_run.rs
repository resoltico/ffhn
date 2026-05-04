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
    assert!(report.error_detail.is_some());
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
    assert!(report.error_detail.is_some());
    std::fs::remove_file(paths.state_file()).expect("remove unreadable state");

    // Windows allows within-process re-locking, so the lock-contention path cannot be exercised
    // inside a single test process on that platform.
    #[cfg(unix)]
    {
        let _lock = try_lock_exclusive(&paths).expect("lock");
        let report = run_once(&paths).expect("lock unavailable run");
        assert_eq!(report.reason_code, ReasonCode::LockUnavailable);
        assert!(report.error_detail.is_some());
    }
    write_exact_text(paths.state_file(), "{not json").expect("write malformed state");
    let report = run_once(&paths).expect("malformed state run");
    assert_eq!(report.reason_code, ReasonCode::StateInvalid);
    assert!(report.error_detail.is_some());

    write_json(
        paths.state_file(),
        &crate::StateDocument {
            schema_name: "wrong".to_owned(),
            schema_version: crate::STATE_SCHEMA_VERSION,
            target_id: target_id("demo"),
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
    assert!(report.error_detail.is_some());

    write_snapshot_state(&paths, "hello", "<main>Hello</main>");
    write_exact_text(
        paths.target_dir().join("snapshots/current/outer.html"),
        "<main>Tampered</main>",
    )
    .expect("tamper state");
    let report = run_once(&paths).expect("integrity mismatch run");
    assert_eq!(report.reason_code, ReasonCode::IntegrityMismatch);
    assert!(report.error_detail.is_some());

    std::fs::remove_file(paths.state_file()).expect("remove state");
    write_target(
        &paths,
        &target_document("demo", false, source_url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("disabled run");
    assert_eq!(report.run_outcome, RunOutcome::SkippedDisabled);
    assert_eq!(report.reason_code, ReasonCode::Disabled);
    assert!(report.error_detail.is_none());
    assert!(report.persist.wrote_state());
    assert!(paths.last_run_file().is_file());
}

#[test]
fn run_once_surfaces_target_load_io_failures_as_fatal_core_errors() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    std::fs::create_dir_all(paths.target_file()).expect("target file directory");

    let error = run_once(&paths).expect_err("target load io error");
    assert!(matches!(error, CoreError::Io { .. }));
}

#[test]
fn run_once_reports_watch_root_directory_failures_at_the_watch_root_path() {
    let temp = tempdir().expect("tempdir");
    let missing_paths = TargetPaths::new(temp.path().join("missing-watch-root"), "demo");
    let missing_error = run_once(&missing_paths).expect_err("missing watch root");
    match missing_error {
        CoreError::Io { path, source } => {
            assert_eq!(path, temp.path().join("missing-watch-root"));
            assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
            assert_eq!(source.to_string(), "watch root does not exist");
        }
        other => panic!("expected io error, got {other:?}"),
    }

    let watch_root_file = temp.path().join("watch-root.txt");
    write_text(watch_root_file.clone(), "not a directory").expect("write watch-root file");
    let file_paths = TargetPaths::new(&watch_root_file, "demo");
    let file_error = run_once(&file_paths).expect_err("file watch root");
    match file_error {
        CoreError::Io { path, source } => {
            assert_eq!(path, watch_root_file);
            assert_eq!(source.kind(), std::io::ErrorKind::Other);
            assert_eq!(source.to_string(), "watch root is not a directory");
        }
        other => panic!("expected io error, got {other:?}"),
    }
}

// Windows allows within-process re-locking, so two threads in the same test binary cannot
// observe the lock contention that this test exercises.
#[cfg(unix)]
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
    assert!(!disabled_report.persist.wrote_state());
    assert!(disabled_report.persist.wrote_last_run());
    assert!(disabled_report.persist.error().is_some());
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
    let fetch_report = with_write_error_injected("state.json", std::io::ErrorKind::Other, || {
        run_once(&fetch_paths).expect("fetch persist failure")
    });
    fetch_handle.join().expect("fetch join");
    assert_eq!(fetch_report.reason_code, ReasonCode::PersistError);
    assert_eq!(fetch_report.run_outcome, RunOutcome::FailedTransient);
    assert!(fetch_report.fetch.is_some());
    assert!(fetch_report.extraction.is_none());
    assert!(!fetch_report.persist.wrote_state());
    assert!(fetch_report.persist.wrote_last_run());
    assert!(fetch_report.persist.error().is_some());

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
    let extraction_report =
        with_write_error_injected("state.json", std::io::ErrorKind::Other, || {
            run_once(&extraction_paths).expect("extraction persist failure")
        });
    extraction_handle.join().expect("extraction join");
    assert_eq!(extraction_report.reason_code, ReasonCode::PersistError);
    assert_eq!(extraction_report.run_outcome, RunOutcome::FailedTransient);
    assert!(extraction_report.fetch.is_some());
    assert!(extraction_report.extraction.is_none());
    assert!(!extraction_report.persist.wrote_state());
    assert!(extraction_report.persist.wrote_last_run());
    assert!(extraction_report.persist.error().is_some());

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
    assert!(!success_report.persist.wrote_state());
    assert!(success_report.persist.wrote_last_run());
    assert!(success_report.persist.error().is_some());
    assert!(success_paths.last_run_file().is_file());
}

#[test]
fn run_once_reports_fetch_duration_after_the_body_read_finishes() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let (url, handle) = serve_once_with_delay(
        TestResponse {
            status_line: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: "<html><body><main>Hello</main></body></html>",
        },
        150,
    );
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );

    let report = run_once(&paths).expect("timed fetch run");
    handle.join().expect("server join");

    assert!(report.fetch.expect("fetch").duration_ms >= 150);
}

#[cfg(unix)]
#[test]
fn run_once_stamps_run_finished_at_after_notification_delivery() {
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main></body></html>",
    });
    let mut target = target_document("demo", true, url, "main", SelectionMatch::Single);
    target.notifications = vec![NotificationHook {
        name: "delay".to_owned(),
        on: vec![RunOutcome::Initialized],
        program: "/bin/sh".to_owned(),
        // Notification hooks receive a newline-delimited JSON payload on stdin. The hook must
        // drain that payload before sleeping or the writer can block indefinitely on Windows.
        args: vec!["-c".to_owned(), "cat >/dev/null; sleep 0.2".to_owned()],
        timeout_ms: 1_000,
    }];
    write_target(&paths, &target);

    let report = run_once(&paths).expect("run with delayed notification");
    handle.join().expect("server join");

    let started = OffsetDateTime::parse(&report.run_started_at, &Rfc3339).expect("started");
    let finished = OffsetDateTime::parse(&report.run_finished_at, &Rfc3339).expect("finished");
    assert!(finished > started);
    assert_eq!(report.notifications.len(), 1);
    assert!(report.notifications[0].duration_ms >= 150);
    assert!((finished - started).whole_milliseconds() >= 150);
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
            .as_str()
            .starts_with("snapshots/history/")
    );
    assert!(paths.last_run_file().is_file());
    let last_run: crate::RunReport = read_json(&paths.last_run_file()).expect("last run");
    assert!(report.persist.wrote_last_run());
    assert!(last_run.persist.wrote_last_run());
    assert_eq!(last_run, report);
}

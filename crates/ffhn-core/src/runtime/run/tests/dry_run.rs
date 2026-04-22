use super::support::*;

#[test]
fn run_once_dry_run_skips_live_only_state_failures_and_persistence() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let source_url = Url::parse("https://example.com").expect("url");

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
    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once_with_options(&paths, RunOptions::DRY_RUN).expect("dry run");
    handle.join().expect("dry-run join");
    assert_eq!(report.run_mode, RunMode::DryRun);
    assert_eq!(report.run_outcome, RunOutcome::Initialized);
    assert!(!report.persist.wrote_state);
    assert!(!paths.last_run_file().exists());

    write_target(
        &paths,
        &target_document("demo", true, source_url, "main", SelectionMatch::Single),
    );
    write_snapshot_state(&paths, "hello", "<main>Hello</main>");
    write_exact_text(
        paths.target_dir().join("snapshots/current/outer.html"),
        "<main>Tampered</main>",
    )
    .expect("tamper outer html");
    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let integrity_dry_run = run_once_with_options(&paths, RunOptions::DRY_RUN)
        .expect("dry run with integrity mismatch");
    handle.join().expect("integrity join");
    assert_eq!(integrity_dry_run.run_outcome, RunOutcome::Initialized);

    let (url, handle) = serve_once(TestResponse {
        status_line: "500 Internal Server Error",
        content_type: "text/html",
        body: "boom",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let fetch_failure =
        run_once_with_options(&paths, RunOptions::DRY_RUN).expect("dry-run fetch failure");
    handle.join().expect("fetch failure join");
    assert_eq!(fetch_failure.run_outcome, RunOutcome::FailedTransient);
    assert!(!fetch_failure.persist.wrote_state);

    #[cfg(unix)]
    {
        write_snapshot_state(&paths, "before", "<main>Before</main>");
        let metadata = std::fs::metadata(paths.state_file()).expect("state metadata");
        let original = metadata.permissions();
        let mut denied = original.clone();
        denied.set_mode(0o000);
        std::fs::set_permissions(paths.state_file(), denied).expect("deny state permissions");
        let (url, handle) = serve_once(TestResponse {
            status_line: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: "<html><body><main>Hello</main></body></html>",
        });
        write_target(
            &paths,
            &target_document("demo", true, url, "main", SelectionMatch::Single),
        );
        let unreadable_state =
            run_once_with_options(&paths, RunOptions::DRY_RUN).expect("dry-run unreadable");
        std::fs::set_permissions(paths.state_file(), original).expect("restore state permissions");
        handle.join().expect("unreadable state join");
        assert_eq!(unreadable_state.run_outcome, RunOutcome::Initialized);
        assert!(!unreadable_state.persist.wrote_state);
    }

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><aside>No match</aside></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let extraction_failure =
        run_once_with_options(&paths, RunOptions::DRY_RUN).expect("dry-run extraction failure");
    handle.join().expect("extraction join");
    assert_eq!(extraction_failure.run_outcome, RunOutcome::FailedPermanent);
    assert!(extraction_failure.fetch.is_some());
    assert!(!paths.last_run_file().exists());
}

#[test]
fn run_once_dry_run_waits_for_a_stable_locked_view() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let (url, response_handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );

    let exclusive_lock = try_lock_exclusive(&paths).expect("exclusive lock");
    let dry_paths = paths.clone();
    let (completion_tx, completion_rx) = mpsc::channel();
    let dry_run = thread::spawn(move || {
        let report = run_once_with_options(&dry_paths, RunOptions::DRY_RUN).expect("dry run");
        completion_tx.send(()).expect("completion signal");
        report
    });

    assert!(matches!(
        completion_rx.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));

    drop(exclusive_lock);
    let report = dry_run.join().expect("join dry run");
    response_handle.join().expect("join response server");

    assert_eq!(report.run_mode, RunMode::DryRun);
    assert_eq!(report.run_outcome, RunOutcome::Initialized);
    assert!(paths.lock_dir().is_dir());
    assert!(paths.run_lock_file().is_file());
    assert!(!paths.state_file().exists());
    assert!(!paths.last_run_file().exists());
}

#[test]
fn run_once_dry_run_surfaces_shared_lock_errors_as_fatal_core_errors() {
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
    std::fs::create_dir_all(paths.target_dir()).expect("create target dir");
    std::fs::write(paths.lock_dir(), "blocked").expect("block lock path");

    let error = run_once_with_options(&paths, RunOptions::DRY_RUN).expect_err("lock io error");
    assert!(matches!(error, CoreError::Io { .. }));
}

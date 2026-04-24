use super::support::*;

#[test]
fn run_batch_covers_all_outcome_buckets_and_fatal_errors() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path();

    let initialized_paths = TargetPaths::new(watch_root, "initialized");
    let (url, initialized_handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Init</main></body></html>",
    });
    write_target(
        &initialized_paths,
        &target_document("initialized", true, url, "main", SelectionMatch::Single),
    );

    let changed_paths = TargetPaths::new(watch_root, "changed");
    let (url, changed_handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>After</main></body></html>",
    });
    write_target(
        &changed_paths,
        &target_document("changed", true, url, "main", SelectionMatch::Single),
    );
    write_snapshot_state(&changed_paths, "before", "<main>Before</main>");

    let unchanged_paths = TargetPaths::new(watch_root, "unchanged");
    let (url, unchanged_handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Same</main></body></html>",
    });
    write_target(
        &unchanged_paths,
        &target_document("unchanged", true, url, "main", SelectionMatch::Single),
    );
    write_snapshot_state(&unchanged_paths, "Same", "<main>Same</main>");

    let transient_paths = TargetPaths::new(watch_root, "transient");
    let (url, transient_handle) = serve_once(TestResponse {
        status_line: "500 Internal Server Error",
        content_type: "text/html",
        body: "boom",
    });
    write_target(
        &transient_paths,
        &target_document("transient", true, url, "main", SelectionMatch::Single),
    );

    let permanent_paths = TargetPaths::new(watch_root, "permanent");
    write_target(
        &permanent_paths,
        &target_document(
            "other",
            true,
            Url::parse("https://example.com").expect("url"),
            "main",
            SelectionMatch::Single,
        ),
    );

    let skipped_paths = TargetPaths::new(watch_root, "skipped");
    write_target(
        &skipped_paths,
        &target_document(
            "skipped",
            false,
            Url::parse("https://example.com").expect("url"),
            "main",
            SelectionMatch::Single,
        ),
    );

    let fatal_paths = TargetPaths::new(watch_root, "fatal");
    write_target(
        &fatal_paths,
        &target_document(
            "fatal",
            true,
            Url::parse("https://example.com").expect("url"),
            "main",
            SelectionMatch::Single,
        ),
    );
    std::fs::write(fatal_paths.lock_dir(), "blocked").expect("block fatal lock path");

    let targets = target_ids(&[
        "initialized",
        "changed",
        "unchanged",
        "transient",
        "permanent",
        "skipped",
        "fatal",
    ]);
    let report = run_batch(watch_root, &targets, RunOptions::LIVE, 3).expect("batch report");

    initialized_handle.join().expect("initialized join");
    changed_handle.join().expect("changed join");
    unchanged_handle.join().expect("unchanged join");
    transient_handle.join().expect("transient join");

    assert_eq!(
        report.requested_targets,
        targets.iter().map(ToString::to_string).collect::<Vec<_>>()
    );
    assert_eq!(report.outcome_counts.initialized, 1);
    assert_eq!(report.outcome_counts.changed, 1);
    assert_eq!(report.outcome_counts.unchanged, 1);
    assert_eq!(report.outcome_counts.failed_transient, 1);
    assert_eq!(report.outcome_counts.failed_permanent, 1, "{report:?}");
    assert_eq!(report.outcome_counts.skipped_disabled, 1);
    assert_eq!(report.outcome_counts.persist_error, 0);
    assert_eq!(report.outcome_counts.fatal_error, 1);
    let fatal_entry = report
        .entries
        .iter()
        .find(|entry| entry.target_id == "fatal")
        .expect("fatal entry");
    assert!(fatal_entry.run_report.is_none());
    assert!(
        fatal_entry
            .fatal_error
            .as_ref()
            .is_some_and(|error| error.kind == crate::ProcessErrorKind::Io)
    );
}

#[test]
fn run_batch_rejects_zero_concurrency_and_duplicate_targets() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path();
    let paths = TargetPaths::new(watch_root, "demo");
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

    assert!(run_batch(watch_root, &target_ids(&["demo"]), RunOptions::LIVE, 0).is_err());
    assert!(
        run_batch(
            watch_root,
            &target_ids(&["demo", "demo"]),
            RunOptions::LIVE,
            1
        )
        .is_err()
    );
}

#[test]
fn run_batch_uses_bounded_worker_scheduling_instead_of_chunk_barriers() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().to_path_buf();

    let slow_one_paths = TargetPaths::new(&watch_root, "slow_one");
    let (slow_one_url, slow_one_rx, slow_one_handle) = serve_once_with_accept_signal(
        TestResponse {
            status_line: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: "<html><body><main>Slow One</main></body></html>",
        },
        400,
    );
    write_target(
        &slow_one_paths,
        &target_document(
            "slow_one",
            true,
            slow_one_url,
            "main",
            SelectionMatch::Single,
        ),
    );

    let fast_paths = TargetPaths::new(&watch_root, "fast");
    let (fast_url, fast_handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Fast</main></body></html>",
    });
    write_target(
        &fast_paths,
        &target_document("fast", true, fast_url, "main", SelectionMatch::Single),
    );

    let slow_two_paths = TargetPaths::new(&watch_root, "slow_two");
    let (slow_two_url, slow_two_rx, slow_two_handle) = serve_once_with_accept_signal(
        TestResponse {
            status_line: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: "<html><body><main>Slow Two</main></body></html>",
        },
        0,
    );
    write_target(
        &slow_two_paths,
        &target_document(
            "slow_two",
            true,
            slow_two_url,
            "main",
            SelectionMatch::Single,
        ),
    );

    let batch = thread::spawn({
        let watch_root = watch_root.clone();
        let targets = target_ids(&["slow_one", "fast", "slow_two"]);
        move || run_batch(&watch_root, &targets, RunOptions::LIVE, 2).expect("batch report")
    });

    slow_one_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("slow one started");
    slow_two_rx
        .recv_timeout(Duration::from_millis(250))
        .expect("slow two started before slow one finished");

    let report = batch.join().expect("batch join");
    slow_one_handle.join().expect("slow one join");
    fast_handle.join().expect("fast join");
    slow_two_handle.join().expect("slow two join");

    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| entry.target_id.as_str())
            .collect::<Vec<_>>(),
        ["slow_one", "fast", "slow_two"]
    );
}

#[test]
fn run_batch_counts_reports_with_reason_code_persist_error() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path();
    let paths = TargetPaths::new(watch_root, "persist_error");
    write_target(
        &paths,
        &target_document(
            "persist_error",
            false,
            Url::parse("https://example.com").expect("url"),
            "main",
            SelectionMatch::Single,
        ),
    );
    std::fs::create_dir_all(paths.state_file()).expect("state dir conflict");

    let report = run_batch(
        watch_root,
        &target_ids(&["persist_error"]),
        RunOptions::LIVE,
        1,
    )
    .expect("batch report");

    assert_eq!(report.outcome_counts.persist_error, 1);
    assert_eq!(report.outcome_counts.failed_transient, 1);
    let entry = report.entries.first().expect("entry");
    let run_report = entry.run_report.as_ref().expect("run report");
    assert_eq!(run_report.reason_code, ReasonCode::PersistError);
    assert!(run_report.persist.error.is_some());
}

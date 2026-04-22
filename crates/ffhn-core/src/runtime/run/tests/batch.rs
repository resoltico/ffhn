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

    let targets = vec![
        "initialized".to_owned(),
        "changed".to_owned(),
        "unchanged".to_owned(),
        "transient".to_owned(),
        "permanent".to_owned(),
        "skipped".to_owned(),
        "fatal".to_owned(),
    ];
    let report = run_batch(watch_root, &targets, RunOptions::LIVE, 3).expect("batch report");

    initialized_handle.join().expect("initialized join");
    changed_handle.join().expect("changed join");
    unchanged_handle.join().expect("unchanged join");
    transient_handle.join().expect("transient join");

    assert_eq!(report.requested_targets, targets);
    assert_eq!(report.outcome_counts.initialized, 1);
    assert_eq!(report.outcome_counts.changed, 1);
    assert_eq!(report.outcome_counts.unchanged, 1);
    assert_eq!(report.outcome_counts.failed_transient, 1);
    assert_eq!(report.outcome_counts.failed_permanent, 1, "{report:?}");
    assert_eq!(report.outcome_counts.skipped_disabled, 1);
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
            .as_deref()
            .is_some_and(|error| error.contains("filesystem error"))
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

    assert!(run_batch(watch_root, &["demo".to_owned()], RunOptions::LIVE, 0).is_err());
    assert!(
        run_batch(
            watch_root,
            &["demo".to_owned(), "demo".to_owned()],
            RunOptions::LIVE,
            1
        )
        .is_err()
    );
}

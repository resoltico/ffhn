use super::*;

#[test]
fn run_command_returns_zero_for_initialized_reports() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();

    let (url, handle) = serve_once(
        "200 OK",
        "text/html; charset=utf-8",
        "<html><main>Hello</main></html>",
    );
    write_target(&watch_root, &url, true);
    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string.clone(),
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    handle.join().expect("server join");
    assert_eq!(exit_code, 0);
    let report = parse_run_report(&stdout);
    assert_eq!(report.run_outcome(), ffhn_core::RunOutcome::Initialized);
    assert_eq!(report.failure_cause(), None);
    assert!(stderr.is_empty());
}

#[test]
fn run_command_returns_zero_for_disabled_reports() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();

    write_target(&watch_root, "https://example.com", false);
    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    let report = parse_run_report(&stdout);
    assert_eq!(report.run_outcome(), ffhn_core::RunOutcome::SkippedDisabled);
    assert_eq!(report.failure_cause(), None);
    assert!(stderr.is_empty());
}

#[test]
fn run_command_returns_failed_exit_for_structured_run_failures() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();

    let (url, handle) = serve_once("500 Internal Server Error", "text/html", "boom");
    write_target(&watch_root, &url, true);
    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string.clone(),
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    handle.join().expect("server join");
    assert_eq!(exit_code, EXIT_CODE_RUN_FAILED);
    let report = parse_run_report(&stdout);
    assert_eq!(report.run_outcome(), ffhn_core::RunOutcome::FailedTransient);
    assert_eq!(
        report.failure_cause(),
        Some(ffhn_core::RunFailureCause::FetchHttpServerError)
    );
    assert!(stderr.is_empty());
}

#[test]
fn run_command_supports_dry_run_and_batch_rendering() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();

    let file_target_dir = watch_root.join("demo_file");
    fs::create_dir_all(&file_target_dir).expect("file target dir");
    let source_path = temp.path().join("source.html");
    fs::write(&source_path, "<html><body><main>Hello</main></body></html>").expect("source");
    fs::write(
        file_target_dir.join("target.toml"),
        format!(
            r#"
schema_name = "ffhn.target"
schema_version = 3
target_id = "demo_file"
display_name = "Demo File"
enabled = true

[target]
kind = "file"
file_path = {source_path:?}

[fetch]
engine = "file"
max_bytes = 2000000

[selection]
kind = "css_selector"
selector = "main"
match = "single"
output = "outer_html"
whitespace = "normalize"
rewrite_urls = false

[compare]
basis = "canonical_text_sha256"
canonicalization = []
"#
        ),
    )
    .expect("file target");

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string.clone(),
        "--target".to_owned(),
        "demo_file".to_owned(),
        "--dry-run".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    let report = parse_run_report(&stdout);
    assert_eq!(report.run_mode(), ffhn_core::RunMode::DryRun);
    assert_eq!(report.run_outcome(), ffhn_core::RunOutcome::Initialized);
    assert!(stderr.is_empty());

    write_target(&watch_root, "https://example.com", false);
    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string.clone(),
        "--target".to_owned(),
        "demo".to_owned(),
        "--dry-run".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    let report = parse_run_report(&stdout);
    assert_eq!(report.run_mode(), ffhn_core::RunMode::DryRun);
    assert_eq!(report.run_outcome(), ffhn_core::RunOutcome::SkippedDisabled);
    assert!(matches!(report.body(), ffhn_core::RunBodyView::None));
    assert!(report.persist().state_commit().is_not_attempted());
    assert!(report.persist().last_run_write().is_not_attempted());
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--all".to_owned(),
        "--jobs".to_owned(),
        "2".to_owned(),
        "--dry-run".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    let report = parse_batch_run_report(&stdout);
    assert_eq!(report.run_mode(), ffhn_core::RunMode::DryRun);
    assert_eq!(report.max_concurrency(), 2);
    assert_eq!(report.requested_targets(), ["demo", "demo_file"]);
    assert_eq!(report.entries().len(), 2);
    assert_eq!(report.outcome_counts().skipped_disabled(), 1);
    assert!(stderr.is_empty());
}

#[test]
fn run_and_status_commands_support_summary_and_pretty_json_output_formats() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    let source_path = temp.path().join("source.html");
    fs::write(&source_path, "<html><body><main>Hello</main></body></html>").expect("source");
    write_named_file_target(&watch_root, "demo_file", "demo_file", &source_path, true);

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string.clone(),
        "--target".to_owned(),
        "demo_file".to_owned(),
        "--format".to_owned(),
        "summary".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    assert!(stdout.contains("Run report"));
    assert!(stdout.contains("Mode: live"));
    assert!(stdout.contains("Outcome: initialized"));
    assert!(stdout.contains("Baseline phase: never_succeeded -> has_baseline"));
    assert!(stdout.contains("Fetch: engine=file"));
    assert!(stdout.contains("Extraction: kind=css_selector, match=single, output=outer_html"));
    assert!(stdout.contains("Compare: basis=canonical_text_sha256"));
    assert!(stdout.contains("Change: kind=initialized"));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "status".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--target".to_owned(),
        "demo_file".to_owned(),
        "--format".to_owned(),
        "json-pretty".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    assert!(stdout.starts_with("{\n  \"schema_name\": \"ffhn.status_report\""));
    let report = parse_status_report(&stdout);
    assert_eq!(report.display_name(), Some("demo_file"));
    assert!(stderr.is_empty());
}

#[test]
fn run_command_batch_covers_live_failure_render_and_validation_fatal_paths() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    let source_path = temp.path().join("source.html");
    fs::write(&source_path, "<html><body><main>Hello</main></body></html>").expect("source");

    write_named_file_target(&watch_root, "demo_file", "demo_file", &source_path, true);
    write_named_file_target(&watch_root, "demo_invalid", "other", &source_path, true);

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string.clone(),
        "--target".to_owned(),
        "demo_file".to_owned(),
        "--target".to_owned(),
        "demo_invalid".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_RUN_FAILED);
    let report = parse_batch_run_report(&stdout);
    assert_eq!(report.run_mode(), ffhn_core::RunMode::Live);
    assert_eq!(report.outcome_counts().failed_permanent(), 1);
    assert!(stderr.is_empty());

    let (url, handle) = serve_once("500 Internal Server Error", "text/html", "boom");
    write_named_http_target(&watch_root, "demo_transient", "demo_transient", &url, true);
    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string.clone(),
        "--target".to_owned(),
        "demo_file".to_owned(),
        "--target".to_owned(),
        "demo_transient".to_owned(),
    ]);
    handle.join().expect("transient server join");
    assert_eq!(exit_code, EXIT_CODE_RUN_FAILED);
    let report = parse_batch_run_report(&stdout);
    assert_eq!(report.outcome_counts().failed_transient(), 1);
    assert!(stderr.is_empty());

    write_named_file_target(&watch_root, "demo_fatal", "demo_fatal", &source_path, true);
    fs::write(watch_root.join("demo_fatal").join("lock"), "blocked").expect("fatal lock path");
    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string.clone(),
        "--target".to_owned(),
        "demo_file".to_owned(),
        "--target".to_owned(),
        "demo_fatal".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_RUN_FAILED);
    let report = parse_batch_run_report(&stdout);
    assert_eq!(report.outcome_counts().fatal_error(), 1);
    let fatal_entry = report
        .entries()
        .iter()
        .find(|entry| entry.target_id() == "demo_fatal")
        .expect("fatal entry");
    assert_eq!(
        fatal_entry.fatal_error().expect("fatal error").kind(),
        ffhn_core::ProcessErrorKind::Io
    );
    assert!(stderr.is_empty());

    let mut broken_stdout = BrokenWriter;
    let mut stderr = Vec::new();
    let exit_code = run(
        vec![
            "ffhn".to_owned(),
            "run".to_owned(),
            "--watch-root".to_owned(),
            temp.path().join("missing").to_string_lossy().into_owned(),
            "--all".to_owned(),
        ],
        &mut broken_stdout,
        &mut stderr,
    );
    assert_eq!(exit_code, EXIT_CODE_FATAL);
    let stderr = String::from_utf8(stderr).expect("stderr utf8");
    assert!(stderr.contains("error: filesystem error"));
    assert!(stderr.contains("watch root does not exist"));

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--target".to_owned(),
        "demo_file".to_owned(),
        "--target".to_owned(),
        "Demo".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains("target_id"));
}

#[test]
fn run_command_reports_persist_failures_as_structured_run_failures() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();

    let (url, handle) = serve_once(
        "200 OK",
        "text/html; charset=utf-8",
        "<html><main>Hello</main></html>",
    );
    write_target(&watch_root, &url, true);
    let state_path = watch_root.join("demo").join("state.json");
    fs::create_dir_all(&state_path).expect("create state dir conflict");
    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    handle.join().expect("server join");
    assert_eq!(exit_code, EXIT_CODE_RUN_FAILED);
    let report = parse_run_report(&stdout);
    assert_eq!(report.run_outcome(), ffhn_core::RunOutcome::FailedTransient);
    assert_eq!(
        report.failure_cause(),
        Some(ffhn_core::RunFailureCause::PersistError)
    );
    assert!(stderr.is_empty());
}

#[test]
fn run_command_returns_failed_exit_when_final_last_run_write_fails() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();

    let (url, handle) = serve_once(
        "200 OK",
        "text/html; charset=utf-8",
        "<html><main>Hello</main></html>",
    );
    write_target(&watch_root, &url, true);
    fs::create_dir_all(watch_root.join("demo").join("last_run.json"))
        .expect("block last_run write");

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    handle.join().expect("server join");

    assert_eq!(exit_code, EXIT_CODE_RUN_FAILED);
    let report = parse_run_report(&stdout);
    assert_eq!(report.run_outcome(), ffhn_core::RunOutcome::Initialized);
    assert_eq!(report.failure_cause(), None);
    assert!(report.persist().last_run_write().is_failed());
    assert_eq!(
        report
            .persist()
            .last_run_write()
            .error()
            .expect("last_run write error")
            .kind(),
        ffhn_core::ProcessErrorKind::Io
    );
    assert!(stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn run_command_returns_failed_exit_when_notification_delivery_fails() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    let source_path = temp.path().join("source.html");
    fs::write(&source_path, "<html><body><main>Hello</main></body></html>").expect("source");
    write_named_file_target(&watch_root, "demo", "demo", &source_path, true);
    fs::write(
        watch_root.join("demo").join("target.toml"),
        format!(
            "{}\n[[notification_endpoints]]\nname = \"broken\"\nkind = \"process_stdin\"\nprogram = \"/bin/sh\"\nargs = [\"-c\", \"echo hook-broke >&2; exit 7\"]\ntimeout_ms = 1000\n\n[[notification_routes]]\nname = \"broken\"\non = [\"initialized\"]\nendpoint = \"broken\"\n",
            fs::read_to_string(watch_root.join("demo").join("target.toml")).expect("read target")
        ),
    )
    .expect("write target with notification");

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--target".to_owned(),
        "demo".to_owned(),
    ]);

    assert_eq!(exit_code, EXIT_CODE_RUN_FAILED);
    let report = parse_run_report(&stdout);
    assert_eq!(report.run_outcome(), ffhn_core::RunOutcome::Initialized);
    let deliveries = report.notifications().collect::<Vec<_>>();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(
        deliveries[0].status(),
        ffhn_core::NotificationDeliveryStatus::Failed
    );
    assert!(
        deliveries[0]
            .error()
            .expect("notification error")
            .contains("hook-broke")
    );
    assert!(stderr.is_empty());
}

#[test]
fn batch_run_command_counts_last_run_write_failures_in_persist_failure_bucket() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();

    let (ok_url, ok_handle) = serve_once(
        "200 OK",
        "text/html; charset=utf-8",
        "<html><main>Ok</main></html>",
    );
    write_named_http_target(&watch_root, "ok", "ok", &ok_url, true);

    let (blocked_url, blocked_handle) = serve_once(
        "200 OK",
        "text/html; charset=utf-8",
        "<html><main>Blocked</main></html>",
    );
    write_named_http_target(&watch_root, "blocked", "blocked", &blocked_url, true);
    fs::create_dir_all(watch_root.join("blocked").join("last_run.json"))
        .expect("block batch last_run write");

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--target".to_owned(),
        "ok".to_owned(),
        "--target".to_owned(),
        "blocked".to_owned(),
    ]);
    ok_handle.join().expect("ok join");
    blocked_handle.join().expect("blocked join");

    assert_eq!(exit_code, EXIT_CODE_RUN_FAILED);
    let report = parse_batch_run_report(&stdout);
    assert_eq!(report.outcome_counts().initialized(), 2);
    assert_eq!(report.outcome_counts().persist_failure(), 1);
    let blocked_entry = report
        .entries()
        .iter()
        .find(|entry| entry.target_id() == "blocked")
        .expect("blocked entry");
    let blocked_report = blocked_entry.run_report().expect("blocked run report");
    assert_eq!(
        blocked_report.run_outcome(),
        ffhn_core::RunOutcome::Initialized
    );
    assert_eq!(blocked_report.failure_cause(), None);
    assert_eq!(
        blocked_report
            .persist()
            .last_run_write()
            .error()
            .expect("last_run write error")
            .kind(),
        ffhn_core::ProcessErrorKind::Io
    );
    assert!(stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn batch_run_command_returns_failed_exit_when_only_notification_delivery_fails() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    let source_path = temp.path().join("source.html");
    fs::write(&source_path, "<html><body><main>Hello</main></body></html>").expect("source");

    write_named_file_target(&watch_root, "ok", "ok", &source_path, true);
    write_named_file_target(&watch_root, "broken", "broken", &source_path, true);
    fs::write(
        watch_root.join("broken").join("target.toml"),
        format!(
            "{}\n[[notification_endpoints]]\nname = \"broken\"\nkind = \"process_stdin\"\nprogram = \"/bin/sh\"\nargs = [\"-c\", \"echo hook-broke >&2; exit 7\"]\ntimeout_ms = 1000\n\n[[notification_routes]]\nname = \"broken\"\non = [\"initialized\"]\nendpoint = \"broken\"\n",
            fs::read_to_string(watch_root.join("broken").join("target.toml")).expect("read target")
        ),
    )
    .expect("write broken target");

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--target".to_owned(),
        "ok".to_owned(),
        "--target".to_owned(),
        "broken".to_owned(),
    ]);

    assert_eq!(exit_code, EXIT_CODE_RUN_FAILED);
    let report = parse_batch_run_report(&stdout);
    assert_eq!(report.outcome_counts().initialized(), 2);
    assert_eq!(report.outcome_counts().notification_failure(), 1);
    let broken_entry = report
        .entries()
        .iter()
        .find(|entry| entry.target_id() == "broken")
        .expect("broken entry");
    let broken_report = broken_entry.run_report().expect("broken run report");
    let deliveries = broken_report.notifications().collect::<Vec<_>>();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(
        deliveries[0].status(),
        ffhn_core::NotificationDeliveryStatus::Failed
    );
    assert!(
        deliveries[0]
            .error()
            .expect("notification error")
            .contains("hook-broke")
    );
    assert!(stderr.is_empty());
}

#[test]
fn run_command_reports_unreadable_state_as_a_structured_failure() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    write_target(&watch_root, "https://example.com", true);
    fs::write(watch_root.join("demo").join("state.json"), [0xff]).expect("broken state");

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--target".to_owned(),
        "demo".to_owned(),
    ]);

    assert_eq!(exit_code, EXIT_CODE_RUN_FAILED);
    let report = parse_run_report(&stdout);
    assert_eq!(report.run_outcome(), ffhn_core::RunOutcome::FailedPermanent);
    assert_eq!(
        report.failure_cause(),
        Some(ffhn_core::RunFailureCause::StateInvalid)
    );
    assert!(stderr.is_empty());
}

#[test]
fn run_and_status_return_fatal_when_lock_path_is_not_a_directory() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    write_target(&watch_root, "https://example.com", true);
    fs::write(watch_root.join("demo").join("lock"), "blocked").expect("block lock directory");

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string.clone(),
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_FATAL);
    assert!(stdout.is_empty());
    assert!(stderr.contains("filesystem error"));

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "status".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_FATAL);
    assert!(stdout.is_empty());
    assert!(stderr.contains("filesystem error"));
}

#[test]
fn run_and_status_report_unavailable_when_target_toml_is_not_a_file() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    fs::create_dir_all(watch_root.join("demo").join("target.toml"))
        .expect("create target file directory");

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string.clone(),
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_RUN_FAILED);
    let report = parse_run_report(&stdout);
    assert_eq!(report.run_outcome(), ffhn_core::RunOutcome::FailedPermanent);
    assert_eq!(
        report.failure_cause(),
        Some(ffhn_core::RunFailureCause::TargetUnavailable)
    );
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "status".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    let report = parse_status_report(&stdout);
    assert!(matches!(
        report.status(),
        ffhn_core::StatusSummary::UnavailableTarget { .. }
    ));
    assert!(stderr.is_empty());
}

#[test]
fn status_and_writer_failures_cover_cli_fatal_paths() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    write_target(&watch_root, "https://example.com", true);

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "status".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string.clone(),
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    let report = parse_status_report(&stdout);
    assert_eq!(report.schema_name(), STATUS_REPORT_SCHEMA_NAME);
    assert!(report.status().is_pending());
    assert!(stderr.is_empty());

    fs::write(watch_root.join("demo").join("state.json"), "{not json").expect("broken state");
    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "status".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string.clone(),
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    let report = parse_status_report(&stdout);
    assert_eq!(report.schema_name(), STATUS_REPORT_SCHEMA_NAME);
    assert!(report.status().is_invalid());
    assert!(matches!(
        report.status(),
        ffhn_core::StatusSummary::InvalidState { .. }
    ));
    assert!(stderr.is_empty());

    fs::write(watch_root.join("demo").join("state.json"), [0xff]).expect("unreadable state");
    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "status".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string.clone(),
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    let report = parse_status_report(&stdout);
    assert!(report.status().is_invalid());
    assert!(matches!(
        report.status(),
        ffhn_core::StatusSummary::InvalidState { .. }
    ));
    assert!(stderr.is_empty());

    fs::remove_file(watch_root.join("demo").join("state.json")).expect("remove broken state");
    let mut broken_stdout = BrokenWriter;
    let mut stderr = Vec::new();
    let exit_code = run(
        vec![
            "ffhn".to_owned(),
            "status".to_owned(),
            "--watch-root".to_owned(),
            watch_root_string.clone(),
            "--target".to_owned(),
            "demo".to_owned(),
        ],
        &mut broken_stdout,
        &mut stderr,
    );
    assert_eq!(exit_code, EXIT_CODE_FATAL);
    assert!(String::from_utf8(stderr).expect("stderr utf8").contains(
        &document_write_error(STATUS_REPORT_SCHEMA_NAME).expect("status report write error")
    ));

    let (url, handle) = serve_once(
        "200 OK",
        "text/html; charset=utf-8",
        "<html><main>Hello</main></html>",
    );
    write_target(&watch_root, &url, true);
    let mut broken_stdout = BrokenWriter;
    let mut stderr = Vec::new();
    let exit_code = run(
        vec![
            "ffhn".to_owned(),
            "run".to_owned(),
            "--watch-root".to_owned(),
            watch_root_string,
            "--target".to_owned(),
            "demo".to_owned(),
        ],
        &mut broken_stdout,
        &mut stderr,
    );
    handle.join().expect("server join");
    assert_eq!(exit_code, EXIT_CODE_FATAL);
    assert!(
        String::from_utf8(stderr).expect("stderr utf8").contains(
            &document_write_error(RUN_REPORT_SCHEMA_NAME).expect("run report write error")
        )
    );

    let file_target_dir = watch_root.join("demo_file");
    fs::create_dir_all(&file_target_dir).expect("batch file target dir");
    let source_path = temp.path().join("batch-source.html");
    fs::write(&source_path, "<html><body><main>Hello</main></body></html>").expect("source");
    fs::write(
        file_target_dir.join("target.toml"),
        format!(
            r#"
schema_name = "ffhn.target"
schema_version = 3
target_id = "demo_file"
display_name = "Demo File"
enabled = true

[target]
kind = "file"
file_path = {source_path:?}

[fetch]
engine = "file"
max_bytes = 2000000

[selection]
kind = "css_selector"
selector = "main"
match = "single"
output = "outer_html"
whitespace = "normalize"
rewrite_urls = false

[compare]
basis = "canonical_text_sha256"
canonicalization = []
"#
        ),
    )
    .expect("batch file target");

    let mut broken_stdout = BrokenWriter;
    let mut stderr = Vec::new();
    let exit_code = run(
        vec![
            "ffhn".to_owned(),
            "run".to_owned(),
            "--watch-root".to_owned(),
            watch_root.to_string_lossy().into_owned(),
            "--all".to_owned(),
            "--dry-run".to_owned(),
        ],
        &mut broken_stdout,
        &mut stderr,
    );
    assert_eq!(exit_code, EXIT_CODE_FATAL);
    assert!(String::from_utf8(stderr).expect("stderr utf8").contains(
        &document_write_error(BATCH_RUN_REPORT_SCHEMA_NAME).expect("batch run report write error")
    ));
}

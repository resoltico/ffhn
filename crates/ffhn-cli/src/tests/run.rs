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
    assert!(stdout.contains("\"run_outcome\":\"initialized\""));
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
    assert!(stdout.contains("\"run_outcome\":\"skipped_disabled\""));
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
    assert!(stdout.contains("\"reason_code\":\"fetch_http_server_error\""));
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
schema_version = 1
target_id = "demo_file"
display_name = "Demo File"
enabled = true

[target]
kind = "file"
file_path = {source_path:?}

[fetch]
engine = "file"
follow_redirects = false
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
    assert!(stdout.contains("\"run_mode\":\"dry_run\""));
    assert!(stdout.contains("\"run_outcome\":\"initialized\""));
    assert!(stderr.is_empty());

    write_target(&watch_root, "https://example.com", false);
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
    assert!(stdout.contains("\"schema_name\":\"ffhn.batch_run_report\""));
    assert!(stdout.contains("\"max_concurrency\":2"));
    assert!(stdout.contains("\"requested_targets\":[\"demo_file\"]"));
    assert!(stdout.contains("\"target_id\":\"demo_file\""));
    assert!(!stdout.contains("\"target_id\":\"demo\""));
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
    assert!(stdout.contains("\"schema_name\":\"ffhn.batch_run_report\""));
    assert!(stdout.contains("\"run_mode\":\"live\""));
    assert!(stdout.contains("\"failed_permanent\":1"));
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
    assert!(stdout.contains("\"failed_transient\":1"));
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
    assert!(stdout.contains("\"fatal_error\":1"));
    assert!(stdout.contains("\"fatal_error\":{\"kind\":\"io\""));
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
    assert!(
        String::from_utf8(stderr)
            .expect("stderr utf8")
            .contains("watch root does not exist")
    );

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
    assert!(stdout.contains("\"run_outcome\":\"failed_transient\""));
    assert!(stdout.contains("\"reason_code\":\"persist_error\""));
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
    assert!(stdout.contains("\"run_outcome\":\"initialized\""));
    assert!(stdout.contains("\"wrote_last_run\":false"));
    assert!(stdout.contains("\"error\":{\"kind\":\"io\""));
    assert!(stderr.is_empty());
}

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
            "{}\n[[notifications]]\nname = \"broken\"\non = [\"initialized\"]\nshell = \"/bin/sh\"\ncommand = \"echo hook-broke >&2; exit 7\"\ntimeout_ms = 1000\n",
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
    assert!(stdout.contains("\"run_outcome\":\"initialized\""));
    assert!(stdout.contains("\"delivered\":false"));
    assert!(stdout.contains("hook-broke"));
    assert!(stderr.is_empty());
}

#[test]
fn batch_run_command_counts_last_run_write_failures_in_persist_error_bucket() {
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
    assert!(stdout.contains("\"persist_error\":1"));
    assert!(stdout.contains("\"target_id\":\"blocked\""));
    assert!(stdout.contains("\"error\":{\"kind\":\"io\""));
    assert!(stderr.is_empty());
}

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
            "{}\n[[notifications]]\nname = \"broken\"\non = [\"initialized\"]\nshell = \"/bin/sh\"\ncommand = \"echo hook-broke >&2; exit 7\"\ntimeout_ms = 1000\n",
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
    assert!(stdout.contains("\"initialized\":2"));
    assert!(stdout.contains("\"target_id\":\"broken\""));
    assert!(stdout.contains("\"delivered\":false"));
    assert!(stdout.contains("hook-broke"));
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
    assert!(stdout.contains("\"run_outcome\":\"failed_permanent\""));
    assert!(stdout.contains("\"reason_code\":\"state_invalid\""));
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
fn run_and_status_return_fatal_when_target_toml_is_unreadable_as_a_file() {
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
    assert!(stdout.contains("\"schema_name\":\"ffhn.status_report\""));
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
    assert!(stdout.contains("\"schema_name\":\"ffhn.status_report\""));
    assert!(stdout.contains("\"target_status\":\"invalid\""));
    assert!(stdout.contains("\"reason_code\":\"state_invalid\""));
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
    assert!(stdout.contains("\"target_status\":\"invalid\""));
    assert!(stdout.contains("\"reason_code\":\"state_invalid\""));
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
schema_version = 1
target_id = "demo_file"
display_name = "Demo File"
enabled = true

[target]
kind = "file"
file_path = {source_path:?}

[fetch]
engine = "file"
follow_redirects = false
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

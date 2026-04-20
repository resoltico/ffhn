use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::thread;
use tempfile::tempdir;

use super::*;
use crate::execute::{collect_watch_root_directories, discover_watch_root_targets};
use ffhn_core::{
    BATCH_RUN_REPORT_SCHEMA_NAME, CLI_OPERATION_RUN_ID, CLI_OPERATION_STATUS_ID,
    RUN_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_NAME, cli_contract, cli_operation,
    document_write_error, duplicate_target_ids_usage_error, positive_batch_concurrency_usage_error,
};

fn run_vec(args: Vec<String>) -> (i32, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit_code = run(args, &mut stdout, &mut stderr);
    (
        exit_code,
        String::from_utf8(stdout).expect("stdout utf8"),
        String::from_utf8(stderr).expect("stderr utf8"),
    )
}

fn workspace_package_field(manifest: &str, field: &str) -> Option<String> {
    let mut in_workspace_package = false;

    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_workspace_package = trimmed == "[workspace.package]";
            continue;
        }

        if !in_workspace_package {
            continue;
        }

        if let Some(value) = trimmed.strip_prefix(&format!("{field} = \""))
            && let Some(value) = value.strip_suffix('"')
        {
            return Some(value.to_owned());
        }
    }

    None
}

struct BrokenWriter;

impl Write for BrokenWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("broken writer"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("broken writer"))
    }
}

fn serve_once(
    status_line: &'static str,
    content_type: &'static str,
    body: &'static str,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind server");
    let address = listener.local_addr().expect("server addr");
    let url = format!("http://{address}");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request);
        let raw = format!(
            "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(raw.as_bytes());
    });
    (url, handle)
}

fn write_named_http_target(
    watch_root: &Path,
    dir_name: &str,
    declared_target_id: &str,
    source_url: &str,
    enabled: bool,
) {
    let target_dir = watch_root.join(dir_name);
    fs::create_dir_all(&target_dir).expect("create target dir");
    fs::write(
        target_dir.join("target.toml"),
        format!(
            r#"
schema_name = "ffhn.target"
schema_version = 1
target_id = "{declared_target_id}"
display_name = "{dir_name}"
enabled = {enabled}

[target]
kind = "http"
source_url = "{source_url}"

[fetch]
engine = "http"
method = "GET"
timeout_ms = 15000
max_bytes = 2000000
user_agent = "ffhn/2.0.0"
follow_redirects = true
accept = "text/html"

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
    .expect("write target.toml");
}

fn write_named_file_target(
    watch_root: &Path,
    dir_name: &str,
    declared_target_id: &str,
    source_path: &Path,
    enabled: bool,
) {
    let target_dir = watch_root.join(dir_name);
    fs::create_dir_all(&target_dir).expect("create target dir");
    fs::write(
        target_dir.join("target.toml"),
        format!(
            r#"
schema_name = "ffhn.target"
schema_version = 1
target_id = "{declared_target_id}"
display_name = "{dir_name}"
enabled = {enabled}

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
    .expect("write target.toml");
}

fn write_target(watch_root: &Path, source_url: &str, enabled: bool) {
    write_named_http_target(watch_root, "demo", "demo", source_url, enabled);
}

#[test]
fn target_command_defaults_watch_root() {
    let cli = parse_cli(["ffhn", "run", "--target", "demo"]).expect("parse cli");

    assert_eq!(
        cli.command,
        Command::Run(RunCommand {
            watch_root: "watchlist".into(),
            targets: vec!["demo".to_owned()],
            all: false,
            jobs: 1,
            dry_run: false,
        })
    );
}

#[test]
fn parse_cli_rejects_non_numeric_jobs_and_unknown_internal_operation_ids() {
    let error = parse_cli(["ffhn", "run", "--target", "demo", "--jobs", "bogus"])
        .expect_err("non-numeric jobs");
    assert!(
        error
            .to_string()
            .contains(positive_batch_concurrency_usage_error())
    );

    let bogus_matches = clap::Command::new("ffhn")
        .subcommand(clap::Command::new("bogus"))
        .try_get_matches_from(["ffhn", "bogus"])
        .expect("bogus matches");
    let error = matches_to_cli(&bogus_matches).expect_err("unknown operation");
    assert!(
        error
            .to_string()
            .contains("unsupported FFHN operation id: bogus")
    );
}

#[test]
fn cli_help_and_parser_render_the_core_contract() {
    let mut root_help = Vec::new();
    build_cli_command()
        .write_long_help(&mut root_help)
        .expect("write help");
    let root_help = String::from_utf8(root_help).expect("help utf8");

    for operation in cli_contract().operations {
        assert!(root_help.contains(operation.id));
        assert!(root_help.contains(operation.help_summary));
    }

    let run = cli_operation(CLI_OPERATION_RUN_ID).expect("run operation");
    let (exit_code, run_help, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        CLI_OPERATION_RUN_ID.to_owned(),
        "--help".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    assert!(stderr.is_empty());
    for argument in run.arguments {
        assert!(run_help.contains(&format!("--{}", argument.long_name)));
        assert!(run_help.contains(argument.help_summary));
    }

    let status = cli_operation(CLI_OPERATION_STATUS_ID).expect("status operation");
    assert_eq!(status.display_label, "Status");
}

#[test]
fn metadata_matches_workspace_package_fields() {
    let workspace_manifest = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate dir")
            .parent()
            .expect("workspace root")
            .join("Cargo.toml"),
    )
    .expect("workspace manifest");
    let workspace_version =
        workspace_package_field(&workspace_manifest, "version").expect("workspace version");
    let workspace_description =
        workspace_package_field(&workspace_manifest, "description").expect("workspace description");

    assert_eq!(FFHN_VERSION, workspace_version);
    assert_eq!(FFHN_DESCRIPTION, workspace_description);
}

#[test]
fn run_covers_root_help_help_version_and_parse_error_modes() {
    let (exit_code, stdout, stderr) = run_vec(vec!["ffhn".to_owned()]);
    assert_eq!(exit_code, 0);
    assert!(stdout.contains("Usage: ffhn"));
    assert!(stdout.contains("status"));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run_vec(vec!["ffhn".to_owned(), "--help".to_owned()]);
    assert_eq!(exit_code, 0);
    assert!(stdout.contains(FFHN_DESCRIPTION));
    assert!(stdout.contains("--version"));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run_vec(vec!["ffhn".to_owned(), "--version".to_owned()]);
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, format!("{}\n", version_banner()));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "--version".to_owned(),
        "--help".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, format!("ffhn {FFHN_VERSION}\n"));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "status".to_owned(),
        "--version".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, format!("{}\n", version_banner()));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run_vec(vec!["ffhn".to_owned(), "bogus".to_owned()]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains("unrecognized subcommand 'bogus'"));

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--target".to_owned(),
        "demo".to_owned(),
        "--jobs".to_owned(),
        "0".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains(positive_batch_concurrency_usage_error()));

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--target".to_owned(),
        "demo".to_owned(),
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains(&duplicate_target_ids_usage_error("demo")));
}

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
fn discover_watch_root_targets_covers_missing_disabled_invalid_and_non_utf_dirs() {
    let temp = tempdir().expect("tempdir");
    let missing = temp.path().join("missing");
    assert!(
        discover_watch_root_targets(&missing)
            .expect("missing watch root")
            .is_empty()
    );

    let watch_root = temp.path().join("watchlist");
    write_named_http_target(&watch_root, "demo", "demo", "https://example.com", true);
    write_named_http_target(
        &watch_root,
        "disabled",
        "disabled",
        "https://example.com",
        false,
    );
    write_named_http_target(&watch_root, "invalid", "other", "https://example.com", true);

    assert_eq!(
        discover_watch_root_targets(&watch_root).expect("discover targets"),
        vec!["demo".to_owned(), "invalid".to_owned()]
    );
}

#[test]
fn collect_watch_root_directories_surfaces_iteration_and_metadata_errors() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    fs::create_dir_all(&watch_root).expect("create watch root");
    let missing_path = watch_root.join("missing");

    let entry_error =
        collect_watch_root_directories(&watch_root, vec![Err(io::Error::other("boom"))])
            .expect_err("entry iteration error");
    assert!(matches!(
        entry_error,
        ffhn_core::CoreError::Io { path, .. } if path == watch_root
    ));

    let metadata_error =
        collect_watch_root_directories(&watch_root, vec![Ok(missing_path.clone())])
            .expect_err("metadata error");
    assert!(matches!(
        metadata_error,
        ffhn_core::CoreError::Io { path, .. } if path == missing_path
    ));
}

#[test]
fn collect_watch_root_directories_keeps_only_directory_entries() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let directory_path = watch_root.join("demo");
    let file_path = watch_root.join("note.txt");
    fs::create_dir_all(&directory_path).expect("create target directory");
    fs::write(&file_path, "ignore").expect("write non-directory entry");

    let directories = collect_watch_root_directories(
        &watch_root,
        vec![Ok(directory_path.clone()), Ok(file_path)],
    )
    .expect("directory entries");

    assert_eq!(directories, vec![directory_path]);
}

#[cfg(unix)]
#[test]
fn run_command_returns_fatal_when_target_discovery_fails() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    fs::create_dir_all(&watch_root).expect("create watch root");

    let original = fs::metadata(&watch_root)
        .expect("watch root metadata")
        .permissions();
    let mut denied = original.clone();
    denied.set_mode(0o000);
    fs::set_permissions(&watch_root, denied).expect("deny watch root access");

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--all".to_owned(),
    ]);

    fs::set_permissions(&watch_root, original).expect("restore watch root access");

    assert_eq!(exit_code, EXIT_CODE_FATAL);
    assert!(stdout.is_empty());
    assert!(stderr.contains("filesystem error"));
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
    assert!(stdout.contains("\"fatal_error\":\"filesystem error"));
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
    assert!(String::from_utf8(stderr).expect("stderr utf8").contains(
        &document_write_error(BATCH_RUN_REPORT_SCHEMA_NAME).expect("batch run report write error")
    ));

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
    assert_eq!(exit_code, EXIT_CODE_FATAL);
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
}

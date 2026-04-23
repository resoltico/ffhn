use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace crates dir")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn status_emits_one_json_document() {
    let temp = tempdir().expect("tempdir");
    let target_dir = temp.path().join("watchlist").join("demo");
    fs::create_dir_all(&target_dir).expect("create target dir");
    fs::write(
        target_dir.join("target.toml"),
        r#"
schema_name = "ffhn.target"
schema_version = 1
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "http"
source_url = "https://example.com"

[fetch]
engine = "http"
method = "GET"
timeout_ms = 15000
max_bytes = 2000000
user_agent = "ffhn/2.0.0"
follow_redirects = true
accept = "text/html,application/xhtml+xml"

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
"#,
    )
    .expect("write target.toml");

    let mut command = Command::cargo_bin("ffhn").expect("ffhn binary");
    command
        .current_dir(temp.path())
        .args(["status", "--target", "demo"])
        .assert()
        .success()
        .stdout(contains("\"schema_name\":\"ffhn.status_report\""))
        .stdout(contains("\"reason_code\":\"ok\""));
}

#[test]
fn readme_quick_start_file_example_flow_stays_runnable() {
    let repo_root = repo_root();
    let materializer =
        repo_root.join("examples/file-target-with-notifications/materialize-target.sh");
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watch-root");
    let target_file = watch_root.join("release_notes").join("target.toml");
    let watch_root_string = watch_root.to_string_lossy().into_owned();

    let materialized = std::process::Command::new("sh")
        .arg(&materializer)
        .arg(&target_file)
        .output()
        .expect("run materializer");
    assert!(
        materialized.status.success(),
        "{}",
        String::from_utf8_lossy(&materialized.stderr)
    );

    let mut run_once = Command::cargo_bin("ffhn").expect("ffhn binary");
    run_once
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--target",
            "release_notes",
        ])
        .assert()
        .success()
        .stdout(contains("\"schema_name\":\"ffhn.run_report\""))
        .stdout(contains("\"run_outcome\":\"initialized\""))
        .stdout(contains("\"reason_code\":\"ok\""));

    let mut status = Command::cargo_bin("ffhn").expect("ffhn binary");
    status
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "release_notes",
        ])
        .assert()
        .success()
        .stdout(contains("\"schema_name\":\"ffhn.status_report\""))
        .stdout(contains("\"target_status\":\"ready\""));

    let mut run_all = Command::cargo_bin("ffhn").expect("ffhn binary");
    run_all
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--all",
            "--jobs",
            "4",
        ])
        .assert()
        .success()
        .stdout(contains("\"schema_name\":\"ffhn.batch_run_report\""))
        .stdout(contains("\"requested_targets\":[\"release_notes\"]"))
        .stdout(contains("\"max_concurrency\":4"))
        .stdout(contains("\"unchanged\":1"));

    let mut dry_run = Command::cargo_bin("ffhn").expect("ffhn binary");
    dry_run
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--target",
            "release_notes",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(contains("\"schema_name\":\"ffhn.run_report\""))
        .stdout(contains("\"run_mode\":\"dry_run\""))
        .stdout(contains("\"run_outcome\":\"unchanged\""));
}

#[test]
fn run_all_on_missing_watch_root_fails_instead_of_succeeding_empty() {
    let temp = tempdir().expect("tempdir");
    let missing_watch_root = temp.path().join("missing-watch-root");
    let watch_root_string = missing_watch_root.to_string_lossy().into_owned();

    let mut command = Command::cargo_bin("ffhn").expect("ffhn binary");
    command
        .args(["run", "--watch-root", &watch_root_string, "--all"])
        .assert()
        .failure()
        .stderr(contains("filesystem error"))
        .stderr(contains("watch root does not exist"));
}

#[test]
fn run_all_ignores_directories_without_a_target_marker() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watch-root");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    let source_path = temp.path().join("source.html");
    fs::create_dir_all(&watch_root).expect("create watch root");
    fs::write(&source_path, "<html><body><main>Hello</main></body></html>").expect("source");
    fs::create_dir_all(watch_root.join("notes")).expect("create unrelated directory");
    fs::write(watch_root.join("notes").join("README.txt"), "ignore").expect("write unrelated file");
    fs::create_dir_all(watch_root.join("broken")).expect("create broken target dir");
    fs::create_dir_all(watch_root.join("broken").join("target.toml"))
        .expect("create broken target marker directory");
    fs::create_dir_all(watch_root.join("demo")).expect("create demo target dir");
    fs::write(
        watch_root.join("demo").join("target.toml"),
        format!(
            r#"
schema_name = "ffhn.target"
schema_version = 1
target_id = "demo"
display_name = "Demo"
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
    .expect("write demo target");

    let mut command = Command::cargo_bin("ffhn").expect("ffhn binary");
    command
        .args(["run", "--watch-root", &watch_root_string, "--all"])
        .assert()
        .failure()
        .stdout(contains("\"requested_targets\":[\"broken\",\"demo\"]"))
        .stdout(contains("\"target_id\":\"broken\""))
        .stdout(contains("\"fatal_error\":{\"kind\":\"io\""))
        .stdout(predicates::str::contains("\"target_id\":\"notes\"").not());
}

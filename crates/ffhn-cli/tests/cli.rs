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

fn available_powershell_program() -> Option<&'static str> {
    ["pwsh", "powershell", "powershell.exe"]
        .into_iter()
        .find(|program| {
            std::process::Command::new(program)
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-Command",
                    "$PSVersionTable.PSVersion",
                ])
                .output()
                .is_ok_and(|output| output.status.success())
        })
}

fn extract_help_example_block(help_text: &str, header: &str, footer: &str) -> String {
    let mut in_block = false;
    let mut lines = Vec::new();

    for line in help_text.lines() {
        if line == header {
            in_block = true;
            continue;
        }
        if in_block && line == footer {
            break;
        }
        if in_block {
            lines.push(line.strip_prefix("  ").unwrap_or(line));
        }
    }

    let mut block = lines.join("\n");
    block.push('\n');
    block
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
user_agent = "ffhn/example"
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
fn help_output_teaches_target_layout_examples_and_invalid_detail() {
    let mut run_help = Command::cargo_bin("ffhn").expect("ffhn binary");
    run_help
        .args(["run", "--help"])
        .assert()
        .success()
        .stdout(contains("<watch_root>/<target_id>/target.toml"))
        .stdout(contains("target_id = \"demo_file\""))
        .stdout(contains("target_id = \"demo_http\""))
        .stdout(contains("user_agent = \"ffhn/example\""))
        .stdout(contains("accept = \"text/html,application/xhtml+xml\""))
        .stdout(contains(
            "Use lowercase letters or digits, with single internal '-' or '_' separators.",
        ))
        .stdout(contains(
            "The watch root itself must already exist and be a directory.",
        ))
        .stdout(contains(
            "--all scans only immediate subdirectories of the watch root.",
        ))
        .stdout(contains(
            "Valid disabled targets are skipped during --all discovery.",
        ))
        .stdout(contains(
            "Status and --dry-run wait behind any active live run so they inspect one stable target view.",
        ))
        .stdout(contains(
            "Explicit --target requests whose target.toml path is missing or unreadable stay fatal process errors.",
        ));

    let mut status_help = Command::cargo_bin("ffhn").expect("ffhn binary");
    status_help
        .args(["status", "--help"])
        .assert()
        .success()
        .stdout(contains("<watch_root>/<target_id>/target.toml"))
        .stdout(contains("structured error_detail"))
        .stdout(contains(
            "Status waits behind any active live run so it can inspect one stable target view.",
        ))
        .stdout(contains(
            "The watch root itself must already exist and be a directory.",
        ))
        .stdout(contains(
            "Missing or unreadable target.toml paths remain fatal process errors.",
        ));
}

#[test]
fn run_help_http_example_validates_as_written() {
    let help_output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args(["run", "--help"])
        .output()
        .expect("run help");
    assert!(help_output.status.success());
    let help_text = String::from_utf8(help_output.stdout).expect("help utf8");
    let example =
        extract_help_example_block(&help_text, "Minimal HTTP target:", "Discovery notes:");

    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watch-root");
    let target_dir = watch_root.join("demo_http");
    fs::create_dir_all(&target_dir).expect("create target dir");
    fs::write(target_dir.join("target.toml"), example).expect("write help example target");

    let watch_root_string = watch_root.to_string_lossy().into_owned();
    let mut status = Command::cargo_bin("ffhn").expect("ffhn binary");
    status
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo_http",
        ])
        .assert()
        .success()
        .stdout(contains("\"reason_code\":\"ok\""))
        .stdout(contains("\"target_status\":\"pending\""));
}

#[test]
fn invalid_target_and_state_reports_surface_structured_error_detail() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watch-root");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    let target_dir = watch_root.join("demo");
    fs::create_dir_all(&target_dir).expect("create target dir");
    fs::write(target_dir.join("target.toml"), "not = [valid").expect("write invalid target");

    let mut invalid_status = Command::cargo_bin("ffhn").expect("ffhn binary");
    invalid_status
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .assert()
        .success()
        .stdout(contains("\"reason_code\":\"config_invalid\""))
        .stdout(contains("\"error_detail\":{\"kind\":\"toml\""))
        .stdout(contains("target.toml"));

    let mut invalid_run = Command::cargo_bin("ffhn").expect("ffhn binary");
    invalid_run
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .assert()
        .failure()
        .stdout(contains("\"reason_code\":\"config_invalid\""))
        .stdout(contains("\"error_detail\":{\"kind\":\"toml\""))
        .stdout(contains("target.toml"));

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
    .expect("write contract-invalid target");

    let mut contract_invalid_status = Command::cargo_bin("ffhn").expect("ffhn binary");
    contract_invalid_status
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .assert()
        .success()
        .stdout(contains("\"reason_code\":\"config_invalid\""))
        .stdout(contains("\"error_detail\":{\"kind\":\"contract\""))
        .stdout(contains("fetch.user_agent must not be empty"))
        .stdout(contains("target.toml"));

    let mut contract_invalid_run = Command::cargo_bin("ffhn").expect("ffhn binary");
    contract_invalid_run
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .assert()
        .failure()
        .stdout(contains("\"reason_code\":\"config_invalid\""))
        .stdout(contains("\"error_detail\":{\"kind\":\"contract\""))
        .stdout(contains("fetch.user_agent must not be empty"))
        .stdout(contains("target.toml"));

    fs::write(
        target_dir.join("target.toml"),
        r#"
schema_name = "ffhn.target"
schema_version = 1
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "file"
file_path = "/tmp/source.html"

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
"#,
    )
    .expect("rewrite valid target");
    fs::write(target_dir.join("state.json"), "{not json").expect("write invalid state");

    let mut invalid_state = Command::cargo_bin("ffhn").expect("ffhn binary");
    invalid_state
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .assert()
        .success()
        .stdout(contains("\"reason_code\":\"state_invalid\""))
        .stdout(contains("\"error_detail\":{\"kind\":\"json\""))
        .stdout(contains("state.json"));

    fs::write(
        target_dir.join("state.json"),
        r#"{
  "schema_name": "ffhn.state",
  "schema_version": 1,
  "target_id": "demo",
  "state_phase": "has_baseline",
  "last_run_at": "2026-04-05T10:15:30Z",
  "last_run_outcome": "changed",
  "last_reason_code": "ok",
  "current_snapshot": null,
  "snapshot_history": []
}"#,
    )
    .expect("write contract-invalid state");

    let mut contract_invalid_state = Command::cargo_bin("ffhn").expect("ffhn binary");
    contract_invalid_state
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .assert()
        .success()
        .stdout(contains("\"reason_code\":\"state_invalid\""))
        .stdout(contains("\"error_detail\":{\"kind\":\"contract\""))
        .stdout(contains(
            "state_phase has_baseline requires current_snapshot",
        ))
        .stdout(contains("state.json"));
}

#[test]
fn contract_invalid_extraction_reports_surface_contract_error_and_artifact_path() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watch-root");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    let target_dir = watch_root.join("demo");
    let source_path = temp.path().join("source.html");
    fs::create_dir_all(&target_dir).expect("create target dir");
    fs::write(&source_path, "<html><main>Hello</main></html>").expect("write source");
    fs::write(
        target_dir.join("target.toml"),
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
    .expect("write target");

    let mut run_once = Command::cargo_bin("ffhn").expect("ffhn binary");
    run_once
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .assert()
        .success()
        .stdout(contains("\"run_outcome\":\"initialized\""));

    fs::write(
        target_dir.join("snapshots/current/extraction.json"),
        r#"{
  "schema_name": "ffhn.extraction_record",
  "schema_version": 1,
  "interop_profile": "htmlcut-v0",
  "htmlcut_plan_digest_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "htmlcut_result_digest_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "comparison_input_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "outer_html_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "strategy_kind": "css_selector",
  "selection_mode": "single",
  "output_kind": "outer_html",
  "candidate_count": 1,
  "selected_candidate_index": 1,
  "match_metadata": {"selector":"main"},
  "warning_codes": [],
  "created_at": "2026-04-05T10:15:30Z"
}"#,
    )
    .expect("write contract-invalid extraction record");

    let mut invalid_extraction = Command::cargo_bin("ffhn").expect("ffhn binary");
    invalid_extraction
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .assert()
        .success()
        .stdout(contains("\"reason_code\":\"integrity_mismatch\""))
        .stdout(contains("\"error_detail\":{\"kind\":\"contract\""))
        .stdout(contains(
            "ffhn.extraction_record interop_profile must match the FFHN HTMLCut profile",
        ))
        .stdout(contains("snapshots/current/extraction.json"));
}

#[test]
fn missing_target_paths_are_fatal_process_errors() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watch-root");
    fs::create_dir_all(&watch_root).expect("create watch root");
    let watch_root_string = watch_root.to_string_lossy().into_owned();

    let mut missing_status = Command::cargo_bin("ffhn").expect("ffhn binary");
    missing_status
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "missing_target",
        ])
        .assert()
        .code(3)
        .stderr(contains("target.toml"));

    let mut missing_run = Command::cargo_bin("ffhn").expect("ffhn binary");
    missing_run
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--target",
            "missing_target",
        ])
        .assert()
        .code(3)
        .stderr(contains("target.toml"));
}

#[test]
fn explicit_single_target_commands_require_a_real_watch_root_directory() {
    let temp = tempdir().expect("tempdir");
    let missing_watch_root = temp.path().join("missing-watch-root");
    let missing_watch_root_string = missing_watch_root.to_string_lossy().into_owned();

    let mut missing_status = Command::cargo_bin("ffhn").expect("ffhn binary");
    missing_status
        .args([
            "status",
            "--watch-root",
            &missing_watch_root_string,
            "--target",
            "demo",
        ])
        .assert()
        .code(3)
        .stderr(contains("watch root does not exist"))
        .stderr(contains("missing-watch-root"))
        .stderr(predicates::str::contains("target.toml").not());

    let watch_root_file = temp.path().join("watch-root.txt");
    fs::write(&watch_root_file, "not a directory").expect("write watch-root file");
    let watch_root_file_string = watch_root_file.to_string_lossy().into_owned();

    let mut invalid_status = Command::cargo_bin("ffhn").expect("ffhn binary");
    invalid_status
        .args([
            "status",
            "--watch-root",
            &watch_root_file_string,
            "--target",
            "demo",
        ])
        .assert()
        .code(3)
        .stderr(contains("watch root is not a directory"))
        .stderr(predicates::str::contains("target.toml").not());

    let mut invalid_dry_run = Command::cargo_bin("ffhn").expect("ffhn binary");
    invalid_dry_run
        .args([
            "run",
            "--watch-root",
            &watch_root_file_string,
            "--target",
            "demo",
            "--dry-run",
        ])
        .assert()
        .code(3)
        .stderr(contains("watch root is not a directory"))
        .stderr(predicates::str::contains("target.toml").not());
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
fn powershell_quick_start_file_example_flow_stays_runnable_when_available() {
    let Some(program) = available_powershell_program() else {
        return;
    };

    let repo_root = repo_root();
    let materializer =
        repo_root.join("examples/file-target-with-notifications/materialize-target.ps1");
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watch-root");
    let target_file = watch_root.join("release_notes").join("target.toml");
    let watch_root_string = watch_root.to_string_lossy().into_owned();

    let materialized = std::process::Command::new(program)
        .args(["-NoLogo", "-NoProfile", "-File"])
        .arg(&materializer)
        .arg(&target_file)
        .output()
        .expect("run powershell materializer");
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

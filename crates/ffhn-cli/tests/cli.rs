use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use ffhn_core::{
    BaselinePhase, BatchRunReport, ProcessErrorKind, RunFailureCause, RunMode, RunOutcome,
    RunReport, StatusReport, StatusSummary,
};
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

fn stdout_string(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout utf8")
}

fn parse_run_report_output(output: &std::process::Output) -> RunReport {
    serde_json::from_slice(&output.stdout).expect("run report json")
}

fn parse_batch_run_report_output(output: &std::process::Output) -> BatchRunReport {
    serde_json::from_slice(&output.stdout).expect("batch run report json")
}

fn parse_status_report_output(output: &std::process::Output) -> StatusReport {
    serde_json::from_slice(&output.stdout).expect("status report json")
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
schema_version = 3
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

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .current_dir(temp.path())
        .args(["status", "--target", "demo"])
        .output()
        .expect("status output");
    assert!(output.status.success());
    let report = parse_status_report_output(&output);
    assert_eq!(report.schema_name(), "ffhn.status_report");
    assert_eq!(report.enabled(), Some(true));
    assert!(report.status().is_pending());
    assert_eq!(report.baseline_phase(), Some(BaselinePhase::NeverSucceeded));
}

#[test]
fn help_output_separates_grammar_examples_output_and_operational_notes() {
    let run_help = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args(["run", "--help"])
        .output()
        .expect("run help");
    assert!(run_help.status.success());
    let run_help = stdout_string(&run_help);
    assert!(run_help.contains(
        "Usage: ffhn run (--target <ID>... | --all) [--watch-root <PATH>] [--jobs <N>] [--dry-run] [--format <FORMAT>]"
    ));
    assert!(run_help.contains("Examples:"));
    assert!(run_help.contains("ffhn run --target demo"));
    assert!(run_help.contains("ffhn run --all --jobs 4"));
    assert!(run_help.contains("Output:"));
    assert!(run_help.contains("ffhn.run_report"));
    assert!(run_help.contains("selected format"));
    assert!(run_help.contains("ffhn.batch_run_report"));
    assert!(run_help.contains("Operational notes:"));
    assert!(
        run_help.contains(
            "Use lowercase letters or digits, with single internal '-' or '_' separators."
        )
    );
    assert!(run_help.contains("The watch root must already exist and be a directory."));
    assert!(run_help.contains(
        "`--all` discovers only immediate watch-root subdirectories containing `target.toml`."
    ));
    assert!(run_help.contains(
        "Disabled targets are discovered and reported normally, but they are not executed."
    ));
    assert!(run_help.contains(
        "Explicit `--target` requests whose `target.toml` path is missing or unreadable emit structured `target_unavailable` results instead of raw fatal stderr."
    ));
    assert!(!run_help.contains("Target layout:"));
    assert!(!run_help.contains("schema_name = \"ffhn.target\""));

    let status_help = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args(["status", "--help"])
        .output()
        .expect("status help");
    assert!(status_help.status.success());
    let status_help = stdout_string(&status_help);
    assert!(
        status_help
            .contains("Usage: ffhn status --target <ID> [--watch-root <PATH>] [--format <FORMAT>]")
    );
    assert!(status_help.contains("Examples:"));
    assert!(status_help.contains("ffhn status --target demo"));
    assert!(status_help.contains("Output:"));
    assert!(status_help.contains("ffhn.status_report"));
    assert!(status_help.contains("selected format"));
    assert!(status_help.contains("structured `status.error_detail`"));
    assert!(status_help.contains("Operational notes:"));
    assert!(
        status_help.contains(
            "`enabled = true|false` so disablement stays separate from baseline readiness"
        )
    );
    assert!(status_help.contains(
        "Status waits behind any active live run so it can inspect one stable target view."
    ));
    assert!(status_help.contains("The watch root must already exist and be a directory."));
    assert!(!status_help.contains("Target layout:"));
}

#[test]
fn invalid_target_and_state_reports_surface_structured_error_detail() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watch-root");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    let target_dir = watch_root.join("demo");
    fs::create_dir_all(&target_dir).expect("create target dir");
    fs::write(target_dir.join("target.toml"), "not = [valid").expect("write invalid target");

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .output()
        .expect("invalid status");
    assert!(output.status.success());
    let report = parse_status_report_output(&output);
    assert_eq!(report.enabled(), None);
    assert!(report.status().is_invalid());
    assert!(matches!(
        report.status(),
        StatusSummary::InvalidConfig { .. }
    ));
    assert_eq!(
        report.error_detail().expect("error detail").kind(),
        ProcessErrorKind::Toml
    );
    assert!(
        report
            .error_detail()
            .expect("error detail")
            .path()
            .expect("path")
            .contains("target.toml")
    );

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .output()
        .expect("invalid run");
    assert!(!output.status.success());
    let report = parse_run_report_output(&output);
    assert_eq!(report.run_outcome(), RunOutcome::FailedPermanent);
    assert_eq!(report.failure_cause(), Some(RunFailureCause::ConfigInvalid));
    assert_eq!(
        report.error_detail().expect("error detail").kind(),
        ProcessErrorKind::Toml
    );

    fs::write(
        target_dir.join("target.toml"),
        r#"
schema_name = "ffhn.target"
schema_version = 3
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

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .output()
        .expect("contract invalid status");
    assert!(output.status.success());
    let report = parse_status_report_output(&output);
    assert_eq!(report.enabled(), None);
    assert!(report.status().is_invalid());
    assert!(matches!(
        report.status(),
        StatusSummary::InvalidConfig { .. }
    ));
    assert_eq!(
        report.error_detail().expect("error detail").kind(),
        ProcessErrorKind::Contract
    );
    assert!(
        report
            .error_detail()
            .expect("error detail")
            .message()
            .contains("fetch.user_agent must not be empty")
    );

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .output()
        .expect("contract invalid run");
    assert!(!output.status.success());
    let report = parse_run_report_output(&output);
    assert_eq!(report.run_outcome(), RunOutcome::FailedPermanent);
    assert_eq!(report.failure_cause(), Some(RunFailureCause::ConfigInvalid));
    assert_eq!(
        report.error_detail().expect("error detail").kind(),
        ProcessErrorKind::Contract
    );
    assert!(
        report
            .error_detail()
            .expect("error detail")
            .message()
            .contains("fetch.user_agent must not be empty")
    );

    let state_test_file_path = std::env::temp_dir().join("source.html");
    fs::write(
        target_dir.join("target.toml"),
        format!(
            r#"
schema_name = "ffhn.target"
schema_version = 3
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "file"
file_path = {state_test_file_path:?}

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
    .expect("rewrite valid target");
    fs::write(target_dir.join("state.json"), "{not json").expect("write invalid state");

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .output()
        .expect("invalid state");
    assert!(output.status.success());
    let report = parse_status_report_output(&output);
    assert_eq!(report.enabled(), Some(true));
    assert!(report.status().is_invalid());
    assert!(matches!(
        report.status(),
        StatusSummary::InvalidState { .. }
    ));
    assert_eq!(
        report.error_detail().expect("error detail").kind(),
        ProcessErrorKind::Json
    );

    fs::write(
        target_dir.join("state.json"),
        r#"{
  "schema_name": "ffhn.state",
  "schema_version": 3,
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

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .output()
        .expect("contract invalid state");
    assert!(output.status.success());
    let report = parse_status_report_output(&output);
    assert_eq!(report.enabled(), Some(true));
    assert!(report.status().is_invalid());
    assert!(matches!(
        report.status(),
        StatusSummary::InvalidState { .. }
    ));
    assert_eq!(
        report.error_detail().expect("error detail").kind(),
        ProcessErrorKind::Contract
    );
    assert!(
        report
            .error_detail()
            .expect("error detail")
            .message()
            .contains("unknown field `current_snapshot`")
    );
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
schema_version = 3
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

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .output()
        .expect("initial run");
    assert!(output.status.success());
    let report = parse_run_report_output(&output);
    assert_eq!(report.run_outcome(), RunOutcome::Initialized);

    fs::write(
        target_dir.join("snapshots/current/extraction.json"),
        r#"{
  "schema_name": "wrong",
  "schema_version": 3,
  "comparison_input_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "outer_html_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "selection_kind": "css_selector",
  "selection_match": "single",
  "output_kind": "outer_html",
  "candidate_count": 1,
  "selected_candidate_index": 1,
  "selection_evidence": {
    "kind": "css_selector",
    "path": "html > body > main",
    "tag_name": "main"
  },
  "warning_codes": [],
  "created_at": "2026-04-05T10:15:30Z"
}"#,
    )
    .expect("write contract-invalid extraction record");

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .output()
        .expect("invalid extraction status");
    assert!(output.status.success());
    let report = parse_status_report_output(&output);
    assert_eq!(report.enabled(), Some(true));
    assert!(report.status().is_invalid());
    assert!(matches!(
        report.status(),
        StatusSummary::IntegrityMismatch { .. }
    ));
    assert_eq!(
        report.error_detail().expect("error detail").kind(),
        ProcessErrorKind::Contract
    );
    assert!(
        report
            .error_detail()
            .expect("error detail")
            .message()
            .contains("schema_name must be \"ffhn.extraction_record\"")
    );
    assert!(
        report
            .error_detail()
            .expect("error detail")
            .path()
            .expect("error path")
            .contains("snapshots/current/extraction.json")
    );
}

#[test]
fn missing_target_paths_emit_structured_unavailable_documents() {
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
        .success()
        .stdout(contains("\"kind\":\"unavailable_target\""))
        .stdout(contains("target.toml"));

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
        .failure()
        .stdout(contains("\"cause\":\"target_unavailable\""))
        .stdout(contains("target.toml"));
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
        .stderr(contains("error: filesystem error"))
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
        .stderr(contains("error: filesystem error"))
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
        .stderr(contains("error: filesystem error"))
        .stderr(contains("watch root is not a directory"))
        .stderr(predicates::str::contains("target.toml").not());
}

#[test]
fn run_without_target_selection_names_the_all_alternative() {
    Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args(["run"])
        .assert()
        .code(2)
        .stderr(contains("one of '--target <ID>' or '--all' is required"))
        .stderr(contains(
            "Usage: ffhn run (--target <ID>... | --all) [--watch-root <PATH>] [--jobs <N>] [--dry-run]",
        ));
}

#[cfg(unix)]
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

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--target",
            "release_notes",
        ])
        .output()
        .expect("readme run");
    assert!(output.status.success(), "{}", stdout_string(&output));
    let report = parse_run_report_output(&output);
    assert_eq!(report.run_outcome(), RunOutcome::Initialized);
    assert_eq!(report.failure_cause(), None);

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "release_notes",
        ])
        .output()
        .expect("readme status");
    assert!(output.status.success(), "{}", stdout_string(&output));
    let report = parse_status_report_output(&output);
    assert_eq!(report.enabled(), Some(true));
    assert!(report.status().is_ready());
    assert_eq!(report.baseline_phase(), Some(BaselinePhase::HasBaseline));

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--all",
            "--jobs",
            "4",
        ])
        .output()
        .expect("readme run all");
    assert!(output.status.success(), "{}", stdout_string(&output));
    let report = parse_batch_run_report_output(&output);
    assert_eq!(report.requested_targets(), ["release_notes"]);
    assert_eq!(report.max_concurrency(), 4);
    assert_eq!(report.outcome_counts().unchanged(), 1);

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--target",
            "release_notes",
            "--dry-run",
        ])
        .output()
        .expect("readme dry run");
    assert!(output.status.success(), "{}", stdout_string(&output));
    let report = parse_run_report_output(&output);
    assert_eq!(report.run_mode(), RunMode::DryRun);
    assert_eq!(report.run_outcome(), RunOutcome::Unchanged);
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

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--target",
            "release_notes",
        ])
        .output()
        .expect("powershell run");
    assert!(output.status.success(), "{}", stdout_string(&output));
    let report = parse_run_report_output(&output);
    assert_eq!(report.run_outcome(), RunOutcome::Initialized);
    assert_eq!(report.failure_cause(), None);

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "release_notes",
        ])
        .output()
        .expect("powershell status");
    assert!(output.status.success(), "{}", stdout_string(&output));
    let report = parse_status_report_output(&output);
    assert_eq!(report.enabled(), Some(true));
    assert!(report.status().is_ready());
    assert_eq!(report.baseline_phase(), Some(BaselinePhase::HasBaseline));

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--all",
            "--jobs",
            "4",
        ])
        .output()
        .expect("powershell run all");
    assert!(output.status.success(), "{}", stdout_string(&output));
    let report = parse_batch_run_report_output(&output);
    assert_eq!(report.requested_targets(), ["release_notes"]);
    assert_eq!(report.max_concurrency(), 4);
    assert_eq!(report.outcome_counts().unchanged(), 1);

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "run",
            "--watch-root",
            &watch_root_string,
            "--target",
            "release_notes",
            "--dry-run",
        ])
        .output()
        .expect("powershell dry run");
    assert!(output.status.success(), "{}", stdout_string(&output));
    let report = parse_run_report_output(&output);
    assert_eq!(report.run_mode(), RunMode::DryRun);
    assert_eq!(report.run_outcome(), RunOutcome::Unchanged);
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
fn status_reports_disabled_enablement_separately_from_pending_baseline_state() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watch-root");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    let source_path = temp.path().join("source.html");
    let target_dir = watch_root.join("demo");
    fs::create_dir_all(&target_dir).expect("create target dir");
    fs::write(&source_path, "<html><body><main>Hello</main></body></html>").expect("source");
    fs::write(
        target_dir.join("target.toml"),
        format!(
            r#"
schema_name = "ffhn.target"
schema_version = 3
target_id = "demo"
display_name = "Demo"
enabled = false

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
    .expect("write disabled target");

    let output = Command::cargo_bin("ffhn")
        .expect("ffhn binary")
        .args([
            "status",
            "--watch-root",
            &watch_root_string,
            "--target",
            "demo",
        ])
        .output()
        .expect("disabled status");
    assert!(output.status.success());
    let report = parse_status_report_output(&output);
    assert_eq!(report.enabled(), Some(false));
    assert!(matches!(report.status(), StatusSummary::Pending));
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
schema_version = 3
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
        .stdout(contains("\"cause\":\"target_unavailable\""))
        .stdout(predicates::str::contains("\"target_id\":\"notes\"").not());
}

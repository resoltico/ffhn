//! Command-adapter integration scenarios.

use super::*;
use std::fs;
use std::io;
use tempfile::tempdir;

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("writer failed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn write_target(root: &std::path::Path, id: &str, value: &str) {
    let directory = root.join(id);
    fs::create_dir_all(&directory).expect("target directory");
    let source = directory.join("source.json");
    let source_path = format!("{:?}", source.to_string_lossy());
    fs::write(&source, value).expect("source");
    fs::write(
            directory.join("target.toml"),
            format!(
                "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"{id}\"\ndisplay_name = \"{id}\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
            ),
        )
        .expect("target");
}

#[test]
fn command_adapter_covers_help_run_batch_status_reset_and_output_failures() {
    let temporary = tempdir().expect("temporary directory");
    let root = temporary.path();
    write_target(root, "demo", r#"{"value":1}"#);
    write_target(root, "second", r#"{"value":2}"#);
    let root_text = root.to_string_lossy().to_string();

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    assert_eq!(run(["ffhn", "--help"], &mut stdout, &mut stderr), 0);
    assert!(
        String::from_utf8(stdout.clone())
            .expect("help")
            .contains("Usage")
    );
    assert_eq!(run(["ffhn"], &mut Vec::new(), &mut stderr), EXIT_CODE_USAGE);
    assert_eq!(
        run(
            ["ffhn", "run", "--all", "--watch-root", "/missing"],
            &mut Vec::new(),
            &mut stderr
        ),
        EXIT_CODE_FATAL
    );

    let single = [
        "ffhn",
        "run",
        "--watch-root",
        &root_text,
        "--target",
        "demo",
        "--format",
        "json",
    ];
    assert_eq!(run(single, &mut stdout, &mut stderr), 0);
    assert!(
        String::from_utf8(stdout)
            .expect("run JSON")
            .contains("initialized")
    );
    assert_eq!(
        run(
            [
                "ffhn",
                "run",
                "--watch-root",
                &root_text,
                "--target",
                "demo",
                "--dry-run"
            ],
            &mut Vec::new(),
            &mut stderr,
        ),
        0
    );
    assert_eq!(
        run(
            [
                "ffhn",
                "status",
                "--watch-root",
                &root_text,
                "--target",
                "demo",
                "--format",
                "summary"
            ],
            &mut Vec::new(),
            &mut stderr,
        ),
        0
    );
    assert_eq!(
        run(
            ["ffhn", "run", "--all", "--watch-root", &root_text],
            &mut Vec::new(),
            &mut stderr,
        ),
        0
    );
    assert_eq!(
        run(
            [
                "ffhn",
                "run",
                "--all",
                "--watch-root",
                &root_text,
                "--dry-run"
            ],
            &mut Vec::new(),
            &mut stderr,
        ),
        0
    );
    assert_eq!(
        run(
            [
                "ffhn",
                "reset",
                "--watch-root",
                &root_text,
                "--target",
                "demo"
            ],
            &mut Vec::new(),
            &mut stderr,
        ),
        0
    );
    assert_eq!(
        run(
            [
                "ffhn",
                "status",
                "--watch-root",
                "/missing",
                "--target",
                "demo"
            ],
            &mut Vec::new(),
            &mut stderr,
        ),
        EXIT_CODE_FATAL
    );
    assert_eq!(
        run(
            [
                "ffhn",
                "reset",
                "--watch-root",
                "/missing",
                "--target",
                "demo"
            ],
            &mut Vec::new(),
            &mut stderr,
        ),
        EXIT_CODE_FATAL
    );

    fs::write(
        root.join("second/source.json"),
        r#"{"value":"not-an-integer"}"#,
    )
    .expect("failed batch source");
    assert_eq!(
        run(
            ["ffhn", "run", "--all", "--watch-root", &root_text],
            &mut Vec::new(),
            &mut stderr,
        ),
        EXIT_CODE_RUN_FAILED
    );

    let single_error = tempdir().expect("single error root");
    write_target(single_error.path(), "demo", r#"{"value":1}"#);
    fs::create_dir_all(single_error.path().join(".ffhn-locks/demo.lock")).expect("lock directory");
    let single_error_root = single_error.path().to_string_lossy().to_string();
    assert_eq!(
        run(
            [
                "ffhn",
                "run",
                "--watch-root",
                &single_error_root,
                "--target",
                "demo",
                "--dry-run"
            ],
            &mut Vec::new(),
            &mut stderr,
        ),
        EXIT_CODE_FATAL
    );

    let batch_error = tempdir().expect("batch error root");
    write_target(batch_error.path(), "demo", r#"{"value":1}"#);
    write_target(batch_error.path(), "second", r#"{"value":2}"#);
    fs::write(batch_error.path().join(".ffhn-locks"), "not a directory")
        .expect("lock parent blocker");
    let batch_error_root = batch_error.path().to_string_lossy().to_string();
    assert_eq!(
        run(
            [
                "ffhn",
                "run",
                "--watch-root",
                &batch_error_root,
                "--target",
                "demo",
                "--target",
                "second"
            ],
            &mut Vec::new(),
            &mut stderr,
        ),
        EXIT_CODE_FATAL
    );

    let mut failing = FailingWriter;
    assert_eq!(
        run(["ffhn", "--help"], &mut failing, &mut stderr),
        EXIT_CODE_FATAL
    );
    assert_eq!(run(single, &mut failing, &mut stderr), EXIT_CODE_FATAL);
    assert_eq!(
        run(
            [
                "ffhn",
                "run",
                "--all",
                "--watch-root",
                &root_text,
                "--dry-run"
            ],
            &mut failing,
            &mut stderr,
        ),
        EXIT_CODE_FATAL
    );
    assert_eq!(
        run(
            [
                "ffhn",
                "status",
                "--watch-root",
                &root_text,
                "--target",
                "second"
            ],
            &mut failing,
            &mut stderr,
        ),
        EXIT_CODE_FATAL
    );
    assert_eq!(
        run(
            [
                "ffhn",
                "reset",
                "--watch-root",
                &root_text,
                "--target",
                "second"
            ],
            &mut failing,
            &mut stderr,
        ),
        EXIT_CODE_FATAL
    );
    assert!(failing.flush().is_ok());
    assert_eq!(write_error(&mut stderr), EXIT_CODE_FATAL);
}

#[test]
fn all_selection_rejects_invalid_discovered_ids_and_run_failures_return_one() {
    let temporary = tempdir().expect("temporary directory");
    let root = temporary.path();
    write_target(root, "valid", r#"{"value":"not-an-integer"}"#);
    fs::create_dir_all(root.join("invalid!")).expect("invalid directory");
    fs::write(root.join("invalid!/target.toml"), "placeholder").expect("placeholder");
    let root_text = root.to_string_lossy().to_string();
    assert_eq!(
        run(
            ["ffhn", "run", "--all", "--watch-root", &root_text],
            &mut Vec::new(),
            &mut Vec::new(),
        ),
        EXIT_CODE_FATAL
    );
    fs::remove_dir_all(root.join("invalid!")).expect("remove invalid directory");
    assert_eq!(
        run(
            [
                "ffhn",
                "run",
                "--watch-root",
                &root_text,
                "--target",
                "valid"
            ],
            &mut Vec::new(),
            &mut Vec::new(),
        ),
        EXIT_CODE_RUN_FAILED
    );
}

#[test]
fn run_failure_classifier_treats_outbox_overflow_as_an_operational_failure() {
    let report: ffhn_core::RunReport = serde_json::from_value(serde_json::json!({
        "schema_name": "ffhn.run_report",
        "schema_version": 17,
        "target_id": "demo",
        "run_mode": "live",
        "outcome": "changed",
        "run_started_at": "2026-07-15T00:00:00Z",
        "run_finished_at": "2026-07-15T00:00:01Z",
        "policy_evaluation": {
            "status": "not_evaluated",
            "event_eligibilities": [],
        },
        "lifecycle": {
            "before": null,
            "after": {
                "source_health": {
                    "state": "healthy",
                    "reason_class": null,
                    "consecutive_unresolved": 0,
                    "first_unresolved_at": null,
                    "last_details": null,
                },
                "permanent_error_episode": null,
                "integration_fault_episode": null,
            },
        },
        "state_persisted": true,
        "delivery_outcomes": [],
        "outbox_overflow": [{
            "event_id": "a".repeat(64),
            "route_id": "run",
            "event_kind": "initialized",
        }],
    }))
    .expect("run report");

    assert!(run_failed(&report));
}

#[cfg(unix)]
#[test]
fn reset_returns_a_failed_exit_when_durable_delivery_is_not_completed() {
    let temporary = tempdir().expect("temporary directory");
    let root = temporary.path();
    write_target(root, "demo", r#"{"value":1}"#);
    let target = root.join("demo/target.toml");
    let base = fs::read_to_string(&target).expect("target");
    fs::write(
            &target,
            format!(
                "{base}\n[[routes]]\nroute_id = \"run\"\nroute_family = \"on_run\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = \"/bin/sh\"\nargs = [\"-c\", \"exit 7\"]\ntimeout_ms = 1000\n"
            ),
        )
        .expect("delivery route");
    let root_text = root.to_string_lossy().to_string();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    assert_eq!(
        run(
            [
                "ffhn",
                "reset",
                "--watch-root",
                &root_text,
                "--target",
                "demo"
            ],
            &mut stdout,
            &mut stderr,
        ),
        EXIT_CODE_RUN_FAILED
    );
    let report: serde_json::Value = serde_json::from_slice(&stdout).expect("reset report");
    assert_eq!(report["delivery_outcomes"][0]["status"], "retry_scheduled");
}

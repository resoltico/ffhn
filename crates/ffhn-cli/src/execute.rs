use std::ffi::OsString;
use std::io::Write;

use clap::error::ErrorKind;
use ffhn_core::{TargetPaths, reset, run_batch, run_once, run_once_dry_run, status};

use crate::args::{Command, parse_cli};
use crate::error::{CLI_OUTPUT_WRITE_ERROR, write_cli_error};
use crate::render::{
    render_batch_report, render_reset_report, render_run_report, render_status_report,
};
use crate::{EXIT_CODE_FATAL, EXIT_CODE_RUN_FAILED, EXIT_CODE_USAGE};

mod discovery;

use discovery::{SelectedTargets, selected_targets};

/// Entry point for the FFHN CLI.
pub fn run<I, T>(args: I, stdout: &mut impl Write, stderr: &mut impl Write) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match parse_cli(args) {
        Ok(cli) => cli,
        Err(error) => {
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                return if write!(stdout, "{error}").is_ok() {
                    0
                } else {
                    write_error(stderr)
                };
            }
            let _ = write_cli_error(stderr, &error.to_string());
            return EXIT_CODE_USAGE;
        }
    };
    match cli.command {
        Command::Run(command) => {
            let selection = match selected_targets(&command) {
                Ok(selection) => selection,
                Err(error) => {
                    let _ = write_cli_error(stderr, &error.to_string());
                    return EXIT_CODE_FATAL;
                }
            };
            let targets = match selection {
                SelectedTargets::Explicit(targets) => targets,
                SelectedTargets::Discovered(entries) => match entries
                    .into_iter()
                    .map(|entry| {
                        entry.validated_id.ok_or_else(|| {
                            entry
                                .validation_message
                                .unwrap_or_else(|| "invalid discovered target id".to_owned())
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
                {
                    Ok(targets) => targets,
                    Err(message) => {
                        let _ = write_cli_error(stderr, &message);
                        return EXIT_CODE_FATAL;
                    }
                },
            };
            if targets.len() == 1 {
                let paths = TargetPaths::try_new(&command.watch_root, targets[0].as_str())
                    .expect("validated target path");
                let result = if command.dry_run {
                    run_once_dry_run(&paths)
                } else {
                    run_once(&paths)
                };
                match result {
                    Ok(report) => {
                        if render_run_report(stdout, &report, command.output_format).is_err() {
                            return write_error(stderr);
                        }
                        if run_failed(&report) {
                            EXIT_CODE_RUN_FAILED
                        } else {
                            0
                        }
                    }
                    Err(error) => {
                        let _ = write_cli_error(stderr, &error.to_string());
                        EXIT_CODE_FATAL
                    }
                }
            } else {
                let mode = if command.dry_run {
                    ffhn_core::RunMode::DryRun
                } else {
                    ffhn_core::RunMode::Live
                };
                match run_batch(&command.watch_root, &targets, mode, command.jobs) {
                    Ok(report) => {
                        if render_batch_report(stdout, &report, command.output_format).is_err() {
                            return write_error(stderr);
                        }
                        if report.reports().iter().any(run_failed) {
                            EXIT_CODE_RUN_FAILED
                        } else {
                            0
                        }
                    }
                    Err(error) => {
                        let _ = write_cli_error(stderr, &error.to_string());
                        EXIT_CODE_FATAL
                    }
                }
            }
        }
        Command::Status(command) => {
            let paths = TargetPaths::try_new(&command.watch_root, command.target.as_str())
                .expect("validated target path");
            match status(&paths) {
                Ok(report) => {
                    if render_status_report(stdout, &report, command.output_format).is_ok() {
                        0
                    } else {
                        write_error(stderr)
                    }
                }
                Err(error) => {
                    let _ = write_cli_error(stderr, &error.to_string());
                    EXIT_CODE_FATAL
                }
            }
        }
        Command::Reset(command) => {
            let paths = TargetPaths::try_new(&command.watch_root, command.target.as_str())
                .expect("validated target path");
            match reset(&paths) {
                Ok(report) => {
                    if render_reset_report(stdout, &report, command.output_format).is_ok() {
                        if report.has_delivery_problem() {
                            EXIT_CODE_RUN_FAILED
                        } else {
                            0
                        }
                    } else {
                        write_error(stderr)
                    }
                }
                Err(error) => {
                    let _ = write_cli_error(stderr, &error.to_string());
                    EXIT_CODE_FATAL
                }
            }
        }
    }
}

fn run_failed(report: &ffhn_core::RunReport) -> bool {
    report.has_delivery_problem()
        || !matches!(
            report.outcome(),
            ffhn_core::RunOutcome::Initialized
                | ffhn_core::RunOutcome::Changed
                | ffhn_core::RunOutcome::Unchanged
                | ffhn_core::RunOutcome::SkippedDisabled
        )
}

fn write_error(stderr: &mut impl Write) -> i32 {
    let _ = write_cli_error(stderr, CLI_OUTPUT_WRITE_ERROR);
    EXIT_CODE_FATAL
}

#[cfg(test)]
mod tests {
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
                "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"{id}\"\ndisplay_name = \"{id}\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
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
        fs::create_dir_all(single_error.path().join(".ffhn-locks/demo.lock"))
            .expect("lock directory");
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
            "schema_version": 7,
            "target_id": "demo",
            "run_mode": "live",
            "outcome": "changed",
            "run_started_at": "2026-07-15T00:00:00Z",
            "run_finished_at": "2026-07-15T00:00:01Z",
            "state_persisted": true,
            "delivery_outcomes": [],
            "outbox_overflow": [{
                "event_id": "a".repeat(64),
                "route_id": "run",
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
}

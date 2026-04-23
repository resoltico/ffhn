use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;

use clap::error::ErrorKind;
use ffhn_core::{
    BATCH_RUN_REPORT_SCHEMA_NAME, CoreError, RUN_REPORT_SCHEMA_NAME, RunMode, RunOutcome,
    STATUS_REPORT_SCHEMA_NAME, TargetPaths, document_write_error, duplicate_target_ids_usage_error,
    run_batch, run_once, run_once_dry_run, status, validate_target,
};

use crate::args::{Command, RunCommand, parse_cli};
use crate::error::write_cli_error;
use crate::help::try_handle_top_level_request;
use crate::render::render_json_document;
use crate::{EXIT_CODE_FATAL, EXIT_CODE_RUN_FAILED, EXIT_CODE_USAGE};

/// Entry point for the FFHN CLI.
pub fn run(
    args: impl IntoIterator<Item = String>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    let raw_args: Vec<String> = args.into_iter().collect();
    match try_handle_top_level_request(&raw_args, stdout) {
        Ok(true) | Err(_) => return 0,
        Ok(false) => {}
    }

    let cli = match parse_cli(raw_args) {
        Ok(cli) => cli,
        Err(error) => {
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                let _ = write!(stdout, "{error}");
                return 0;
            }
            let _ = write_cli_error(stderr, &error.to_string());
            return EXIT_CODE_USAGE;
        }
    };

    match cli.command {
        Command::Run(command) => {
            if let Some(duplicate) = duplicate_target_id(&command) {
                let _ = write_cli_error(stderr, &duplicate_target_ids_usage_error(duplicate));
                return EXIT_CODE_USAGE;
            }

            let targets = match selected_targets(&command) {
                Ok(targets) => targets,
                Err(error) => {
                    let _ = write_cli_error(stderr, &error.to_string());
                    return EXIT_CODE_FATAL;
                }
            };

            if targets.len() == 1 && !command.all {
                let paths = TargetPaths::new(command.watch_root, targets[0].clone());
                let run_result = if command.dry_run {
                    run_once_dry_run(&paths)
                } else {
                    run_once(&paths)
                };
                return match run_result {
                    Ok(report) => {
                        if render_json_document(stdout, &report).is_err() {
                            let _ = write_cli_error(
                                stderr,
                                &document_write_error(RUN_REPORT_SCHEMA_NAME)
                                    .expect("registered run report write error"),
                            );
                            EXIT_CODE_FATAL
                        } else if run_report_requires_failed_exit(&report) {
                            EXIT_CODE_RUN_FAILED
                        } else {
                            0
                        }
                    }
                    Err(error) => {
                        let _ = write_cli_error(stderr, &error.to_string());
                        EXIT_CODE_FATAL
                    }
                };
            }

            match run_batch(
                &command.watch_root,
                &targets,
                if command.dry_run {
                    RunMode::DryRun
                } else {
                    RunMode::Live
                },
                command.jobs,
            ) {
                Ok(report) => {
                    if render_json_document(stdout, &report).is_err() {
                        let _ = write_cli_error(
                            stderr,
                            &document_write_error(BATCH_RUN_REPORT_SCHEMA_NAME)
                                .expect("registered batch run report write error"),
                        );
                        EXIT_CODE_FATAL
                    } else if report.outcome_counts.failed_transient > 0
                        || report.outcome_counts.failed_permanent > 0
                        || report.outcome_counts.persist_error > 0
                        || report.outcome_counts.fatal_error > 0
                        || batch_report_has_notification_failures(&report)
                    {
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
        Command::Status(command) => {
            let paths = TargetPaths::new(command.watch_root, command.target);
            match status(&paths) {
                Ok(report) => {
                    if render_json_document(stdout, &report).is_err() {
                        let _ = write_cli_error(
                            stderr,
                            &document_write_error(STATUS_REPORT_SCHEMA_NAME)
                                .expect("registered status report write error"),
                        );
                        return EXIT_CODE_FATAL;
                    }
                    0
                }
                Err(error) => {
                    let _ = write_cli_error(stderr, &error.to_string());
                    EXIT_CODE_FATAL
                }
            }
        }
    }
}

fn selected_targets(command: &RunCommand) -> Result<Vec<String>, CoreError> {
    if !command.all {
        return Ok(command.targets.clone());
    }

    discover_watch_root_targets(&command.watch_root)
}

fn duplicate_target_id(command: &RunCommand) -> Option<&str> {
    if command.all {
        return None;
    }

    let mut seen = BTreeSet::new();
    command
        .targets
        .iter()
        .find_map(|target_id| (!seen.insert(target_id.as_str())).then_some(target_id.as_str()))
}

pub(crate) fn discover_watch_root_targets(watch_root: &Path) -> Result<Vec<String>, CoreError> {
    let mut targets = Vec::new();
    if !watch_root.exists() {
        return Err(CoreError::io(
            watch_root,
            io::Error::new(io::ErrorKind::NotFound, "watch root does not exist"),
        ));
    }
    if !watch_root.is_dir() {
        return Err(CoreError::io(
            watch_root,
            io::Error::other("watch root is not a directory"),
        ));
    }

    let read_dir = fs::read_dir(watch_root)
        .map_err(|error| CoreError::io(watch_root, error))?
        .map(|entry| entry.map(|entry| entry.path()));
    let mut entries = collect_watch_root_directories(watch_root, read_dir)?;
    entries.sort();

    for target_id in entries.into_iter().filter_map(|path| {
        if !path.join("target.toml").exists() {
            return None;
        }
        path.file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
    }) {
        let target_paths = TargetPaths::new(watch_root, target_id.clone());
        let include_target = match validate_target(&target_paths) {
            Ok(target) => target.enabled,
            Err(_) => true,
        };
        if include_target {
            targets.push(target_id);
        }
    }

    Ok(targets)
}

pub(crate) fn collect_watch_root_directories<I>(
    watch_root: &Path,
    entries: I,
) -> Result<Vec<std::path::PathBuf>, CoreError>
where
    I: IntoIterator<Item = std::io::Result<std::path::PathBuf>>,
{
    let mut directories = Vec::new();
    for entry in entries {
        let path = entry.map_err(|error| CoreError::io(watch_root, error))?;
        let metadata = fs::symlink_metadata(&path).map_err(|error| CoreError::io(&path, error))?;
        if metadata.is_dir() {
            directories.push(path);
        }
    }
    Ok(directories)
}

fn run_report_requires_failed_exit(report: &ffhn_core::RunReport) -> bool {
    matches!(
        report.run_outcome,
        RunOutcome::FailedTransient | RunOutcome::FailedPermanent
    ) || report.run_mode == RunMode::Live
        && (report.persist.error.is_some() || run_report_has_notification_failures(report))
}

fn batch_report_has_notification_failures(report: &ffhn_core::BatchRunReport) -> bool {
    report
        .entries
        .iter()
        .filter_map(|entry| entry.run_report.as_ref())
        .any(run_report_has_notification_failures)
}

fn run_report_has_notification_failures(report: &ffhn_core::RunReport) -> bool {
    report
        .notifications
        .iter()
        .any(|delivery| !delivery.delivered)
}

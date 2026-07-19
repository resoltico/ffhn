use std::ffi::OsString;
use std::io::Write;

use clap::error::ErrorKind;
use ffhn_core::{TargetPaths, reset, run_batch, run_once, run_once_dry_run, status};

use crate::args::Command;
use crate::args::parse::parse_cli;
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
mod tests;

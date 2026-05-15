use std::ffi::OsString;
use std::io::Write;

use clap::error::ErrorKind;
use ffhn_core::{
    TargetPaths, run_batch, run_once, run_once_dry_run, run_report_write_error, status,
    status_report_write_error,
};

use crate::args::{Command, parse_cli};
use crate::error::{CLI_OUTPUT_WRITE_ERROR, write_cli_error};
use crate::render::{render_run_report, render_status_report};
use crate::{EXIT_CODE_FATAL, EXIT_CODE_RUN_FAILED, EXIT_CODE_USAGE};

mod batch;
mod discovery;

use batch::{
    render_batch_result, requested_run_mode, run_discovered_batch, run_report_requires_failed_exit,
};
#[cfg(test)]
pub(crate) use discovery::{
    DiscoveredTarget, collect_watch_root_directories, discover_watch_root_targets,
};
use discovery::{SelectedTargets, selected_targets};

/// Entry point for the FFHN CLI.
pub fn run<I, T>(args: I, stdout: &mut impl Write, stderr: &mut impl Write) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let raw_args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let cli = match parse_cli(raw_args) {
        Ok(cli) => cli,
        Err(error) => {
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                return if write!(stdout, "{error}").is_ok() {
                    0
                } else {
                    report_cli_output_error(stderr)
                };
            }
            let _ = write_cli_error(stderr, &error.to_string());
            return EXIT_CODE_USAGE;
        }
    };

    match cli.command {
        Command::Run(command) => {
            let selection = match selected_targets(&command) {
                Ok(targets) => targets,
                Err(error) => {
                    let _ = write_cli_error(stderr, &error.to_string());
                    return EXIT_CODE_FATAL;
                }
            };

            if let SelectedTargets::Explicit(targets) = &selection
                && targets.len() == 1
            {
                let paths = TargetPaths::try_new(&command.watch_root, targets[0].as_str())
                    .expect("validated target path");
                let run_result = if command.dry_run {
                    run_once_dry_run(&paths)
                } else {
                    run_once(&paths)
                };
                return match run_result {
                    Ok(report) => {
                        if render_run_report(stdout, &report, command.output_format).is_err() {
                            let _ = write_cli_error(stderr, &run_report_write_error());
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

            let batch_result = match selection {
                SelectedTargets::Explicit(targets) => run_batch(
                    &command.watch_root,
                    &targets,
                    requested_run_mode(command.dry_run),
                    command.jobs,
                ),
                SelectedTargets::Discovered(targets) => run_discovered_batch(
                    &command.watch_root,
                    targets,
                    requested_run_mode(command.dry_run),
                    command.jobs,
                ),
            };

            render_batch_result(batch_result, command.output_format, stdout, stderr)
        }
        Command::Status(command) => {
            let paths = TargetPaths::try_new(&command.watch_root, command.target.as_str())
                .expect("validated target path");
            match status(&paths) {
                Ok(report) => {
                    if render_status_report(stdout, &report, command.output_format).is_err() {
                        let _ = write_cli_error(stderr, &status_report_write_error());
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

fn report_cli_output_error(stderr: &mut impl Write) -> i32 {
    let _ = write_cli_error(stderr, CLI_OUTPUT_WRITE_ERROR);
    EXIT_CODE_FATAL
}

#[cfg(test)]
mod tests;

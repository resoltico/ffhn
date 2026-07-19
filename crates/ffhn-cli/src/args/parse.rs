//! Argument-match translation and CLI usage diagnostics.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::PathBuf;

use clap::{ArgMatches, Error as ClapError, error::ErrorKind};
use ffhn_core::{
    CLI_ARGUMENT_ALL_ID, CLI_ARGUMENT_DRY_RUN_ID, CLI_ARGUMENT_FORMAT_ID, CLI_ARGUMENT_JOBS_ID,
    CLI_ARGUMENT_TARGET_ID, CLI_ARGUMENT_WATCH_ROOT_ID, CLI_OPERATION_RESET_ID,
    CLI_OPERATION_RUN_ID, CLI_OPERATION_STATUS_ID, TargetId, duplicate_target_ids_usage_error,
    positive_batch_concurrency_usage_error, run_operation, run_target_selection_usage_error,
    status_operation,
};

use super::definition::build_cli_command;
use super::{Cli, Command, OutputFormat, ResetCommand, RunCommand, StatusCommand};

pub(crate) fn parse_cli<I, T>(args: I) -> Result<Cli, ClapError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let raw_args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    if let Some(error) = detect_misplaced_operation_flag(&raw_args) {
        return Err(error);
    }
    let matches = build_cli_command().try_get_matches_from(raw_args)?;
    matches_to_cli(&matches)
}

pub(crate) fn matches_to_cli(matches: &ArgMatches) -> Result<Cli, ClapError> {
    let (operation_id, submatches) = matches
        .subcommand()
        .expect("subcommand_required keeps one operation present");

    if operation_id == CLI_OPERATION_RUN_ID {
        let targets: Vec<TargetId> = submatches
            .get_many::<TargetId>(CLI_ARGUMENT_TARGET_ID)
            .map(|targets| targets.cloned().collect())
            .unwrap_or_default();
        let all = submatches.get_flag(CLI_ARGUMENT_ALL_ID);
        if targets.is_empty() && !all {
            return Err(operation_usage_error(
                ErrorKind::MissingRequiredArgument,
                run_target_selection_usage_error(),
                run_operation().usage,
            ));
        }
        if let Some(duplicate) = duplicate_target_id(&targets) {
            return Err(operation_usage_error(
                ErrorKind::ArgumentConflict,
                duplicate_target_ids_usage_error(duplicate),
                run_operation().usage,
            ));
        }

        return Ok(Cli {
            command: Command::Run(RunCommand {
                watch_root: submatches
                    .get_one::<PathBuf>(CLI_ARGUMENT_WATCH_ROOT_ID)
                    .expect("run watch-root default")
                    .clone(),
                targets,
                all,
                jobs: parse_jobs(
                    submatches
                        .get_one::<String>(CLI_ARGUMENT_JOBS_ID)
                        .expect("run jobs default"),
                )?,
                dry_run: submatches.get_flag(CLI_ARGUMENT_DRY_RUN_ID),
                output_format: *submatches
                    .get_one::<OutputFormat>(CLI_ARGUMENT_FORMAT_ID)
                    .expect("run output format default"),
            }),
        });
    }

    if operation_id == CLI_OPERATION_STATUS_ID {
        return Ok(Cli {
            command: Command::Status(StatusCommand {
                watch_root: submatches
                    .get_one::<PathBuf>(CLI_ARGUMENT_WATCH_ROOT_ID)
                    .expect("status watch-root default")
                    .clone(),
                target: submatches
                    .get_one::<TargetId>(CLI_ARGUMENT_TARGET_ID)
                    .expect("status target")
                    .clone(),
                output_format: *submatches
                    .get_one::<OutputFormat>(CLI_ARGUMENT_FORMAT_ID)
                    .expect("status output format default"),
            }),
        });
    }

    if operation_id == CLI_OPERATION_RESET_ID {
        return Ok(Cli {
            command: Command::Reset(ResetCommand {
                watch_root: submatches
                    .get_one::<PathBuf>(CLI_ARGUMENT_WATCH_ROOT_ID)
                    .expect("reset watch-root default")
                    .clone(),
                target: submatches
                    .get_one::<TargetId>(CLI_ARGUMENT_TARGET_ID)
                    .expect("reset target")
                    .clone(),
                output_format: *submatches
                    .get_one::<OutputFormat>(CLI_ARGUMENT_FORMAT_ID)
                    .expect("reset output format default"),
            }),
        });
    }

    Err(ClapError::raw(
        ErrorKind::InvalidSubcommand,
        format!("unsupported FFHN operation id: {operation_id}"),
    ))
}

pub(super) fn parse_jobs(raw: &str) -> Result<usize, ClapError> {
    let parsed = raw.parse::<usize>().map_err(|_| {
        operation_usage_error(
            ErrorKind::ValueValidation,
            positive_batch_concurrency_usage_error(),
            run_operation().usage,
        )
    })?;
    if parsed == 0 {
        return Err(operation_usage_error(
            ErrorKind::ValueValidation,
            positive_batch_concurrency_usage_error(),
            run_operation().usage,
        ));
    }
    Ok(parsed)
}

pub(super) fn duplicate_target_id(targets: &[TargetId]) -> Option<&str> {
    let mut seen = BTreeSet::new();
    targets
        .iter()
        .find_map(|target_id| (!seen.insert(target_id.as_str())).then_some(target_id.as_str()))
}

pub(super) fn operation_usage_error(
    kind: ErrorKind,
    message: impl AsRef<str>,
    usage: &str,
) -> ClapError {
    ClapError::raw(
        kind,
        format!(
            "{}\n\nUsage: {}\n\nFor more information, try '--help'.",
            message.as_ref(),
            usage,
        ),
    )
}

pub(super) fn detect_misplaced_operation_flag(raw_args: &[OsString]) -> Option<ClapError> {
    let first = raw_args.get(1)?.to_str()?;
    if first == "--"
        || first == "run"
        || first == "status"
        || first == "reset"
        || first == "--help"
        || first == "-h"
        || first == "--version"
        || first == "-V"
    {
        return None;
    }

    let suggestion = if first == "--jobs" || first == "--all" || first == "--dry-run" {
        format!(
            "missing operation before '{first}'; try '{}' first",
            run_operation().usage
        )
    } else if first == "--watch-root" || first == "--target" {
        format!(
            "missing operation before '{first}'; try '{}' or '{}'",
            run_operation().usage,
            status_operation().usage,
        )
    } else {
        return None;
    };

    Some(ClapError::raw(
        ErrorKind::MissingSubcommand,
        format!("{suggestion}\n\nUsage: ffhn <COMMAND>\n\nFor more information, try '--help'."),
    ))
}

use std::ffi::OsString;
use std::path::PathBuf;

use clap::{
    Arg, ArgAction, ArgMatches, Command as ClapCommand, Error as ClapError, error::ErrorKind,
};
use ffhn_core::{
    CLI_ARGUMENT_ALL_ID, CLI_ARGUMENT_DRY_RUN_ID, CLI_ARGUMENT_JOBS_ID, CLI_ARGUMENT_TARGET_ID,
    CLI_ARGUMENT_WATCH_ROOT_ID, CLI_OPERATION_RUN_ID, CLI_OPERATION_STATUS_ID, CliArgumentContract,
    CliArgumentValueKind, CliOperationContract, cli_operation,
    positive_batch_concurrency_usage_error,
};

use crate::metadata::{FFHN_DESCRIPTION, FFHN_VERSION, TOOL_NAME};

/// Top-level FFHN CLI payload.
#[derive(Debug, PartialEq, Eq)]
pub struct Cli {
    /// Command to execute.
    pub command: Command,
}

/// Supported FFHN CLI commands.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Run one or more configured targets once.
    Run(RunCommand),
    /// Read one target's current machine-readable status.
    Status(StatusCommand),
}

/// Run-command arguments for one or more targets.
#[derive(Debug, PartialEq, Eq)]
pub struct RunCommand {
    /// Watch-root directory containing per-target subdirectories.
    pub watch_root: PathBuf,
    /// One or more target ids under the watch root.
    pub targets: Vec<String>,
    /// Run every target directory discovered under the watch root.
    pub all: bool,
    /// Maximum concurrent target runs.
    pub jobs: usize,
    /// Validate/fetch/extract without mutating persistent state.
    pub dry_run: bool,
}

/// Status-command arguments.
#[derive(Debug, PartialEq, Eq)]
pub struct StatusCommand {
    /// Watch-root directory containing per-target subdirectories.
    pub watch_root: PathBuf,
    /// Target id under the watch root.
    pub target: String,
}

pub(crate) fn build_cli_command() -> ClapCommand {
    ClapCommand::new(TOOL_NAME)
        .version(FFHN_VERSION)
        .about(FFHN_DESCRIPTION)
        .subcommand_required(true)
        .subcommand(build_operation_subcommand(
            cli_operation(CLI_OPERATION_RUN_ID).expect("run operation"),
        ))
        .subcommand(build_operation_subcommand(
            cli_operation(CLI_OPERATION_STATUS_ID).expect("status operation"),
        ))
}

pub(crate) fn parse_cli<I, T>(args: I) -> Result<Cli, ClapError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = build_cli_command().try_get_matches_from(args)?;
    matches_to_cli(&matches)
}

pub(crate) fn matches_to_cli(matches: &ArgMatches) -> Result<Cli, ClapError> {
    let (operation_id, submatches) = matches
        .subcommand()
        .expect("subcommand_required keeps one operation present");

    if operation_id == CLI_OPERATION_RUN_ID {
        return Ok(Cli {
            command: Command::Run(RunCommand {
                watch_root: submatches
                    .get_one::<PathBuf>(CLI_ARGUMENT_WATCH_ROOT_ID)
                    .expect("run watch-root default")
                    .clone(),
                targets: submatches
                    .get_many::<String>(CLI_ARGUMENT_TARGET_ID)
                    .map(|targets| targets.cloned().collect())
                    .unwrap_or_default(),
                all: submatches.get_flag(CLI_ARGUMENT_ALL_ID),
                jobs: parse_jobs(
                    submatches
                        .get_one::<String>(CLI_ARGUMENT_JOBS_ID)
                        .expect("run jobs default"),
                )?,
                dry_run: submatches.get_flag(CLI_ARGUMENT_DRY_RUN_ID),
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
                    .get_one::<String>(CLI_ARGUMENT_TARGET_ID)
                    .expect("status target")
                    .clone(),
            }),
        });
    }

    Err(ClapError::raw(
        ErrorKind::InvalidSubcommand,
        format!("unsupported FFHN operation id: {operation_id}"),
    ))
}

fn build_operation_subcommand(operation: &CliOperationContract) -> ClapCommand {
    operation.arguments.iter().fold(
        ClapCommand::new(operation.id).about(operation.help_summary),
        |command, argument| command.arg(build_argument(argument)),
    )
}

fn build_argument(argument: &CliArgumentContract) -> Arg {
    let mut arg = Arg::new(argument.id)
        .long(argument.long_name)
        .help(argument.help_summary);

    if let Some(value_name) = argument.value_name {
        arg = arg.value_name(value_name);
    }

    if let Some(default_value) = argument.default_value {
        arg = arg.default_value(default_value);
    }

    if argument.required {
        arg = arg.required(true);
    }

    if let Some(other) = argument.required_unless_present {
        arg = arg.required_unless_present(other);
    }

    for conflicting_argument_id in argument.conflicts_with {
        arg = arg.conflicts_with(conflicting_argument_id);
    }

    if argument.repeatable {
        arg = arg.action(ArgAction::Append);
    }

    match argument.value_kind {
        CliArgumentValueKind::Flag => arg.action(ArgAction::SetTrue),
        CliArgumentValueKind::Path => arg.value_parser(clap::value_parser!(PathBuf)),
        CliArgumentValueKind::String => arg.value_parser(clap::value_parser!(String)),
        CliArgumentValueKind::PositiveInteger => arg.value_parser(clap::value_parser!(String)),
    }
}

fn parse_jobs(raw: &str) -> Result<usize, ClapError> {
    let parsed = raw.parse::<usize>().map_err(|_| {
        ClapError::raw(
            ErrorKind::ValueValidation,
            positive_batch_concurrency_usage_error(),
        )
    })?;
    if parsed == 0 {
        return Err(ClapError::raw(
            ErrorKind::ValueValidation,
            positive_batch_concurrency_usage_error(),
        ));
    }
    Ok(parsed)
}

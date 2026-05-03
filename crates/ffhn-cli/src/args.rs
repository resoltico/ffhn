use std::ffi::OsString;
use std::path::PathBuf;

use clap::{
    Arg, ArgAction, ArgMatches, Command as ClapCommand, Error as ClapError, error::ErrorKind,
};
use ffhn_core::{
    CLI_ARGUMENT_ALL_ID, CLI_ARGUMENT_DRY_RUN_ID, CLI_ARGUMENT_JOBS_ID, CLI_ARGUMENT_TARGET_ID,
    CLI_ARGUMENT_WATCH_ROOT_ID, CLI_OPERATION_RUN_ID, CLI_OPERATION_STATUS_ID, CliArgumentContract,
    CliArgumentValueKind, CliOperationContract, TargetId, positive_batch_concurrency_usage_error,
    run_operation, status_operation,
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
    /// Watch root directory containing per-target subdirectories.
    pub watch_root: PathBuf,
    /// One or more target ids under the watch root.
    pub targets: Vec<TargetId>,
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
    /// Watch root directory containing per-target subdirectories.
    pub watch_root: PathBuf,
    /// Target id under the watch root.
    pub target: TargetId,
}

pub(crate) fn build_cli_command() -> ClapCommand {
    ClapCommand::new(TOOL_NAME)
        .version(FFHN_VERSION)
        .about(FFHN_DESCRIPTION)
        .subcommand_required(true)
        .subcommand(build_operation_subcommand(run_operation()))
        .subcommand(build_operation_subcommand(status_operation()))
}

const RUN_AFTER_HELP: &str = "\
Target layout:
  <watch_root>/<target_id>/target.toml

Minimal file target:
  schema_name = \"ffhn.target\"
  schema_version = 1
  target_id = \"demo_file\"
  display_name = \"Demo File\"
  enabled = true

  [target]
  kind = \"file\"
  file_path = \"/absolute/path/to/file.html\"

  [fetch]
  engine = \"file\"

  [selection]
  kind = \"css_selector\"
  match = \"single\"
  output = \"outer_html\"
  whitespace = \"preserve\"
  rewrite_urls = false
  selector = \"main\"

  [compare]
  basis = \"canonical_text_sha256\"
  canonicalization = []

Minimal HTTP target:
  schema_name = \"ffhn.target\"
  schema_version = 1
  target_id = \"demo_http\"
  display_name = \"Demo HTTP\"
  enabled = true

  [target]
  kind = \"http\"
  source_url = \"https://example.com/\"

  [fetch]
  engine = \"http\"
  user_agent = \"ffhn/example\"
  accept = \"text/html,application/xhtml+xml\"

  [selection]
  kind = \"css_selector\"
  match = \"single\"
  output = \"outer_html\"
  whitespace = \"preserve\"
  rewrite_urls = false
  selector = \"main\"

  [compare]
  basis = \"canonical_text_sha256\"
  canonicalization = []

Discovery notes:
  The watch root itself must already exist and be a directory.
  --all scans only immediate subdirectories of the watch root.
  Valid disabled targets are skipped during --all discovery.
  Invalid directory names become fatal batch entries so they do not hide silently.
  Status and --dry-run wait behind any active live run so they inspect one stable target view.
  Explicit --target requests whose target.toml path is missing or unreadable stay fatal process errors.";

const STATUS_AFTER_HELP: &str = "\
Target layout:
  <watch_root>/<target_id>/target.toml

Status writes one ffhn.status_report JSON document to stdout when FFHN can open the named target.toml path.
Malformed or contract-invalid target documents return target_status = invalid with structured error_detail.
Status waits behind any active live run so it can inspect one stable target view.
The watch root itself must already exist and be a directory.
Missing or unreadable target.toml paths remain fatal process errors.";

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
                    .get_many::<TargetId>(CLI_ARGUMENT_TARGET_ID)
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
                    .get_one::<TargetId>(CLI_ARGUMENT_TARGET_ID)
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
    let command = operation.arguments.iter().fold(
        ClapCommand::new(operation.id).about(operation.help_summary),
        |command, argument| command.arg(build_argument(argument)),
    );

    match operation.id {
        CLI_OPERATION_RUN_ID => command.after_long_help(RUN_AFTER_HELP),
        CLI_OPERATION_STATUS_ID => command.after_long_help(STATUS_AFTER_HELP),
        _ => command,
    }
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
        CliArgumentValueKind::String => {
            if argument.id == CLI_ARGUMENT_TARGET_ID {
                arg.value_parser(clap::value_parser!(TargetId))
            } else {
                arg.value_parser(clap::value_parser!(String))
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_target_string_arguments_use_plain_string_parsing() {
        let command = ClapCommand::new("ffhn").arg(build_argument(&CliArgumentContract {
            id: "label",
            long_name: "label",
            display_label: "Label",
            value_name: Some("VALUE"),
            help_summary: "Arbitrary label.",
            value_kind: CliArgumentValueKind::String,
            repeatable: false,
            required: true,
            required_unless_present: None,
            conflicts_with: &[],
            default_value: None,
        }));

        let matches = command
            .try_get_matches_from(["ffhn", "--label", "Demo"])
            .expect("parse custom string argument");
        assert_eq!(
            matches
                .get_one::<String>("label")
                .expect("string value parsed"),
            "Demo"
        );
    }

    #[test]
    fn unknown_operation_subcommands_do_not_attach_run_or_status_appendices() {
        let mut command = build_operation_subcommand(&CliOperationContract {
            id: "inspect",
            display_label: "Inspect",
            help_summary: "Inspect one thing.",
            invocations: &[],
            arguments: &[],
        });

        let mut help = Vec::new();
        command.write_long_help(&mut help).expect("write help");
        let help = String::from_utf8(help).expect("help utf8");

        assert!(help.contains("Inspect one thing."));
        assert!(!help.contains("Target layout:"));
        assert!(!help.contains("Minimal file target:"));
    }
}

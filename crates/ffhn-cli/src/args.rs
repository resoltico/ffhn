use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::PathBuf;

use clap::{
    Arg, ArgAction, ArgMatches, Command as ClapCommand, Error as ClapError, ValueEnum,
    error::ErrorKind,
};
use ffhn_core::{
    CLI_ARGUMENT_ALL_ID, CLI_ARGUMENT_DRY_RUN_ID, CLI_ARGUMENT_FORMAT_ID, CLI_ARGUMENT_JOBS_ID,
    CLI_ARGUMENT_TARGET_ID, CLI_ARGUMENT_WATCH_ROOT_ID, CLI_OPERATION_RUN_ID,
    CLI_OPERATION_STATUS_ID, CliArgumentContract, CliArgumentValueKind, CliOperationContract,
    TargetId, duplicate_target_ids_usage_error, positive_batch_concurrency_usage_error,
    run_operation, run_target_selection_usage_error, status_operation,
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

/// Output presentation mode for successful FFHN documents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Compact machine-oriented JSON on one line.
    Json,
    /// Pretty-printed machine-oriented JSON.
    JsonPretty,
    /// Concise human-oriented summary text.
    Summary,
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
    /// Output format for the emitted document.
    pub output_format: OutputFormat,
}

/// Status-command arguments.
#[derive(Debug, PartialEq, Eq)]
pub struct StatusCommand {
    /// Watch root directory containing per-target subdirectories.
    pub watch_root: PathBuf,
    /// Target id under the watch root.
    pub target: TargetId,
    /// Output format for the emitted document.
    pub output_format: OutputFormat,
}

pub(crate) fn build_cli_command() -> ClapCommand {
    ClapCommand::new(TOOL_NAME)
        .version(FFHN_VERSION)
        .about(FFHN_DESCRIPTION)
        .disable_help_subcommand(true)
        .subcommand_required(true)
        .subcommand(build_operation_subcommand(run_operation()))
        .subcommand(build_operation_subcommand(status_operation()))
}

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
    let long_help = render_operation_long_help(operation);
    let command = command.override_usage(operation.usage);

    if long_help.is_empty() {
        command
    } else {
        command.after_long_help(long_help)
    }
}

fn render_operation_long_help(operation: &CliOperationContract) -> String {
    let mut rendered = String::new();
    append_help_section(&mut rendered, "Examples", operation.examples);
    append_help_section(&mut rendered, "Output", operation.output_notes);
    append_help_section(
        &mut rendered,
        "Operational notes",
        operation.operational_notes,
    );
    rendered
}

fn append_help_section(rendered: &mut String, title: &str, lines: &[&str]) {
    if lines.is_empty() {
        return;
    }
    if !rendered.is_empty() {
        rendered.push('\n');
    }
    let _ = writeln!(rendered, "{title}:");
    for line in lines {
        let _ = writeln!(rendered, "  {line}");
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
            } else if argument.id == CLI_ARGUMENT_FORMAT_ID {
                arg.value_parser(clap::value_parser!(OutputFormat))
            } else {
                arg.value_parser(clap::value_parser!(String))
            }
        }
        CliArgumentValueKind::PositiveInteger => arg.value_parser(clap::value_parser!(String)),
    }
}

fn parse_jobs(raw: &str) -> Result<usize, ClapError> {
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

fn duplicate_target_id(targets: &[TargetId]) -> Option<&str> {
    let mut seen = BTreeSet::new();
    targets
        .iter()
        .find_map(|target_id| (!seen.insert(target_id.as_str())).then_some(target_id.as_str()))
}

fn operation_usage_error(kind: ErrorKind, message: impl AsRef<str>, usage: &str) -> ClapError {
    ClapError::raw(
        kind,
        format!(
            "{}\n\nUsage: {}\n\nFor more information, try '--help'.",
            message.as_ref(),
            usage,
        ),
    )
}

fn detect_misplaced_operation_flag(raw_args: &[OsString]) -> Option<ClapError> {
    let first = raw_args.get(1)?.to_str()?;
    if first == "--"
        || first == "run"
        || first == "status"
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
            usage: "ffhn inspect",
            invocations: &[],
            arguments: &[],
            examples: &[],
            output_notes: &[],
            operational_notes: &[],
        });

        let mut help = Vec::new();
        command.write_long_help(&mut help).expect("write help");
        let help = String::from_utf8(help).expect("help utf8");

        assert!(help.contains("Inspect one thing."));
        assert!(!help.contains("Examples:"));
        assert!(!help.contains("Operational notes:"));
    }

    #[test]
    fn misplaced_operation_flag_detection_is_specific_and_actionable() {
        let no_operation = [OsString::from("ffhn")];
        assert!(detect_misplaced_operation_flag(&no_operation).is_none());

        for allowed_first in [
            "run",
            "status",
            "--help",
            "-h",
            "--version",
            "-V",
            "--",
            "inspect",
        ] {
            let args = [OsString::from("ffhn"), OsString::from(allowed_first)];
            assert!(
                detect_misplaced_operation_flag(&args).is_none(),
                "{allowed_first}"
            );
        }

        let target_hint =
            detect_misplaced_operation_flag(&[OsString::from("ffhn"), OsString::from("--target")])
                .expect("target hint");
        assert!(target_hint.to_string().contains(run_operation().usage));
        assert!(target_hint.to_string().contains(status_operation().usage));

        let jobs_hint =
            detect_misplaced_operation_flag(&[OsString::from("ffhn"), OsString::from("--jobs")])
                .expect("jobs hint");
        assert!(jobs_hint.to_string().contains(run_operation().usage));
        assert!(!jobs_hint.to_string().contains(status_operation().usage));

        let all_hint =
            detect_misplaced_operation_flag(&[OsString::from("ffhn"), OsString::from("--all")])
                .expect("all hint");
        assert!(all_hint.to_string().contains(run_operation().usage));

        let dry_run_hint =
            detect_misplaced_operation_flag(&[OsString::from("ffhn"), OsString::from("--dry-run")])
                .expect("dry-run hint");
        assert!(dry_run_hint.to_string().contains(run_operation().usage));
        assert!(!dry_run_hint.to_string().contains(status_operation().usage));

        let watch_root_hint = detect_misplaced_operation_flag(&[
            OsString::from("ffhn"),
            OsString::from("--watch-root"),
        ])
        .expect("watch-root hint");
        assert!(watch_root_hint.to_string().contains(run_operation().usage));
        assert!(
            watch_root_hint
                .to_string()
                .contains(status_operation().usage)
        );
    }

    #[cfg(unix)]
    #[test]
    fn misplaced_operation_flag_detection_ignores_non_utf8_operands() {
        use std::os::unix::ffi::OsStringExt;

        let args = [OsString::from("ffhn"), OsString::from_vec(vec![0xff, 0xfe])];
        assert!(detect_misplaced_operation_flag(&args).is_none());
    }
}

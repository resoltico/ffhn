//! Declarative Clap command construction from FFHN's operation contract.

use std::fmt::Write as _;
use std::path::PathBuf;

use clap::{Arg, ArgAction, Command as ClapCommand};
use ffhn_core::{
    CLI_ARGUMENT_FORMAT_ID, CLI_ARGUMENT_TARGET_ID, CliArgumentContract, CliArgumentValueKind,
    CliOperationContract, TargetId, reset_operation, run_operation, status_operation,
};

use crate::metadata::{FFHN_DESCRIPTION, FFHN_VERSION, TOOL_NAME};

use super::OutputFormat;

pub(crate) fn build_cli_command() -> ClapCommand {
    ClapCommand::new(TOOL_NAME)
        .version(FFHN_VERSION)
        .about(FFHN_DESCRIPTION)
        .disable_help_subcommand(true)
        .subcommand_required(true)
        .subcommand(build_operation_subcommand(run_operation()))
        .subcommand(build_operation_subcommand(status_operation()))
        .subcommand(build_operation_subcommand(reset_operation()))
}

pub(super) fn build_operation_subcommand(operation: &CliOperationContract) -> ClapCommand {
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

pub(super) fn render_operation_long_help(operation: &CliOperationContract) -> String {
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

pub(super) fn append_help_section(rendered: &mut String, title: &str, lines: &[&str]) {
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

pub(super) fn build_argument(argument: &CliArgumentContract) -> Arg {
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

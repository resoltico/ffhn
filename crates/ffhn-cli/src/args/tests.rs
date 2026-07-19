//! Focused parser and help-contract scenarios.

use std::ffi::OsString;

use clap::{Command as ClapCommand, error::ErrorKind};
use ffhn_core::{
    CliArgumentContract, CliArgumentValueKind, CliOperationContract, TargetId, run_operation,
    status_operation,
};

use super::definition::{
    append_help_section, build_argument, build_cli_command, build_operation_subcommand,
};
use super::parse::{
    detect_misplaced_operation_flag, duplicate_target_id, matches_to_cli, operation_usage_error,
    parse_cli, parse_jobs,
};
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
        "reset",
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

    let watch_root_hint =
        detect_misplaced_operation_flag(&[OsString::from("ffhn"), OsString::from("--watch-root")])
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

#[test]
fn parses_current_run_status_and_reset_commands_and_rejects_invalid_run_selection() {
    let run = parse_cli([
        "ffhn",
        "run",
        "--watch-root",
        "/tmp/watch",
        "--target",
        "one",
        "--target",
        "two",
        "--jobs",
        "2",
        "--dry-run",
        "--format",
        "json-pretty",
    ])
    .expect("run command");
    assert!(matches!(&run.command, Command::Run(_)));
    assert_eq!(
        output_format_for_command(&run.command),
        OutputFormat::JsonPretty
    );
    assert!(parse_cli(["ffhn", "--target", "demo"]).is_err());
    for (operation, expected_format) in [
        ("status", OutputFormat::Summary),
        ("reset", OutputFormat::Json),
    ] {
        let parsed = parse_cli([
            "ffhn",
            operation,
            "--watch-root",
            "/tmp/watch",
            "--target",
            "demo",
            "--format",
            if operation == "status" {
                "summary"
            } else {
                "json"
            },
        ])
        .expect("single target command");
        assert_eq!(output_format_for_command(&parsed.command), expected_format);
    }
    assert!(parse_cli(["ffhn", "run", "--watch-root", "/tmp/watch"]).is_err());
    assert!(parse_cli(["ffhn", "run", "--target", "demo", "--target", "demo"]).is_err());
    assert!(parse_cli(["ffhn", "run", "--target", "demo", "--jobs", "0"]).is_err());
    assert!(parse_cli(["ffhn", "run", "--target", "demo", "--jobs", "many"]).is_err());
    assert!(parse_jobs("3").is_ok());
    assert_eq!(
        duplicate_target_id(&[
            TargetId::new("a").expect("id"),
            TargetId::new("a").expect("id")
        ]),
        Some("a")
    );
}

fn output_format_for_command(command: &Command) -> OutputFormat {
    match command {
        Command::Run(command) => command.output_format,
        Command::Status(command) => command.output_format,
        Command::Reset(command) => command.output_format,
    }
}

#[test]
fn help_building_and_unknown_matches_cover_the_catalog_driven_adapter() {
    let mut help = Vec::new();
    build_cli_command()
        .write_long_help(&mut help)
        .expect("help");
    let help = String::from_utf8(help).expect("UTF-8");
    assert!(help.contains("run"));
    assert!(help.contains("reset"));
    let matches = ClapCommand::new("ffhn")
        .subcommand(ClapCommand::new("other"))
        .try_get_matches_from(["ffhn", "other"])
        .expect("matches");
    assert_eq!(
        matches_to_cli(&matches)
            .expect_err("unknown operation")
            .kind(),
        ErrorKind::InvalidSubcommand
    );
    let mut rendered = String::new();
    append_help_section(&mut rendered, "Empty", &[]);
    assert!(rendered.is_empty());
    append_help_section(&mut rendered, "One", &["line"]);
    append_help_section(&mut rendered, "Two", &["line"]);
    assert!(rendered.contains("One:"));
    assert!(
        operation_usage_error(ErrorKind::ValueValidation, "message", "ffhn run")
            .to_string()
            .contains("message")
    );
}

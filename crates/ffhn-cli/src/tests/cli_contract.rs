use super::*;
use ffhn_core::TargetId;

#[test]
fn target_command_defaults_watch_root() {
    let cli = parse_cli(["ffhn", "run", "--target", "demo"]).expect("parse cli");

    assert_eq!(
        cli.command,
        Command::Run(RunCommand {
            watch_root: "watchlist".into(),
            targets: vec![TargetId::new("demo").expect("target id")],
            all: false,
            jobs: 1,
            dry_run: false,
            output_format: OutputFormat::Json,
        })
    );
}

#[test]
fn parse_cli_rejects_non_numeric_jobs_and_unknown_internal_operation_ids() {
    let error = parse_cli(["ffhn", "run", "--target", "demo", "--jobs", "bogus"])
        .expect_err("non-numeric jobs");
    assert!(
        error
            .to_string()
            .contains(positive_batch_concurrency_usage_error())
    );

    let bogus_matches = clap::Command::new("ffhn")
        .subcommand(clap::Command::new("bogus"))
        .try_get_matches_from(["ffhn", "bogus"])
        .expect("bogus matches");
    let error = matches_to_cli(&bogus_matches).expect_err("unknown operation");
    assert!(
        error
            .to_string()
            .contains("unsupported FFHN operation id: bogus")
    );
}

#[test]
fn parse_cli_rejects_run_requests_without_target_selection() {
    let error = parse_cli(["ffhn", "run"]).expect_err("run without target selection");
    let rendered = error.to_string();

    assert!(rendered.contains(run_target_selection_usage_error()));
    assert!(rendered.contains(
        "Usage: ffhn run (--target <ID>... | --all) [--watch-root <PATH>] [--jobs <N>] [--dry-run] [--format <FORMAT>]"
    ));
    assert!(rendered.contains("For more information, try '--help'."));
}

#[test]
fn parse_cli_accepts_explicit_output_formats() {
    let cli = parse_cli(["ffhn", "status", "--target", "demo", "--format", "summary"])
        .expect("parse status summary");
    assert_eq!(
        cli.command,
        Command::Status(StatusCommand {
            watch_root: "watchlist".into(),
            target: TargetId::new("demo").expect("target id"),
            output_format: OutputFormat::Summary,
        })
    );

    let cli = parse_cli(["ffhn", "run", "--target", "demo", "--format", "json-pretty"])
        .expect("parse run pretty json");
    assert_eq!(
        cli.command,
        Command::Run(RunCommand {
            watch_root: "watchlist".into(),
            targets: vec![TargetId::new("demo").expect("target id")],
            all: false,
            jobs: 1,
            dry_run: false,
            output_format: OutputFormat::JsonPretty,
        })
    );
}

#[test]
fn cli_help_and_parser_render_the_core_contract() {
    let mut root_help = Vec::new();
    build_cli_command()
        .write_long_help(&mut root_help)
        .expect("write help");
    let root_help = String::from_utf8(root_help).expect("help utf8");

    for operation in cli_contract().operations {
        assert!(root_help.contains(operation.id));
        assert!(root_help.contains(operation.help_summary));
    }

    let run = cli_operation(CLI_OPERATION_RUN_ID).expect("run operation");
    let (exit_code, run_help, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        CLI_OPERATION_RUN_ID.to_owned(),
        "--help".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    assert!(stderr.is_empty());
    for argument in run.arguments {
        assert!(run_help.contains(&format!("--{}", argument.long_name)));
        assert!(run_help.contains(argument.help_summary));
    }
    assert!(run_help.contains(run.usage));
    for example in run.examples {
        assert!(run_help.contains(example));
    }
    for note in run.output_notes {
        assert!(run_help.contains(note));
    }
    for note in run.operational_notes {
        assert!(run_help.contains(note));
    }
    assert!(!run_help.contains("Target layout:"));
    assert!(!run_help.contains("schema_name = \"ffhn.target\""));

    let status = cli_operation(CLI_OPERATION_STATUS_ID).expect("status operation");
    assert_eq!(status.display_label, "Status");
    assert_eq!(
        status.usage,
        "ffhn status --target <ID> [--watch-root <PATH>] [--format <FORMAT>]"
    );
}

#[test]
fn metadata_matches_workspace_package_fields() {
    let workspace_manifest = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate dir")
            .parent()
            .expect("workspace root")
            .join("Cargo.toml"),
    )
    .expect("workspace manifest");
    let workspace_version =
        workspace_package_field(&workspace_manifest, "version").expect("workspace version");
    let workspace_description =
        workspace_package_field(&workspace_manifest, "description").expect("workspace description");

    assert_eq!(FFHN_VERSION, workspace_version);
    assert_eq!(FFHN_DESCRIPTION, workspace_description);
}

#[test]
fn run_covers_root_help_help_version_and_parse_error_modes() {
    let mut root_help = Vec::new();
    build_cli_command()
        .write_long_help(&mut root_help)
        .expect("write root help");
    let root_help = String::from_utf8(root_help).expect("root help utf8");
    assert!(root_help.contains("Usage: ffhn <COMMAND>"));
    assert_eq!(root_help.matches(FFHN_DESCRIPTION).count(), 1);

    let (exit_code, stdout, stderr) = run_vec(vec!["ffhn".to_owned()]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains("Usage: ffhn <COMMAND>"));

    let (exit_code, stdout, stderr) = run_vec(vec!["ffhn".to_owned(), "--help".to_owned()]);
    assert_eq!(exit_code, 0);
    assert!(stdout.contains("Usage: ffhn <COMMAND>"));
    assert!(stdout.contains("Run one or more configured targets once."));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run_vec(vec!["ffhn".to_owned(), "--version".to_owned()]);
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, format!("ffhn {FFHN_VERSION}\n"));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "--version".to_owned(),
        "--help".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, format!("ffhn {FFHN_VERSION}\n"));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "--help".to_owned(),
        "--version".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    assert!(stdout.contains("Usage: ffhn <COMMAND>"));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run_vec(vec!["ffhn".to_owned(), "help".to_owned()]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains("unrecognized subcommand 'help'"));

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "help".to_owned(),
        "--version".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains("unrecognized subcommand 'help'"));

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "status".to_owned(),
        "--version".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains("unexpected argument '--version'"));

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--help".to_owned(),
        "--version".to_owned(),
    ]);
    assert_eq!(exit_code, 0);
    assert!(stdout.contains("Usage: ffhn run"));
    assert!(stdout.contains("--dry-run"));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--target".to_owned(),
        "demo".to_owned(),
        "--version".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains("unexpected argument '--version'"));

    let (exit_code, stdout, stderr) = run_vec(vec!["ffhn".to_owned(), "bogus".to_owned()]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains("unrecognized subcommand 'bogus'"));

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "bogus".to_owned(),
        "--version".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains("unrecognized subcommand 'bogus'"));

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--target".to_owned(),
        "demo".to_owned(),
        "--jobs".to_owned(),
        "0".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains(positive_batch_concurrency_usage_error()));

    let (exit_code, stdout, stderr) = run_vec(vec!["ffhn".to_owned(), "run".to_owned()]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains(run_target_selection_usage_error()));
    assert!(stderr.contains(
        "Usage: ffhn run (--target <ID>... | --all) [--watch-root <PATH>] [--jobs <N>] [--dry-run] [--format <FORMAT>]"
    ));

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--target".to_owned(),
        "demo".to_owned(),
        "--target".to_owned(),
        "demo".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains(&duplicate_target_ids_usage_error("demo")));

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "--target".to_owned(),
        "demo".to_owned(),
        "status".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains("missing operation before '--target'"));
    assert!(stderr.contains(
        "ffhn run (--target <ID>... | --all) [--watch-root <PATH>] [--jobs <N>] [--dry-run] [--format <FORMAT>]"
    ));
    assert!(stderr.contains("ffhn status --target <ID> [--watch-root <PATH>] [--format <FORMAT>]"));

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "--jobs".to_owned(),
        "4".to_owned(),
        "run".to_owned(),
        "--all".to_owned(),
    ]);
    assert_eq!(exit_code, EXIT_CODE_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.contains("missing operation before '--jobs'"));
    assert!(stderr.contains(
        "ffhn run (--target <ID>... | --all) [--watch-root <PATH>] [--jobs <N>] [--dry-run] [--format <FORMAT>]"
    ));
}

#[test]
fn run_treats_help_and_version_output_write_failures_as_fatal() {
    for args in [
        vec!["ffhn".to_owned(), "--version".to_owned()],
        vec!["ffhn".to_owned(), "run".to_owned(), "--help".to_owned()],
        vec!["ffhn".to_owned(), "--help".to_owned()],
    ] {
        let mut broken_stdout = BrokenWriter;
        let mut stderr = Vec::new();
        let exit_code = run(args, &mut broken_stdout, &mut stderr);

        assert_eq!(exit_code, EXIT_CODE_FATAL);
        assert!(
            String::from_utf8(stderr)
                .expect("stderr utf8")
                .contains(CLI_OUTPUT_WRITE_ERROR)
        );
    }
}

#[cfg(unix)]
#[test]
fn run_accepts_non_utf8_watch_root_arguments_without_panicking() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let (exit_code, stdout, stderr) = run_vec(vec![
        OsString::from("ffhn"),
        OsString::from("status"),
        OsString::from("--watch-root"),
        OsString::from_vec(vec![
            b'/', b't', b'm', b'p', b'/', b'f', b'f', b'h', b'n', b'-', 0xff,
        ]),
        OsString::from("--target"),
        OsString::from("demo"),
    ]);
    assert_eq!(exit_code, EXIT_CODE_FATAL);
    assert!(stdout.is_empty());
    assert!(stderr.contains("watch root does not exist"));
}

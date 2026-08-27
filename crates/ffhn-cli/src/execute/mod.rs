//! v11 CLI orchestration over the observation-graph core boundary.

use std::{ffi::OsString, io::Write, path::Path};

use clap::error::ErrorKind;
use ffhn_core::{CoreError, graph};
use serde::Serialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    EXIT_CODE_BUSY, EXIT_CODE_FATAL, EXIT_CODE_RUN_FAILED, EXIT_CODE_USAGE,
    args::{AgentAction, Cli, Command, ListCommand, NewKind, OutputFormat, parse_cli},
    error::{CLI_OUTPUT_WRITE_ERROR, write_cli_error},
    render::render_document,
};

mod agent;

/// Entry point for the FFHN v11 graph CLI.
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
    execute(cli, stdout, stderr)
}

fn execute(cli: Cli, stdout: &mut impl Write, stderr: &mut impl Write) -> i32 {
    match cli.command {
        Command::Measure(command) => {
            let graph = match open_graph(&command.graph_root) {
                Ok(graph) => graph,
                Err(error) => return report_error(stderr, error),
            };
            let result = match (command.dry_run, command.measurements.is_empty()) {
                (false, true) => graph::measure_source_once(&graph, command.source),
                (false, false) => graph::measure_selected_source_once(
                    &graph,
                    command.source,
                    command.measurements,
                ),
                (true, true) => graph::measure_source_dry_run(&graph, command.source),
                (true, false) => graph::measure_selected_source_dry_run(
                    &graph,
                    command.source,
                    command.measurements,
                ),
            };
            match result {
                Ok(result) => {
                    let busy = result.status() == graph::GraphSourceStatus::Locked;
                    let failed = result.has_handled_failure();
                    let exit = render_or_fatal(
                        stdout,
                        stderr,
                        &graph::GraphMeasureReport::from(&result),
                        command.output_format,
                        failed,
                    );
                    if exit == 0 && busy {
                        EXIT_CODE_BUSY
                    } else {
                        exit
                    }
                }
                Err(error) => report_error(stderr, error),
            }
        }
        Command::Status(command) => {
            let graph = match open_graph(&command.graph_root) {
                Ok(graph) => graph,
                Err(error) => return report_error(stderr, error),
            };
            let measurement_selected = command.measurement.is_some();
            match graph::status_source(&graph, command.source, command.measurement) {
                Ok(result) => {
                    let failed = status_failed(&result);
                    if measurement_selected {
                        render_measurement_status(
                            graph::GraphMeasurementStatusReport::try_from(&result),
                            command.output_format,
                            failed,
                            stdout,
                            stderr,
                        )
                    } else {
                        render_or_fatal(
                            stdout,
                            stderr,
                            &graph::GraphSourceStatusReport::from(&result),
                            command.output_format,
                            failed,
                        )
                    }
                }
                Err(error) => report_error(stderr, error),
            }
        }
        Command::Reset(command) => {
            let graph = match open_graph(&command.graph_root) {
                Ok(graph) => graph,
                Err(error) => return report_error(stderr, error),
            };
            let result = match command.measurement {
                Some(measurement) => graph::reset_measurement(&graph, command.source, measurement),
                None => graph::reset_source(&graph, command.source),
            };
            match result {
                Ok(result) => render_or_fatal(
                    stdout,
                    stderr,
                    &graph::GraphResetReport::from(&result),
                    command.output_format,
                    false,
                ),
                Err(error) => report_error(stderr, error),
            }
        }
        Command::Validate(command) => {
            let graph = match open_graph(&command.graph_root) {
                Ok(graph) => graph,
                Err(error) => return report_error(stderr, error),
            };
            match graph::validate_graph(&graph, command.source) {
                Ok(result) => {
                    let valid = result.is_valid();
                    render_or_fatal(
                        stdout,
                        stderr,
                        &graph::GraphValidateReport::from(&result),
                        command.output_format,
                        !valid,
                    )
                }
                Err(error) => report_error(stderr, error),
            }
        }
        Command::List(command) => execute_list(command, stdout, stderr),
        Command::New(command) => execute_new(command.kind, stdout, stderr),
        Command::Agent(command) => match command.action {
            AgentAction::Tick(options) => agent::tick(options, stdout, stderr),
            AgentAction::Status(options) => agent::status(options, stdout, stderr),
            AgentAction::Run(options) => agent::run(options, stdout, stderr),
        },
    }
}

fn execute_list(command: ListCommand, stdout: &mut impl Write, stderr: &mut impl Write) -> i32 {
    let graph = match open_graph(&command.graph_root) {
        Ok(graph) => graph,
        Err(error) => return report_error(stderr, error),
    };
    let scope = if command.sources {
        graph::GraphListScope::Sources
    } else {
        graph::GraphListScope::Measurements
    };
    match graph::list_graph(&graph, scope) {
        Ok(result) => render_or_fatal(
            stdout,
            stderr,
            &graph::GraphListReport::from(&result),
            command.output_format,
            false,
        ),
        Err(error) => report_error(stderr, error),
    }
}

fn execute_new(kind: NewKind, stdout: &mut impl Write, stderr: &mut impl Write) -> i32 {
    let (format, result) = match kind {
        NewKind::Source(command) => {
            let result = (|| {
                let graph = initialize_or_open_graph(&command.graph_root)?;
                let document = source_template(&command.source, &command.graph_root)?;
                graph.create_source_document(&document)?;
                Ok(graph::GraphNewReport::source(&command.source))
            })();
            (command.output_format, result)
        }
        NewKind::Measurement(command) => {
            let result = (|| {
                let graph = open_graph(&command.graph_root)?;
                let source = graph.open_source(command.source.clone())?;
                let document = measurement_template(&command.measurement)?;
                source.create_measurement_document(&document)?;
                Ok(graph::GraphNewReport::measurement(
                    &command.source,
                    &command.measurement,
                ))
            })();
            (command.output_format, result)
        }
    };
    match result {
        Ok(report) => render_or_fatal(stdout, stderr, &report, format, false),
        Err(error) => report_error(stderr, error),
    }
}

fn render_measurement_status(
    report: Result<graph::GraphMeasurementStatusReport, CoreError>,
    format: OutputFormat,
    failed: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    match report {
        Ok(report) => render_or_fatal(stdout, stderr, &report, format, failed),
        Err(error) => report_error(stderr, error),
    }
}

pub(super) fn open_graph(root: &Path) -> Result<graph::TrustedGraphRoot, CoreError> {
    graph::TrustedGraphRoot::open(graph::GraphPaths::new(root))
}

fn initialize_or_open_graph(root: &Path) -> Result<graph::TrustedGraphRoot, CoreError> {
    if !root.exists() || graph_root_is_empty(root)? {
        graph::TrustedGraphRoot::initialize(graph::GraphPaths::new(root), utc_now())
    } else {
        open_graph(root)
    }
}

fn graph_root_is_empty(root: &Path) -> Result<bool, CoreError> {
    let mut entries = std::fs::read_dir(root).map_err(|error| CoreError::io(root, error))?;
    Ok(entries.next().is_none())
}

fn status_failed(result: &graph::GraphStatusResult) -> bool {
    matches!(
        result.source().kind(),
        graph::GraphSourceStatusKind::Pending
            | graph::GraphSourceStatusKind::LineageRefused
            | graph::GraphSourceStatusKind::ConfigInvalid
    ) || result.measurements().iter().any(|measurement| {
        matches!(
            measurement.kind(),
            graph::GraphMeasurementStatusKind::LineageHeld
                | graph::GraphMeasurementStatusKind::ConfigInvalid
                | graph::GraphMeasurementStatusKind::Quarantined
        )
    })
}

fn source_template(
    source_id: &graph::SourceId,
    graph_root: &Path,
) -> Result<graph::SourceDocument, CoreError> {
    let graph_root = if graph_root.is_absolute() {
        graph_root.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| CoreError::io(".", error))?
            .join(graph_root)
    };
    let placeholder = graph_root.join("replace-me");
    toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"{}\"\ndisplay_name = \"{}\"\nenabled = false\nescalate_after = 3\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 2000000\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 300000\nmin_interval_ms = 60000\n",
        source_id.as_str(),
        source_id.as_str(),
        placeholder.to_string_lossy(),
    ))
    .map_err(CoreError::from)
}

fn measurement_template(
    measurement_id: &graph::MeasurementId,
) -> Result<graph::MeasurementDocument, CoreError> {
    toml::from_str(&format!(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"{}\"\ndisplay_name = \"{}\"\nenabled = false\nescalate_after = 3\ndeclared_type = \"text\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = \"/replace-me\"\n",
        measurement_id.as_str(),
        measurement_id.as_str(),
    ))
    .map_err(CoreError::from)
}

pub(super) fn utc_now() -> String {
    utc(OffsetDateTime::now_utc())
}

pub(super) fn utc(time: OffsetDateTime) -> String {
    time.format(&Rfc3339)
        .expect("RFC3339 formatting is supported by the static format description")
}

pub(super) fn render_or_fatal(
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    document: &impl Serialize,
    format: OutputFormat,
    failed: bool,
) -> i32 {
    if render_document(stdout, document, format).is_err() {
        return write_error(stderr);
    }
    if failed { EXIT_CODE_RUN_FAILED } else { 0 }
}

pub(super) fn report_error(stderr: &mut impl Write, error: CoreError) -> i32 {
    let busy = matches!(&error, CoreError::Contract(message) if matches!(message.as_str(), "source is busy" | "agent is already running"));
    let _ = write_cli_error(stderr, &error.to_string());
    if busy {
        EXIT_CODE_BUSY
    } else {
        EXIT_CODE_FATAL
    }
}

pub(super) fn write_error(stderr: &mut impl Write) -> i32 {
    let _ = write_cli_error(stderr, CLI_OUTPUT_WRITE_ERROR);
    EXIT_CODE_FATAL
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

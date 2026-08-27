//! The observation-graph command line contract.

use std::{ffi::OsString, path::PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
use ffhn_core::graph::{MeasurementId, SourceId};

/// Parsed FFHN invocation.
#[derive(Debug, Parser)]
#[command(name = "ffhn", version, about = "Focused Fragment History Notifier")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// Top-level graph operation.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Run the singleton graph agent.
    Agent(AgentCommand),
    /// Acquire one source and evaluate configured measurements.
    Measure(MeasureCommand),
    /// Read one source's stable graph status without mutation.
    Status(StatusCommand),
    /// Mint fresh source or measurement lineage and remove only its owned state.
    Reset(ResetCommand),
    /// Validate graph configuration without fetching, delivery, or mutation.
    Validate(ValidateCommand),
    /// List configured sources or measurements.
    List(ListCommand),
    /// Create one editable source or measurement configuration template.
    New(NewCommand),
}

/// Agent subcommand family.
#[derive(Debug, Args)]
pub(crate) struct AgentCommand {
    #[command(subcommand)]
    pub(crate) action: AgentAction,
}

/// Finite and continuous agent actions.
#[derive(Debug, Subcommand)]
pub(crate) enum AgentAction {
    /// Continuously schedule due source acquisition and delivery drains.
    Run(AgentOptions),
    /// Execute exactly one finite graph-agent tick.
    Tick(AgentOptions),
    /// Read a status snapshot for every configured source.
    Status(AgentOptions),
}

/// Shared graph-agent options.
#[derive(Debug, Args)]
pub(crate) struct AgentOptions {
    /// Graph-root directory. The default is the current directory.
    #[arg(long, default_value = ".")]
    pub(crate) graph_root: PathBuf,
    /// Maximum number of source turns that may execute concurrently.
    #[arg(long, default_value = "1", value_parser = parse_positive_usize)]
    pub(crate) jobs: usize,
    /// Output rendering format.
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::JsonPretty)]
    pub(crate) output_format: OutputFormat,
}

/// One measurement invocation.
#[derive(Debug, Args)]
pub(crate) struct MeasureCommand {
    /// Source identifier to acquire exactly once.
    #[arg(long)]
    pub(crate) source: SourceId,
    /// Restrict this acquisition to one or more configured measurement identifiers.
    #[arg(long = "measurement")]
    pub(crate) measurements: Vec<MeasurementId>,
    /// Graph-root directory. The default is the current directory.
    #[arg(long, default_value = ".")]
    pub(crate) graph_root: PathBuf,
    /// Reserved source-level parallelism bound; one explicit source is always serialized.
    #[arg(long, default_value = "1", value_parser = parse_positive_usize)]
    pub(crate) jobs: usize,
    /// Fetch and evaluate without writing lineage, state, outboxes, or delivery attempts.
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// Output rendering format.
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::JsonPretty)]
    pub(crate) output_format: OutputFormat,
}

/// One read-only source status invocation.
#[derive(Debug, Args)]
pub(crate) struct StatusCommand {
    /// Source identifier to inspect.
    #[arg(long)]
    pub(crate) source: SourceId,
    /// Optionally restrict output to one configured, authoritative, or artifact-backed measurement.
    #[arg(long)]
    pub(crate) measurement: Option<MeasurementId>,
    /// Graph-root directory. The default is the current directory.
    #[arg(long, default_value = ".")]
    pub(crate) graph_root: PathBuf,
    /// Output rendering format.
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::JsonPretty)]
    pub(crate) output_format: OutputFormat,
}

/// Mint-only reset invocation.
#[derive(Debug, Args)]
pub(crate) struct ResetCommand {
    /// Source identifier whose lineage is reset.
    #[arg(long)]
    pub(crate) source: SourceId,
    /// Reset only this measurement while preserving sibling source lineage.
    #[arg(long)]
    pub(crate) measurement: Option<MeasurementId>,
    /// Graph-root directory. The default is the current directory.
    #[arg(long, default_value = ".")]
    pub(crate) graph_root: PathBuf,
    /// Output rendering format.
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::JsonPretty)]
    pub(crate) output_format: OutputFormat,
}

/// Offline validation invocation.
#[derive(Debug, Args)]
pub(crate) struct ValidateCommand {
    /// Restrict validation to one source.
    #[arg(long, conflicts_with = "all")]
    pub(crate) source: Option<SourceId>,
    /// Validate every configured source.
    #[arg(long, required_unless_present = "source")]
    pub(crate) all: bool,
    /// Graph-root directory. The default is the current directory.
    #[arg(long, default_value = ".")]
    pub(crate) graph_root: PathBuf,
    /// Output rendering format.
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::JsonPretty)]
    pub(crate) output_format: OutputFormat,
}

/// Configuration listing invocation.
#[derive(Debug, Args)]
pub(crate) struct ListCommand {
    /// List source identifiers.
    #[arg(
        long,
        conflicts_with = "measurements",
        required_unless_present = "measurements"
    )]
    pub(crate) sources: bool,
    /// List every configured measurement paired with its source identifier.
    #[arg(long, conflicts_with = "sources", required_unless_present = "sources")]
    pub(crate) measurements: bool,
    /// Graph-root directory. The default is the current directory.
    #[arg(long, default_value = ".")]
    pub(crate) graph_root: PathBuf,
    /// Output rendering format.
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::JsonPretty)]
    pub(crate) output_format: OutputFormat,
}

/// Configuration template creation invocation.
#[derive(Debug, Args)]
pub(crate) struct NewCommand {
    #[command(subcommand)]
    pub(crate) kind: NewKind,
}

/// Source and measurement template kinds.
#[derive(Debug, Subcommand)]
pub(crate) enum NewKind {
    /// Create a disabled file-backed source template.
    Source(NewSourceCommand),
    /// Create a disabled text JSON-pointer measurement template.
    Measurement(NewMeasurementCommand),
}

/// New source template arguments.
#[derive(Debug, Args)]
pub(crate) struct NewSourceCommand {
    /// New source identifier.
    #[arg(long)]
    pub(crate) source: SourceId,
    /// Graph-root directory. The default is the current directory.
    #[arg(long, default_value = ".")]
    pub(crate) graph_root: PathBuf,
    /// Output rendering format.
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::JsonPretty)]
    pub(crate) output_format: OutputFormat,
}

/// New measurement template arguments.
#[derive(Debug, Args)]
pub(crate) struct NewMeasurementCommand {
    /// Existing source identifier that will own this measurement configuration.
    #[arg(long)]
    pub(crate) source: SourceId,
    /// New measurement identifier.
    #[arg(long)]
    pub(crate) measurement: MeasurementId,
    /// Graph-root directory. The default is the current directory.
    #[arg(long, default_value = ".")]
    pub(crate) graph_root: PathBuf,
    /// Output rendering format.
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::JsonPretty)]
    pub(crate) output_format: OutputFormat,
}

/// Output presentation mode for public documents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum OutputFormat {
    /// Compact machine-oriented JSON on one line.
    Json,
    /// Pretty-printed machine-oriented JSON.
    JsonPretty,
    /// Concise human-oriented text derived from the document.
    Summary,
}

/// Parses one complete CLI invocation into the closed command vocabulary.
pub(crate) fn parse_cli<I, T>(args: I) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    Cli::try_parse_from(args)
}

fn parse_positive_usize(raw: &str) -> Result<usize, String> {
    let value = raw
        .parse::<usize>()
        .map_err(|_| "must be a positive integer".to_owned())?;
    if value == 0 {
        return Err("must be a positive integer".to_owned());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_exposes_only_graph_commands() {
        let parsed =
            parse_cli(["ffhn", "measure", "--source", "shop", "--dry-run"]).expect("measure");
        assert!(matches!(parsed.command, Command::Measure(_)));
        assert!(parse_cli(["ffhn", "run"]).is_err());
        assert_eq!(parse_positive_usize("2"), Ok(2));
        assert!(parse_positive_usize("0").is_err());
        assert!(parse_positive_usize("not-a-number").is_err());
    }
}

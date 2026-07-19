//! FFHN command-line input vocabulary, command construction, and match translation.

use std::path::PathBuf;

use clap::ValueEnum;
use ffhn_core::TargetId;

mod definition;
pub(crate) mod parse;
#[cfg(test)]
mod tests;

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
    /// Blindly remove one target's isolated v2 storage root.
    Reset(ResetCommand),
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

/// Reset-command arguments.
#[derive(Debug, PartialEq, Eq)]
pub struct ResetCommand {
    /// Watch root directory containing per-target subdirectories.
    pub watch_root: PathBuf,
    /// Target id whose v2 storage root must be deleted.
    pub target: TargetId,
    /// Output format for the emitted document.
    pub output_format: OutputFormat,
}

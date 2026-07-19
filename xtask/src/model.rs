use std::collections::BTreeMap;
use std::error::Error;
use std::path::PathBuf;

use serde::Deserialize;

/// Convenience result type used by the FFHN maintenance helpers.
pub type DynResult<T> = Result<T, Box<dyn Error>>;
pub(crate) type BranchSpan = (u64, u64, u64, u64);
pub(crate) type BranchCounts = (u64, u64);
pub(crate) type BranchCoverageByFile = BTreeMap<PathBuf, BTreeMap<BranchSpan, BranchCounts>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Cargo artifact-root policy for one external maintainer command.
pub(crate) enum CommandArtifactLayout {
    /// Use the ambient Cargo artifact layout.
    Inherit,
    /// Route Cargo output into the managed workspace artifact roots.
    ManagedWorkspace,
    /// Route Cargo output into the isolated managed coverage artifact roots.
    ManagedCoverage,
}

/// One external command in the maintenance plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandSpec {
    /// Stable identifier of the check step that owns this command.
    pub step_id: String,
    /// Program to execute.
    pub program: PathBuf,
    /// Arguments to pass.
    pub args: Vec<String>,
    /// Environment overrides to apply for the command.
    pub env: BTreeMap<String, String>,
    /// Whether stdout should be suppressed.
    pub quiet_stdout: bool,
    /// Artifact-root policy for the command.
    pub artifact_layout: CommandArtifactLayout,
}

impl CommandSpec {
    /// Builds one command specification.
    pub(crate) fn new<I, S>(program: impl Into<PathBuf>, args: I, quiet_stdout: bool) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            step_id: "unnamed-command".to_owned(),
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            env: BTreeMap::new(),
            quiet_stdout,
            artifact_layout: CommandArtifactLayout::Inherit,
        }
    }

    /// Assigns the stable maintainer-gate step identifier for this command.
    pub(crate) fn with_step_id(mut self, step_id: impl Into<String>) -> Self {
        self.step_id = step_id.into();
        self
    }

    /// Attaches environment overrides to the command specification.
    pub(crate) fn with_envs<I, K, V>(mut self, envs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.env = envs
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect();
        self
    }

    /// Overrides the artifact-root policy for the command specification.
    pub(crate) fn with_artifact_layout(mut self, artifact_layout: CommandArtifactLayout) -> Self {
        self.artifact_layout = artifact_layout;
        self
    }
}

/// Top-level `cargo llvm-cov --json` payload.
#[derive(Debug, Deserialize)]
pub(crate) struct CoverageReport {
    /// Datasets.
    #[serde(default)]
    pub data: Vec<CoverageDataSet>,
}

/// One dataset entry inside the coverage payload.
#[derive(Debug, Deserialize)]
pub(crate) struct CoverageDataSet {
    /// File coverage records.
    #[serde(default)]
    pub files: Vec<CoverageFile>,
}

/// Coverage details for one source file.
#[derive(Debug, Deserialize)]
pub(crate) struct CoverageFile {
    /// Absolute file name.
    pub filename: PathBuf,
    /// Segment records.
    #[serde(default)]
    pub segments: Vec<(u64, u64, u64, bool, bool, bool)>,
    /// Branch coverage records.
    #[serde(default)]
    pub branches: Vec<CoverageBranchRecord>,
    /// Aggregate branch summary.
    #[serde(default)]
    pub summary: CoverageFileSummary,
}

/// Raw branch tuple emitted by llvm-cov.
pub(crate) type CoverageBranchRecord = (u64, u64, u64, u64, u64, u64, u64, u64, u64);

/// File-level branch summary.
#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CoverageFileSummary {
    /// Branch counters.
    #[serde(default)]
    pub branches: CoverageCounter,
}

/// Generic coverage counter tuple.
#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CoverageCounter {
    /// Total tracked entities.
    #[serde(default)]
    pub count: u64,
    /// Covered entities.
    #[serde(default)]
    pub covered: u64,
    /// Uncovered entities.
    #[serde(default, rename = "notcovered")]
    pub not_covered: u64,
}

/// One tracked file that missed the coverage bar.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CoverageFailure {
    /// Repo-relative file path.
    pub file: String,
    /// Uncovered executable lines.
    pub uncovered_lines: Vec<String>,
    /// Uncovered branch count.
    pub uncovered_branch_count: usize,
}

/// Coverage status across the tracked files.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CoverageSummary {
    /// Total tracked executable line count.
    pub tracked_line_count: usize,
    /// Total tracked branch count.
    pub tracked_branch_count: usize,
    /// Files that missed coverage.
    pub failures: Vec<CoverageFailure>,
}

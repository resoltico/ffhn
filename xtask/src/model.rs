use std::collections::BTreeMap;
use std::error::Error;
use std::path::PathBuf;

use serde::Deserialize;

/// Convenience result type used by the FFHN maintenance helpers.
pub type DynResult<T> = Result<T, Box<dyn Error>>;
pub(crate) type BranchSpan = (u64, u64, u64, u64);
pub(crate) type BranchCounts = (u64, u64);
pub(crate) type BranchCoverageByFile = BTreeMap<PathBuf, BTreeMap<BranchSpan, BranchCounts>>;

pub(crate) const TRACKED_RELATIVE_PATHS: &[&str] = &[
    "crates/ffhn-core/src/canonical.rs",
    "crates/ffhn-core/src/contract/catalog.rs",
    "crates/ffhn-core/src/contract/types.rs",
    "crates/ffhn-core/src/fetch.rs",
    "crates/ffhn-core/src/fetch/file.rs",
    "crates/ffhn-core/src/fetch/http.rs",
    "crates/ffhn-core/src/model/extraction.rs",
    "crates/ffhn-core/src/model/report/batch.rs",
    "crates/ffhn-core/src/model/report/checks.rs",
    "crates/ffhn-core/src/model/report/notification.rs",
    "crates/ffhn-core/src/model/report/run.rs",
    "crates/ffhn-core/src/model/report/status.rs",
    "crates/ffhn-core/src/model/state.rs",
    "crates/ffhn-core/src/model/target/defaults.rs",
    "crates/ffhn-core/src/model/target/types.rs",
    "crates/ffhn-core/src/model/target/validation.rs",
    "crates/ffhn-core/src/model/validate.rs",
    "crates/ffhn-core/src/model/vocab.rs",
    "crates/ffhn-core/src/runtime/interop.rs",
    "crates/ffhn-core/src/runtime/lock.rs",
    "crates/ffhn-core/src/runtime/persist/snapshot_store.rs",
    "crates/ffhn-core/src/runtime/persist/state_update.rs",
    "crates/ffhn-core/src/runtime/persist/successful.rs",
    "crates/ffhn-core/src/runtime/run/batch.rs",
    "crates/ffhn-core/src/runtime/run/execute.rs",
    "crates/ffhn-core/src/runtime/run/failures.rs",
    "crates/ffhn-core/src/runtime/run/outcome.rs",
    "crates/ffhn-core/src/runtime/state.rs",
    "crates/ffhn-core/src/runtime/status.rs",
    "crates/ffhn-core/src/runtime/storage.rs",
    "crates/ffhn-core/src/stable_json.rs",
    "crates/ffhn-cli/src/args.rs",
    "crates/ffhn-cli/src/execute.rs",
    "xtask/src/model.rs",
    "xtask/src/plan.rs",
    "xtask/src/coverage.rs",
    "xtask/src/repo_contract/mod.rs",
];
pub(crate) const COVERAGE_TOOLCHAIN: &str = "+nightly";

/// One external command in the maintenance plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandSpec {
    /// Program to execute.
    pub program: PathBuf,
    /// Arguments to pass.
    pub args: Vec<String>,
    /// Environment overrides to apply for the command.
    pub env: BTreeMap<String, String>,
    /// Whether stdout should be suppressed.
    pub quiet_stdout: bool,
    /// Whether clang should be forced.
    pub force_clang: bool,
}

impl CommandSpec {
    /// Builds one command specification.
    pub(crate) fn new<I, S>(
        program: impl Into<PathBuf>,
        args: I,
        quiet_stdout: bool,
        force_clang: bool,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            env: BTreeMap::new(),
            quiet_stdout,
            force_clang,
        }
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

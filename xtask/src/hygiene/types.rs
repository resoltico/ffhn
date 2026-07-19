//! Hygiene report vocabulary, artifact policy constants, and cleanup outcomes.

use std::path::PathBuf;

use clap::ValueEnum;
use serde::Serialize;

pub(super) const GIB: u64 = 1024 * 1024 * 1024;
pub(super) const MIB: u64 = 1024 * 1024;
pub(super) const ARTIFACT_MANIFEST_NAME: &str = ".ffhn-artifact.toml";
pub(super) const CACHEDIR_TAG_NAME: &str = "CACHEDIR.TAG";
pub(super) const CACHEDIR_TAG_CONTENTS: &str = "Signature: 8a477f597d28d172789f06886806bc55\n# This directory stores disposable FFHN build cache data.\n";
pub(super) const SCHEMA_NAME: &str = "xtask.hygiene_report@1";
pub(super) const ARTIFACT_SCHEMA_NAME: &str = "xtask.artifact_root@1";
pub(super) const MANAGED_TARGET_BUDGET_BYTES: u64 = 4 * GIB;
pub(super) const MANAGED_BUILD_BUDGET_BYTES: u64 = 24 * GIB;
pub(super) const MANAGED_COVERAGE_TARGET_BUDGET_BYTES: u64 = 2 * GIB;
pub(super) const MANAGED_COVERAGE_BUILD_BUDGET_BYTES: u64 = 8 * GIB;
pub(super) const LEGACY_REPO_TARGET_BUDGET_BYTES: u64 = 512 * MIB;
pub(super) const LEGACY_REPO_FUZZ_TARGET_BUDGET_BYTES: u64 = 512 * MIB;
pub(super) const REPO_TMP_BUDGET_BYTES: u64 = 256 * MIB;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ManagedArtifactKind {
    WorkspaceTarget,
    WorkspaceBuild,
    CoverageTarget,
    CoverageBuild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
/// Output format for `cargo xtask hygiene report`.
pub enum HygieneReportFormat {
    /// Human-readable text.
    Text,
    /// Structured JSON.
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
/// Cleanup profile for `cargo xtask hygiene clean`.
pub enum HygieneCleanMode {
    /// Remove disposable scratch, repo-local temporary workspace state, and accidental legacy target trees.
    Safe,
    /// Remove every rebuildable artifact root, including managed Cargo caches.
    Rebuildable,
}

#[derive(Debug, Serialize)]
/// Machine-readable repository artifact report.
pub struct HygieneReport {
    /// Schema identity for the report document.
    pub schema: &'static str,
    /// Repository root that owns this report.
    pub repo_root: String,
    /// Total bytes across the reported entries.
    pub total_bytes: u64,
    /// Reported artifact entries.
    pub entries: Vec<HygieneEntry>,
    /// Policy violations detected for the current repository.
    pub violations: Vec<HygieneViolation>,
}

#[derive(Debug, Serialize)]
/// One classified artifact root or aggregate inside the hygiene report.
pub struct HygieneEntry {
    /// Stable report identifier.
    pub id: String,
    /// Artifact class name.
    pub kind: String,
    /// Path represented by the entry.
    pub path: String,
    /// Whether the path currently exists.
    pub present: bool,
    /// Total bytes under the path or aggregate.
    pub bytes: u64,
    /// Optional budget enforced for the entry.
    pub budget_bytes: Option<u64>,
    /// Whether the entry is owned by the managed hygiene system.
    pub managed: bool,
    /// Whether the entry is safe to delete and rebuild.
    pub safe_to_delete: bool,
    /// Human-readable details for aggregate entries.
    pub details: Vec<String>,
}

#[derive(Debug, Serialize)]
/// One hygiene-policy violation.
pub struct HygieneViolation {
    /// Report entry or policy identifier that failed.
    pub id: String,
    /// Human-readable explanation of the violation.
    pub message: String,
}

#[derive(Debug, Default)]
/// Summary of one cleanup operation.
pub struct HygieneCleanResult {
    /// Number of bytes reclaimed by the cleanup.
    pub reclaimed_bytes: u64,
    /// Removed artifact roots.
    pub removed_paths: Vec<PathBuf>,
}

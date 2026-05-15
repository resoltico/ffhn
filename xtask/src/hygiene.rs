use std::fs;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use serde::Serialize;

use crate::app::remove_dir_if_exists;
use crate::model::{CommandArtifactLayout, DynResult};
use crate::plan::{
    cargo_build_root, cargo_target_root, coverage_build_root, coverage_cargo_build_dir,
    coverage_cargo_target_dir, coverage_target_root, semver_baseline_target_dir, semver_build_dir,
    semver_scratch_dir,
};

const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;
const ARTIFACT_MANIFEST_NAME: &str = ".ffhn-artifact.toml";
const CACHEDIR_TAG_NAME: &str = "CACHEDIR.TAG";
const CACHEDIR_TAG_CONTENTS: &str = "Signature: 8a477f597d28d172789f06886806bc55\n# This directory stores disposable FFHN build cache data.\n";
const SCHEMA_NAME: &str = "xtask.hygiene_report@1";
const ARTIFACT_SCHEMA_NAME: &str = "xtask.artifact_root@1";
const MANAGED_TARGET_BUDGET_BYTES: u64 = 4 * GIB;
const MANAGED_BUILD_BUDGET_BYTES: u64 = 24 * GIB;
const MANAGED_COVERAGE_TARGET_BUDGET_BYTES: u64 = 2 * GIB;
const MANAGED_COVERAGE_BUILD_BUDGET_BYTES: u64 = 8 * GIB;
const LEGACY_REPO_TARGET_BUDGET_BYTES: u64 = 512 * MIB;
const LEGACY_REPO_FUZZ_TARGET_BUDGET_BYTES: u64 = 512 * MIB;
const REPO_TMP_BUDGET_BYTES: u64 = 256 * MIB;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ManagedArtifactKind {
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

#[derive(Debug, Serialize)]
struct ArtifactRootManifest<'a> {
    schema: &'static str,
    kind: ManagedArtifactKind,
    repo_root: &'a str,
    safe_to_delete: bool,
    purpose: &'a str,
}

struct EntrySpec<'a> {
    id: &'a str,
    kind: &'a str,
    path: &'a Path,
    budget_bytes: Option<u64>,
    managed: bool,
    safe_to_delete: bool,
    details: Vec<String>,
}

/// Prepares the managed artifact roots for one command layout and returns the env paths to use.
pub fn prepare_artifact_layout(
    repo_root: &Path,
    layout: CommandArtifactLayout,
) -> DynResult<Option<(PathBuf, PathBuf)>> {
    match layout {
        CommandArtifactLayout::Inherit => Ok(None),
        CommandArtifactLayout::ManagedWorkspace => {
            let target_root = cargo_target_root(repo_root);
            let build_root = cargo_build_root(repo_root);
            prepare_managed_artifact_roots(
                repo_root,
                [
                    (
                        target_root.as_path(),
                        ManagedArtifactKind::WorkspaceTarget,
                        "final Cargo artifacts for maintained workspace commands",
                    ),
                    (
                        build_root.as_path(),
                        ManagedArtifactKind::WorkspaceBuild,
                        "intermediate Cargo build cache for maintained workspace commands",
                    ),
                ],
            )?;
            Ok(Some((target_root, build_root)))
        }
        CommandArtifactLayout::ManagedCoverage => {
            let target_root = coverage_target_root(repo_root);
            let build_root = coverage_build_root(repo_root);
            let cargo_target_dir = coverage_cargo_target_dir(repo_root);
            let cargo_build_dir = coverage_cargo_build_dir(repo_root);
            prepare_managed_artifact_roots(
                repo_root,
                [
                    (
                        target_root.as_path(),
                        ManagedArtifactKind::CoverageTarget,
                        "managed coverage workspace root for the maintained llvm-cov gate",
                    ),
                    (
                        build_root.as_path(),
                        ManagedArtifactKind::CoverageBuild,
                        "managed coverage build root for the maintained llvm-cov gate",
                    ),
                    (
                        cargo_target_dir.as_path(),
                        ManagedArtifactKind::CoverageTarget,
                        "nested Cargo target root created by cargo llvm-cov",
                    ),
                    (
                        cargo_build_dir.as_path(),
                        ManagedArtifactKind::CoverageBuild,
                        "nested Cargo build root created by cargo llvm-cov",
                    ),
                ],
            )?;
            Ok(Some((target_root, build_root)))
        }
    }
}

/// Builds a full repository artifact report.
pub fn hygiene_report(repo_root: &Path) -> DynResult<HygieneReport> {
    reconcile_managed_artifact_roots(repo_root)?;

    let tmp_root = repo_root.join("tmp");
    let legacy_target_root = repo_root.join("target");
    let legacy_fuzz_target_root = repo_root.join("fuzz").join("target");
    let tmp_cargo_roots = repo_tmp_cargo_roots(repo_root)?;
    let managed_target = cargo_target_root(repo_root);
    let managed_build = cargo_build_root(repo_root);
    let managed_coverage_target = coverage_target_root(repo_root);
    let managed_coverage_build = coverage_build_root(repo_root);

    let mut entries = managed_entries([
        (
            "managed-workspace-target",
            "workspace-target",
            managed_target.as_path(),
            MANAGED_TARGET_BUDGET_BYTES,
        ),
        (
            "managed-workspace-build",
            "workspace-build",
            managed_build.as_path(),
            MANAGED_BUILD_BUDGET_BYTES,
        ),
        (
            "managed-coverage-target",
            "coverage-target",
            managed_coverage_target.as_path(),
            MANAGED_COVERAGE_TARGET_BUDGET_BYTES,
        ),
        (
            "managed-coverage-build",
            "coverage-build",
            managed_coverage_build.as_path(),
            MANAGED_COVERAGE_BUILD_BUDGET_BYTES,
        ),
    ])?;
    entries.extend(unmanaged_entries([
        (
            "legacy-repo-target",
            "legacy-repo-target",
            legacy_target_root.as_path(),
            LEGACY_REPO_TARGET_BUDGET_BYTES,
            vec![
                "Legacy repo-local Cargo target tree. Use `cargo xtask hygiene clean --mode rebuildable` to reclaim it."
                    .to_owned(),
            ],
        ),
        (
            "legacy-repo-fuzz-target",
            "legacy-repo-fuzz-target",
            legacy_fuzz_target_root.as_path(),
            LEGACY_REPO_FUZZ_TARGET_BUDGET_BYTES,
            vec![
                "Legacy repo-local cargo-fuzz target tree. Use `cargo xtask hygiene clean --mode rebuildable` to reclaim it."
                    .to_owned(),
            ],
        ),
    ])?);
    entries.push(repo_tmp_entry(&tmp_root, &tmp_cargo_roots)?);
    entries.push(repo_tmp_cargo_entry(&tmp_root, &tmp_cargo_roots)?);

    let total_bytes = entries.iter().map(|entry| entry.bytes).sum();
    let violations = report_violations(&entries);
    entries.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(HygieneReport {
        schema: SCHEMA_NAME,
        repo_root: repo_root.display().to_string(),
        total_bytes,
        entries,
        violations,
    })
}

fn reconcile_managed_artifact_roots(repo_root: &Path) -> DynResult<()> {
    reconcile_managed_artifact_roots_if_present(
        repo_root,
        [
            (
                cargo_target_root(repo_root),
                ManagedArtifactKind::WorkspaceTarget,
                "final Cargo artifacts for maintained workspace commands",
            ),
            (
                cargo_build_root(repo_root),
                ManagedArtifactKind::WorkspaceBuild,
                "intermediate Cargo build cache for maintained workspace commands",
            ),
            (
                coverage_target_root(repo_root),
                ManagedArtifactKind::CoverageTarget,
                "managed coverage workspace root for the maintained llvm-cov gate",
            ),
            (
                coverage_build_root(repo_root),
                ManagedArtifactKind::CoverageBuild,
                "managed coverage build root for the maintained llvm-cov gate",
            ),
            (
                coverage_cargo_target_dir(repo_root),
                ManagedArtifactKind::CoverageTarget,
                "nested Cargo target root created by cargo llvm-cov",
            ),
            (
                coverage_cargo_build_dir(repo_root),
                ManagedArtifactKind::CoverageBuild,
                "nested Cargo build root created by cargo llvm-cov",
            ),
        ],
    )
}

/// Renders the report as human-readable text.
pub fn render_hygiene_report(report: &HygieneReport) -> String {
    let mut lines = vec![
        format!("schema: {}", report.schema),
        format!("repo_root: {}", report.repo_root),
        format!(
            "total_bytes: {} ({})",
            report.total_bytes,
            format_bytes(report.total_bytes)
        ),
        "entries:".to_owned(),
    ];

    for entry in &report.entries {
        let budget = entry
            .budget_bytes
            .map(format_bytes)
            .unwrap_or_else(|| "n/a".to_owned());
        lines.push(format!(
            "- {} | {} | {} | present={} | bytes={} ({}) | budget={} | managed={} | safe_to_delete={}",
            entry.id,
            entry.kind,
            entry.path,
            entry.present,
            entry.bytes,
            format_bytes(entry.bytes),
            budget,
            entry.managed,
            entry.safe_to_delete
        ));
        for detail in &entry.details {
            lines.push(format!("  detail: {detail}"));
        }
    }

    if report.violations.is_empty() {
        lines.push("violations: none".to_owned());
    } else {
        lines.push("violations:".to_owned());
        for violation in &report.violations {
            lines.push(format!("- {}: {}", violation.id, violation.message));
        }
    }

    lines.join("\n")
}

/// Fails when the repository violates the maintained hygiene policy.
pub fn ensure_hygiene(repo_root: &Path) -> DynResult<()> {
    let report = hygiene_report(repo_root)?;
    if report.violations.is_empty() {
        return Ok(());
    }

    Err(format!(
        "artifact hygiene policy failed.\n{}\n\nRepair with `cargo xtask hygiene report` and `cargo xtask hygiene clean --mode rebuildable`.",
        render_hygiene_report(&report)
    )
    .into())
}

/// Removes disposable artifact roots according to the requested cleanup mode.
pub fn clean_hygiene(repo_root: &Path, mode: HygieneCleanMode) -> DynResult<HygieneCleanResult> {
    let mut removal_roots = vec![
        coverage_target_root(repo_root),
        coverage_build_root(repo_root),
        semver_scratch_dir(repo_root),
        semver_build_dir(repo_root),
        semver_baseline_target_dir(repo_root),
        repo_root.join("tmp"),
        repo_root.join("target"),
        repo_root.join("fuzz").join("target"),
        repo_root.join("target").join("llvm-cov-target"),
        repo_root.join("target").join("semver-checks"),
    ];

    if mode == HygieneCleanMode::Rebuildable {
        removal_roots.push(cargo_target_root(repo_root));
        removal_roots.push(cargo_build_root(repo_root));
    }

    let removal_roots = deduplicate_root_set(removal_roots);
    let mut result = HygieneCleanResult::default();

    for path in removal_roots {
        if !path.exists() {
            continue;
        }

        let bytes = dir_size_bytes(&path)?;
        remove_dir_if_exists(&path).map_err(|error| {
            format!(
                "failed to remove hygiene artifact root {}: {error}",
                path.display()
            )
        })?;
        result.reclaimed_bytes += bytes;
        result.removed_paths.push(path);
    }

    Ok(result)
}

fn prepare_managed_artifact_root(
    repo_root: &Path,
    artifact_root: &Path,
    kind: ManagedArtifactKind,
    purpose: &str,
) -> DynResult<()> {
    fs::create_dir_all(artifact_root).map_err(|error| {
        format!(
            "failed to create managed hygiene artifact root {}: {error}",
            artifact_root.display()
        )
    })?;
    write_cachedir_tag(artifact_root).map_err(|error| {
        format!(
            "failed to write managed hygiene cache marker {}: {error}",
            artifact_root.display()
        )
    })?;
    let repo_root_string = repo_root.display().to_string();
    let manifest = ArtifactRootManifest {
        schema: ARTIFACT_SCHEMA_NAME,
        kind,
        repo_root: &repo_root_string,
        safe_to_delete: true,
        purpose,
    };
    let manifest_path = artifact_root.join(ARTIFACT_MANIFEST_NAME);
    let manifest_contents = toml::to_string(&manifest)?;
    fs::write(&manifest_path, manifest_contents).map_err(|error| {
        format!(
            "failed to write managed hygiene manifest {}: {error}",
            manifest_path.display()
        )
    })?;
    Ok(())
}

fn prepare_managed_artifact_roots<const N: usize>(
    repo_root: &Path,
    roots: [(&Path, ManagedArtifactKind, &str); N],
) -> DynResult<()> {
    for (artifact_root, kind, purpose) in roots {
        prepare_managed_artifact_root(repo_root, artifact_root, kind, purpose)?;
    }
    Ok(())
}

fn reconcile_managed_artifact_roots_if_present<const N: usize>(
    repo_root: &Path,
    roots: [(PathBuf, ManagedArtifactKind, &str); N],
) -> DynResult<()> {
    for (artifact_root, kind, purpose) in roots {
        if artifact_root.exists() {
            prepare_managed_artifact_root(repo_root, &artifact_root, kind, purpose)?;
        }
    }
    Ok(())
}

fn write_cachedir_tag(path: &Path) -> DynResult<()> {
    let tag_path = path.join(CACHEDIR_TAG_NAME);
    if tag_path.exists() {
        return Ok(());
    }

    fs::write(tag_path, CACHEDIR_TAG_CONTENTS)?;
    Ok(())
}

fn managed_entries<const N: usize>(
    entries: [(&str, &str, &Path, u64); N],
) -> DynResult<Vec<HygieneEntry>> {
    entries
        .into_iter()
        .map(|(id, kind, path, budget_bytes)| {
            entry_from_path(id, kind, path, Some(budget_bytes), true, true, Vec::new())
        })
        .collect()
}

fn unmanaged_entries<const N: usize>(
    entries: [(&str, &str, &Path, u64, Vec<String>); N],
) -> DynResult<Vec<HygieneEntry>> {
    entries
        .into_iter()
        .map(|(id, kind, path, budget_bytes, details)| {
            entry_from_path(id, kind, path, Some(budget_bytes), false, true, details)
        })
        .collect()
}

fn repo_tmp_entry(tmp_root: &Path, tmp_cargo_roots: &[PathBuf]) -> DynResult<HygieneEntry> {
    let mut details = vec![
        "Repository scratch root mandated by AGENTS.md for temporary investigations.".to_owned(),
    ];
    if !tmp_cargo_roots.is_empty() {
        details.push(format!(
            "Excludes {} repo-local Cargo target roots reported separately under repo-tmp-cargo-targets.",
            tmp_cargo_roots.len()
        ));
    }

    entry_from_path_excluding_roots(
        EntrySpec {
            id: "repo-tmp",
            kind: "repo-tmp",
            path: tmp_root,
            budget_bytes: Some(REPO_TMP_BUDGET_BYTES),
            managed: false,
            safe_to_delete: true,
            details,
        },
        tmp_cargo_roots,
    )
}

fn repo_tmp_cargo_entry(tmp_root: &Path, tmp_cargo_roots: &[PathBuf]) -> DynResult<HygieneEntry> {
    aggregate_entry(
        "repo-tmp-cargo-targets",
        "repo-tmp-cargo-targets",
        tmp_root,
        tmp_cargo_roots,
        None,
        false,
        true,
    )
}

fn aggregate_entry(
    id: &str,
    kind: &str,
    path: &Path,
    roots: &[PathBuf],
    budget_bytes: Option<u64>,
    managed: bool,
    safe_to_delete: bool,
) -> DynResult<HygieneEntry> {
    let mut bytes = 0u64;
    for root in roots {
        bytes += dir_size_bytes(root).map_err(|error| {
            format!(
                "failed to inspect hygiene aggregate member {}: {error}",
                root.display()
            )
        })?;
    }
    let details = roots
        .iter()
        .map(|root| root.display().to_string())
        .collect::<Vec<_>>();

    Ok(HygieneEntry {
        id: id.to_owned(),
        kind: kind.to_owned(),
        path: path.display().to_string(),
        present: !roots.is_empty(),
        bytes,
        budget_bytes,
        managed,
        safe_to_delete,
        details,
    })
}

fn entry_from_path(
    id: &str,
    kind: &str,
    path: &Path,
    budget_bytes: Option<u64>,
    managed: bool,
    safe_to_delete: bool,
    details: Vec<String>,
) -> DynResult<HygieneEntry> {
    entry_from_path_excluding_roots(
        EntrySpec {
            id,
            kind,
            path,
            budget_bytes,
            managed,
            safe_to_delete,
            details,
        },
        &[],
    )
}

fn entry_from_path_excluding_roots(
    spec: EntrySpec<'_>,
    skipped_roots: &[PathBuf],
) -> DynResult<HygieneEntry> {
    Ok(HygieneEntry {
        id: spec.id.to_owned(),
        kind: spec.kind.to_owned(),
        path: spec.path.display().to_string(),
        present: spec.path.exists(),
        bytes: dir_size_bytes_excluding_roots(spec.path, skipped_roots).map_err(|error| {
            format!(
                "failed to inspect hygiene artifact root {}: {error}",
                spec.path.display()
            )
        })?,
        budget_bytes: spec.budget_bytes,
        managed: spec.managed,
        safe_to_delete: spec.safe_to_delete,
        details: spec.details,
    })
}

fn report_violations(entries: &[HygieneEntry]) -> Vec<HygieneViolation> {
    let mut violations = Vec::new();

    for entry in entries {
        if entry.managed && entry.present {
            let missing_markers = missing_managed_markers_for_entry(entry);
            if !missing_markers.is_empty() {
                violations.push(HygieneViolation {
                    id: entry.id.clone(),
                    message: format!(
                        "{} is missing managed-artifact markers: {}",
                        entry.path,
                        missing_markers.join(", ")
                    ),
                });
            }
        }

        if let Some(budget_bytes) = entry.budget_bytes
            && entry.bytes > budget_bytes
        {
            violations.push(HygieneViolation {
                id: entry.id.clone(),
                message: format!(
                    "{} exceeds its {} budget ({})",
                    entry.path,
                    format_bytes(budget_bytes),
                    format_bytes(entry.bytes)
                ),
            });
        }

        if entry.id == "repo-tmp-cargo-targets" && entry.present {
            violations.push(HygieneViolation {
                id: entry.id.clone(),
                message: format!(
                    "repository tmp contains {} cargo target roots; move those builds to the managed artifact roots",
                    entry.details.len()
                ),
            });
        }
    }

    violations
}

fn repo_tmp_cargo_roots(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    let tmp_root = repo_root.join("tmp");
    if !tmp_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut roots = fs::read_dir(&tmp_root)
        .map_err(|error| {
            format!(
                "failed to inspect repository temporary root {}: {error}",
                tmp_root.display()
            )
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| looks_like_cargo_target_dir(path))
        .collect::<Vec<_>>();
    roots.sort();
    Ok(roots)
}

fn looks_like_cargo_target_dir(path: &Path) -> bool {
    [
        ".fingerprint",
        ".rustc_info.json",
        "debug",
        "release",
        "dist",
        "package",
        "CACHEDIR.TAG",
    ]
    .iter()
    .any(|component| path.join(component).exists())
}

fn dir_size_bytes(path: &Path) -> DynResult<u64> {
    dir_size_bytes_excluding_roots(path, &[])
}

fn dir_size_bytes_excluding_roots(path: &Path, skipped_roots: &[PathBuf]) -> DynResult<u64> {
    if skipped_roots.iter().any(|root| path == root.as_path()) {
        return Ok(0);
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => {
            return Err(format!(
                "failed to read hygiene metadata {}: {error}",
                path.display()
            )
            .into());
        }
    };
    if metadata.file_type().is_symlink() {
        return Ok(0);
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Ok(0);
    }

    let entries = fs::read_dir(path).map_err(|error| {
        format!(
            "failed to read hygiene directory {}: {error}",
            path.display()
        )
    })?;
    entries.into_iter().try_fold(0u64, |total, entry| {
        let entry = entry?;
        dir_size_bytes_excluding_roots(&entry.path(), skipped_roots)
            .map(|entry_bytes| total + entry_bytes)
    })
}

fn deduplicate_root_set(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort();
    paths.dedup();
    let mut pruned = Vec::new();

    'candidate: for path in paths {
        for existing in &pruned {
            if path.starts_with(existing) {
                continue 'candidate;
            }
        }
        pruned.retain(|existing: &PathBuf| !existing.starts_with(&path));
        pruned.push(path);
    }

    pruned
}

fn missing_managed_markers(path: &Path) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if !path.join(CACHEDIR_TAG_NAME).is_file() {
        missing.push(CACHEDIR_TAG_NAME);
    }
    if !path.join(ARTIFACT_MANIFEST_NAME).is_file() {
        missing.push(ARTIFACT_MANIFEST_NAME);
    }
    missing
}

fn missing_managed_markers_for_entry(entry: &HygieneEntry) -> Vec<String> {
    let mut missing = missing_managed_markers(Path::new(&entry.path))
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();

    if entry.id == "managed-coverage-target" || entry.id == "managed-coverage-build" {
        let nested_path = Path::new(&entry.path).join("llvm-cov-target");
        missing.extend(
            missing_managed_markers(&nested_path)
                .into_iter()
                .map(|marker| format!("llvm-cov-target/{marker}")),
        );
    }

    missing
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= GIB {
        return format!("{:.1} GiB", bytes as f64 / GIB as f64);
    }
    if bytes >= MIB {
        return format!("{:.1} MiB", bytes as f64 / MIB as f64);
    }
    if bytes >= 1024 {
        return format!("{:.1} KiB", bytes as f64 / 1024.0);
    }

    format!("{bytes} B")
}

#[cfg(test)]
pub(crate) fn aggregate_entry_for_tests(path: &Path, roots: &[PathBuf]) -> DynResult<HygieneEntry> {
    aggregate_entry(
        "test-aggregate",
        "test-aggregate",
        path,
        roots,
        Some(0),
        false,
        true,
    )
}

#[cfg(test)]
pub(crate) fn dir_size_bytes_for_tests(path: &Path) -> u64 {
    dir_size_bytes(path).expect("dir size bytes")
}

#[cfg(test)]
pub(crate) fn dir_size_bytes_result_for_tests(path: &Path) -> DynResult<u64> {
    dir_size_bytes(path)
}

#[cfg(all(test, unix))]
pub(crate) fn entry_from_path_for_tests(path: &Path) -> DynResult<HygieneEntry> {
    entry_from_path(
        "test-entry",
        "test-entry",
        path,
        Some(0),
        true,
        true,
        Vec::new(),
    )
}

#[cfg(test)]
pub(crate) fn format_bytes_for_tests(bytes: u64) -> String {
    format_bytes(bytes)
}

#[cfg(test)]
pub(crate) fn looks_like_cargo_target_dir_for_tests(path: &Path) -> bool {
    looks_like_cargo_target_dir(path)
}

#[cfg(test)]
pub(crate) fn missing_managed_markers_for_tests(path: &Path) -> Vec<String> {
    missing_managed_markers(path)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
pub(crate) fn report_violations_for_tests(entries: &[HygieneEntry]) -> Vec<HygieneViolation> {
    report_violations(entries)
}

#[cfg(all(test, unix))]
pub(crate) fn repo_tmp_cargo_roots_for_tests(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    repo_tmp_cargo_roots(repo_root)
}

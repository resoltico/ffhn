//! Repository artifact inventory, rendering, and policy-violation evaluation.

use std::path::{Path, PathBuf};

use crate::model::DynResult;
use crate::plan::{cargo_build_root, cargo_target_root, coverage_build_root, coverage_target_root};

use super::filesystem::{
    dir_size_bytes, dir_size_bytes_excluding_roots, format_bytes,
    missing_managed_markers_for_entry, repo_tmp_cargo_roots,
};
use super::layout::reconcile_managed_artifact_roots;
struct EntrySpec<'a> {
    id: &'a str,
    kind: &'a str,
    path: &'a Path,
    budget_bytes: Option<u64>,
    managed: bool,
    safe_to_delete: bool,
    details: Vec<String>,
}

use super::types::{
    HygieneEntry, HygieneReport, HygieneViolation, LEGACY_REPO_FUZZ_TARGET_BUDGET_BYTES,
    LEGACY_REPO_TARGET_BUDGET_BYTES, MANAGED_BUILD_BUDGET_BYTES,
    MANAGED_COVERAGE_BUILD_BUDGET_BYTES, MANAGED_COVERAGE_TARGET_BUDGET_BYTES,
    MANAGED_TARGET_BUDGET_BYTES, REPO_TMP_BUDGET_BYTES, SCHEMA_NAME,
};

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

pub(super) fn aggregate_entry(
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

pub(super) fn entry_from_path(
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

pub(super) fn report_violations(entries: &[HygieneEntry]) -> Vec<HygieneViolation> {
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

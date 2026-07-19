//! Deterministic removal of disposable artifact roots.

use std::path::Path;

use crate::app::remove_dir_if_exists;
use crate::model::DynResult;
use crate::plan::{
    cargo_build_root, cargo_target_root, coverage_build_root, coverage_target_root,
    semver_baseline_target_dir, semver_build_dir, semver_scratch_dir,
};

use super::filesystem::{deduplicate_root_set, dir_size_bytes};
use super::types::{HygieneCleanMode, HygieneCleanResult};

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

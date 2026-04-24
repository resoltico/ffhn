use std::fs;
use std::path::{Path, PathBuf};

use crate::model::DynResult;

/// Canonicalizes one repo-relative or absolute path.
pub(crate) fn normalize_path(repo_root: &Path, path: &Path) -> DynResult<PathBuf> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    };

    Ok(fs::canonicalize(candidate)?)
}

/// Returns the dist-profile binary path used by the smoke check.
pub(crate) fn release_binary_path(repo_root: &Path) -> PathBuf {
    repo_root.join("target").join("dist").join(binary_name())
}

/// Returns the manifest path for the public `ffhn-core` crate.
pub(crate) fn core_manifest_path(repo_root: &Path) -> PathBuf {
    repo_root
        .join("crates")
        .join("ffhn-core")
        .join("Cargo.toml")
}

/// Returns the unpacked semver baseline directory for `ffhn-core`.
pub(crate) fn semver_baseline_path(repo_root: &Path) -> PathBuf {
    repo_root.join("semver-baseline").join("ffhn-core")
}

/// Returns the stale baseline-local target directory older semver runs may leave behind.
pub(crate) fn semver_baseline_target_dir(repo_root: &Path) -> PathBuf {
    semver_baseline_path(repo_root).join("target")
}

/// Returns the dedicated fuzz-package manifest path.
pub(crate) fn fuzz_manifest_path(repo_root: &Path) -> PathBuf {
    repo_root.join("fuzz").join("Cargo.toml")
}

/// Returns the dedicated fuzz-package lockfile path.
pub(crate) fn fuzz_lockfile_path(repo_root: &Path) -> PathBuf {
    repo_root.join("fuzz").join("Cargo.lock")
}

/// Returns the semver scratch directory under the Cargo target tree.
pub(crate) fn semver_scratch_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("target").join("semver-checks")
}

#[cfg(windows)]
/// Returns the platform-specific FFHN binary name.
pub(crate) fn binary_name() -> &'static str {
    "ffhn.exe"
}

#[cfg(not(windows))]
/// Returns the platform-specific FFHN binary name.
pub(crate) fn binary_name() -> &'static str {
    "ffhn"
}

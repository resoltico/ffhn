use std::fs;
use std::path::{Path, PathBuf};

use crate::model::DynResult;
use serde::Deserialize;

#[cfg(test)]
use std::cell::RefCell;

#[cfg(test)]
thread_local! {
    static TEST_CARGO_TARGET_ROOT_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    static TEST_CARGO_BUILD_ROOT_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

#[derive(Debug, Default, Deserialize)]
struct CargoConfigDocument {
    #[serde(default)]
    build: CargoBuildConfig,
}

#[derive(Debug, Default, Deserialize)]
struct CargoBuildConfig {
    #[serde(default, rename = "target-dir")]
    target_dir: Option<PathBuf>,
    #[serde(default, rename = "build-dir")]
    build_dir: Option<PathBuf>,
}

/// Canonicalizes one repo-relative or absolute path.
pub(crate) fn normalize_path(repo_root: &Path, path: &Path) -> DynResult<PathBuf> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    };

    Ok(fs::canonicalize(candidate)?)
}

/// Returns the active Cargo target root for maintainer and release commands.
pub(crate) fn cargo_target_root(repo_root: &Path) -> PathBuf {
    #[cfg(test)]
    if let Some(override_dir) = test_cargo_target_root_override() {
        return override_dir;
    }

    let config = cargo_build_config(repo_root);
    resolve_artifact_root(repo_root, config.target_dir, repo_root.join("target"))
}

/// Returns the active Cargo build root for maintainer and release commands.
pub(crate) fn cargo_build_root(repo_root: &Path) -> PathBuf {
    #[cfg(test)]
    if let Some(override_dir) = test_cargo_build_root_override() {
        return override_dir;
    }

    let config = cargo_build_config(repo_root);
    resolve_artifact_root(repo_root, config.build_dir, cargo_target_root(repo_root))
}

/// Returns the dist-profile binary path used by the smoke check.
pub(crate) fn release_binary_path(repo_root: &Path) -> PathBuf {
    cargo_target_root(repo_root)
        .join("dist")
        .join(binary_name())
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

/// Returns the isolated Cargo build root used by the coverage gate.
pub(crate) fn coverage_build_root(repo_root: &Path) -> PathBuf {
    sibling_artifact_dir(&cargo_build_root(repo_root), "coverage-build")
}

/// Returns the isolated Cargo target root used by the coverage gate.
pub(crate) fn coverage_target_root(repo_root: &Path) -> PathBuf {
    sibling_artifact_dir(&cargo_target_root(repo_root), "coverage-target")
}

/// Returns the managed evidence root retained for mutation-testing campaigns.
pub(crate) fn mutation_report_root(repo_root: &Path) -> PathBuf {
    sibling_artifact_dir(&cargo_target_root(repo_root), "mutation-runs")
}

/// Returns the nested Cargo build directory created by `cargo llvm-cov`.
pub(crate) fn coverage_cargo_build_dir(repo_root: &Path) -> PathBuf {
    coverage_build_root(repo_root).join("llvm-cov-target")
}

/// Returns the nested Cargo target directory created by `cargo llvm-cov`.
pub(crate) fn coverage_cargo_target_dir(repo_root: &Path) -> PathBuf {
    coverage_target_root(repo_root).join("llvm-cov-target")
}

/// Returns the dedicated fuzz-package manifest path.
pub(crate) fn fuzz_manifest_path(repo_root: &Path) -> PathBuf {
    repo_root.join("fuzz").join("Cargo.toml")
}

/// Returns the dedicated fuzz-package lockfile path.
pub(crate) fn fuzz_lockfile_path(repo_root: &Path) -> PathBuf {
    repo_root.join("fuzz").join("Cargo.lock")
}

/// Returns the semver scratch target directory under the managed Cargo target tree.
pub(crate) fn semver_scratch_dir(repo_root: &Path) -> PathBuf {
    cargo_target_root(repo_root).join("semver-checks")
}

/// Returns the semver scratch build directory under the managed Cargo build tree.
pub(crate) fn semver_build_dir(repo_root: &Path) -> PathBuf {
    cargo_build_root(repo_root).join("semver-checks")
}

/// Returns the platform-specific FFHN binary name.
pub(crate) fn binary_name() -> &'static str {
    binary_name_for_windows(cfg!(windows))
}

fn binary_name_for_windows(is_windows: bool) -> &'static str {
    if is_windows { "ffhn.exe" } else { "ffhn" }
}

fn cargo_build_config(repo_root: &Path) -> CargoBuildConfig {
    let config_path = repo_root.join(".cargo").join("config.toml");
    let Ok(contents) = fs::read_to_string(config_path) else {
        return CargoBuildConfig::default();
    };

    toml::from_str::<CargoConfigDocument>(&contents)
        .map(|document| document.build)
        .unwrap_or_default()
}

fn resolve_artifact_root(
    repo_root: &Path,
    configured_root: Option<PathBuf>,
    default_root: PathBuf,
) -> PathBuf {
    match configured_root {
        Some(root) => repo_root.join(root),
        None => default_root,
    }
}

fn sibling_artifact_dir(path: &Path, sibling_name: &str) -> PathBuf {
    path.parent()
        .map(|parent| parent.join(sibling_name))
        .unwrap_or_else(|| PathBuf::from(sibling_name))
}

#[cfg(test)]
pub(crate) fn cargo_target_root_for_tests(repo_root: &Path, target_root: Option<&Path>) -> PathBuf {
    resolve_artifact_root(
        repo_root,
        target_root.map(Path::to_path_buf),
        repo_root.join("target"),
    )
}

#[cfg(test)]
pub(crate) fn cargo_build_root_for_tests(repo_root: &Path, build_root: Option<&Path>) -> PathBuf {
    resolve_artifact_root(
        repo_root,
        build_root.map(Path::to_path_buf),
        cargo_target_root_for_tests(repo_root, None),
    )
}

#[cfg(test)]
pub(crate) fn coverage_target_root_for_tests(
    repo_root: &Path,
    target_root: Option<&Path>,
) -> PathBuf {
    sibling_artifact_dir(
        &cargo_target_root_for_tests(repo_root, target_root),
        "coverage-target",
    )
}

#[cfg(test)]
pub(crate) fn coverage_build_root_for_tests(
    repo_root: &Path,
    build_root: Option<&Path>,
) -> PathBuf {
    sibling_artifact_dir(
        &cargo_build_root_for_tests(repo_root, build_root),
        "coverage-build",
    )
}

#[cfg(test)]
pub(crate) fn mutation_report_root_for_tests(
    repo_root: &Path,
    target_root: Option<&Path>,
) -> PathBuf {
    sibling_artifact_dir(
        &cargo_target_root_for_tests(repo_root, target_root),
        "mutation-runs",
    )
}

#[cfg(test)]
pub(crate) fn coverage_cargo_target_dir_for_tests(
    repo_root: &Path,
    target_root: Option<&Path>,
) -> PathBuf {
    coverage_target_root_for_tests(repo_root, target_root).join("llvm-cov-target")
}

#[cfg(test)]
pub(crate) fn coverage_cargo_build_dir_for_tests(
    repo_root: &Path,
    build_root: Option<&Path>,
) -> PathBuf {
    coverage_build_root_for_tests(repo_root, build_root).join("llvm-cov-target")
}

#[cfg(test)]
pub(crate) fn semver_scratch_dir_for_tests(
    repo_root: &Path,
    target_root: Option<&Path>,
) -> PathBuf {
    cargo_target_root_for_tests(repo_root, target_root).join("semver-checks")
}

#[cfg(test)]
pub(crate) fn release_binary_path_for_tests(
    repo_root: &Path,
    target_root: Option<&Path>,
) -> PathBuf {
    cargo_target_root_for_tests(repo_root, target_root)
        .join("dist")
        .join(binary_name())
}

#[cfg(test)]
pub(crate) fn sibling_artifact_dir_for_tests(path: &Path, sibling_name: &str) -> PathBuf {
    sibling_artifact_dir(path, sibling_name)
}

#[cfg(test)]
pub(crate) fn binary_name_for_windows_for_tests(is_windows: bool) -> &'static str {
    binary_name_for_windows(is_windows)
}

#[cfg(test)]
pub(crate) fn with_cargo_artifact_root_overrides<T>(
    target_root: PathBuf,
    build_root: PathBuf,
    operation: impl FnOnce() -> T,
) -> T {
    TEST_CARGO_TARGET_ROOT_OVERRIDE.with_borrow_mut(|slot| {
        assert!(
            slot.is_none(),
            "test cargo target root override should not already be installed"
        );
        *slot = Some(target_root);
    });
    TEST_CARGO_BUILD_ROOT_OVERRIDE.with_borrow_mut(|slot| {
        assert!(
            slot.is_none(),
            "test cargo build root override should not already be installed"
        );
        *slot = Some(build_root);
    });

    let outcome = operation();

    TEST_CARGO_TARGET_ROOT_OVERRIDE.with_borrow_mut(|slot| {
        *slot = None;
    });
    TEST_CARGO_BUILD_ROOT_OVERRIDE.with_borrow_mut(|slot| {
        *slot = None;
    });

    outcome
}

#[cfg(test)]
fn test_cargo_target_root_override() -> Option<PathBuf> {
    TEST_CARGO_TARGET_ROOT_OVERRIDE.with_borrow(|slot| slot.clone())
}

#[cfg(test)]
fn test_cargo_build_root_override() -> Option<PathBuf> {
    TEST_CARGO_BUILD_ROOT_OVERRIDE.with_borrow(|slot| slot.clone())
}

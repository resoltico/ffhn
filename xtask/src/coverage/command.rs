use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::model::{CommandArtifactLayout, CommandSpec, DynResult};
use crate::plan::{coverage_target_root, normalize_path};
use crate::repo_files::maintained_rust_source_entries;
use crate::tooling::RustTooling;

/// Builds the llvm-cov command used by the coverage gate.
pub(crate) fn coverage_command(repo_root: &Path, tooling: &RustTooling) -> CommandSpec {
    CommandSpec::new(
        "cargo",
        [
            &tooling.qa_nightly_toolchain_arg(),
            "llvm-cov",
            "--branch",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
            "--json",
            "--output-path",
            coverage_output_path(repo_root).to_string_lossy().as_ref(),
            "--",
            "--test-threads=1",
        ],
        false,
    )
    .with_envs([("CC", "clang")])
    .with_artifact_layout(CommandArtifactLayout::ManagedCoverage)
}

/// Builds the cleanup command that resets llvm-cov state.
pub(crate) fn coverage_clean_command(tooling: &RustTooling) -> CommandSpec {
    CommandSpec::new(
        "cargo",
        [
            &tooling.qa_nightly_toolchain_arg(),
            "llvm-cov",
            "clean",
            "--workspace",
        ],
        false,
    )
    .with_artifact_layout(CommandArtifactLayout::ManagedCoverage)
}

/// Returns the coverage JSON output path.
pub(crate) fn coverage_output_path(repo_root: &Path) -> PathBuf {
    coverage_target_root(repo_root).join("coverage.json")
}

/// Loads the maintained non-test Rust source files for the coverage gate.
pub(crate) fn tracked_files(repo_root: &Path) -> DynResult<BTreeMap<PathBuf, String>> {
    let mut tracked = BTreeMap::new();

    for (source_path, relative_path) in maintained_rust_source_entries(repo_root)? {
        let absolute = normalize_path(repo_root, &source_path)?;
        tracked.insert(absolute, relative_path);
    }

    Ok(tracked)
}

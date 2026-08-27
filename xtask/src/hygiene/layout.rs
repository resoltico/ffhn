//! Managed Cargo artifact-root creation and marker reconciliation.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::model::{CommandArtifactLayout, DynResult};
use crate::plan::{
    cargo_build_root, cargo_target_root, coverage_build_root, coverage_cargo_build_dir,
    coverage_cargo_target_dir, coverage_target_root, mutation_report_root,
};

use super::types::{
    ARTIFACT_MANIFEST_NAME, ARTIFACT_SCHEMA_NAME, CACHEDIR_TAG_CONTENTS, CACHEDIR_TAG_NAME,
    ManagedArtifactKind,
};

#[derive(Debug, Serialize)]
struct ArtifactRootManifest<'a> {
    schema: &'static str,
    kind: ManagedArtifactKind,
    repo_root: &'a str,
    safe_to_delete: bool,
    purpose: &'a str,
}

/// Prepares and returns the managed root that retains mutation-testing evidence.
pub fn prepare_mutation_report_root(repo_root: &Path) -> DynResult<PathBuf> {
    let root = mutation_report_root(repo_root);
    prepare_managed_artifact_roots(
        repo_root,
        [(
            root.as_path(),
            ManagedArtifactKind::MutationReports,
            "retained cargo-mutants results for the latest runtime and tooling campaigns",
        )],
    )?;
    Ok(root)
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

pub(super) fn reconcile_managed_artifact_roots(repo_root: &Path) -> DynResult<()> {
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
            (
                mutation_report_root(repo_root),
                ManagedArtifactKind::MutationReports,
                "retained cargo-mutants results for the latest runtime and tooling campaigns",
            ),
        ],
    )
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

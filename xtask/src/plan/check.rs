use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::model::{CommandArtifactLayout, CommandSpec, DynResult};
use crate::tooling::RustTooling;

use super::{
    core_manifest_path, fuzz_lockfile_path, fuzz_manifest_path, release_binary_path,
    semver_baseline_path, semver_build_dir, semver_release_type, semver_scratch_dir,
};

/// Builds the ordered command plan for `cargo xtask check`.
pub(crate) fn check_plan(repo_root: &Path, tooling: &RustTooling) -> DynResult<Vec<CommandSpec>> {
    let scripts = shell_script_paths(repo_root)?;
    let mut plan = Vec::new();

    if !scripts.is_empty() {
        plan.push(CommandSpec::new(
            "bash",
            std::iter::once("-n".to_owned()).chain(path_strings(&scripts)),
            false,
        ));
        plan.push(CommandSpec::new(
            "shellcheck",
            path_strings(&scripts),
            false,
        ));
    }

    plan.push(
        CommandSpec::new("cargo", ["fmt", "--check"], false)
            .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace),
    );
    plan.push(
        CommandSpec::new(
            "cargo",
            [
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--locked",
                "--",
                "-D",
                "warnings",
            ],
            false,
        )
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace),
    );
    plan.push(
        CommandSpec::new("cargo", ["xtask", "audit"], false)
            .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace),
    );
    plan.push(
        CommandSpec::new(
            "cargo",
            [
                "xtask",
                "audit",
                "--file",
                fuzz_lockfile_path(repo_root).to_string_lossy().as_ref(),
            ],
            false,
        )
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace),
    );
    plan.push(
        CommandSpec::new(
            "cargo",
            ["deny", "check", "advisories", "bans", "licenses", "sources"],
            false,
        )
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace),
    );
    plan.push(semver_check_spec(repo_root)?);
    plan.push(
        CommandSpec::new(
            "cargo",
            [
                "check",
                "--manifest-path",
                fuzz_manifest_path(repo_root).to_string_lossy().as_ref(),
                "--bins",
                "--locked",
            ],
            false,
        )
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace),
    );
    plan.push(
        CommandSpec::new(
            "cargo",
            [
                "clippy",
                "--manifest-path",
                fuzz_manifest_path(repo_root).to_string_lossy().as_ref(),
                "--bins",
                "--locked",
                "--",
                "-D",
                "warnings",
            ],
            false,
        )
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace),
    );
    plan.push(
        CommandSpec::new(
            "cargo",
            [
                &tooling.coverage_toolchain_arg(),
                "fuzz",
                "check",
                "--fuzz-dir",
                "fuzz",
            ],
            false,
        )
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace),
    );
    plan.push(
        CommandSpec::new(
            "cargo",
            [
                "nextest",
                "run",
                "--no-fail-fast",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--locked",
            ],
            false,
        )
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace),
    );
    plan.push(
        CommandSpec::new(
            "cargo",
            ["test", "--workspace", "--doc", "--all-features", "--locked"],
            false,
        )
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace),
    );
    plan.push(
        CommandSpec::new(
            "cargo",
            [
                "doc",
                "--workspace",
                "--all-features",
                "--no-deps",
                "--locked",
            ],
            false,
        )
        .with_envs([("RUSTDOCFLAGS", "-D warnings")])
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace),
    );
    plan.push(
        CommandSpec::new(
            "cargo",
            [
                "build",
                "--profile",
                "dist",
                "-p",
                "ffhn-cli",
                "--bin",
                "ffhn",
                "--locked",
            ],
            false,
        )
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace),
    );
    plan.push(CommandSpec::new(
        release_binary_path(repo_root),
        ["--version"],
        true,
    ));

    Ok(plan)
}

/// Lists shell scripts that should be linted by the maintenance gate.
pub(crate) fn shell_script_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    let root_check = repo_root.join("check.sh");
    let scripts_dir = repo_root.join("scripts");
    let mut scripts = Vec::new();

    if root_check.is_file() {
        scripts.push(root_check);
    }

    if scripts_dir.is_dir() {
        let read_dir = fs::read_dir(&scripts_dir)?.map(|entry| entry.map(|entry| entry.path()));
        scripts.extend(collect_shell_script_paths(read_dir)?);
    }
    scripts.sort();
    Ok(scripts)
}

pub(crate) fn semver_check_spec(repo_root: &Path) -> DynResult<CommandSpec> {
    let semver_release_type = semver_release_type(repo_root)?;
    Ok(CommandSpec::new(
        "cargo",
        [
            "semver-checks",
            "--manifest-path",
            core_manifest_path(repo_root).to_string_lossy().as_ref(),
            "--baseline-root",
            semver_baseline_path(repo_root).to_string_lossy().as_ref(),
            "--release-type",
            semver_release_type.as_str(),
            "--all-features",
        ],
        false,
    )
    .with_envs([
        (
            "CARGO_TARGET_DIR",
            semver_scratch_dir(repo_root).to_string_lossy().into_owned(),
        ),
        (
            "CARGO_BUILD_BUILD_DIR",
            semver_build_dir(repo_root).to_string_lossy().into_owned(),
        ),
    ]))
}

pub(crate) fn collect_shell_script_paths<I>(entries: I) -> DynResult<Vec<PathBuf>>
where
    I: IntoIterator<Item = io::Result<PathBuf>>,
{
    let mut scripts = Vec::new();
    for entry in entries {
        let path = entry?;
        if path.extension() == Some(OsStr::new("sh")) {
            scripts.push(path);
        }
    }
    scripts.sort();
    Ok(scripts)
}

/// Returns whether one command spec is the semver gate step.
pub(crate) fn is_semver_check_spec(spec: &CommandSpec) -> bool {
    spec.program == Path::new("cargo")
        && matches!(spec.args.first().map(String::as_str), Some("semver-checks"))
}

fn path_strings(paths: &[PathBuf]) -> impl Iterator<Item = String> + '_ {
    paths.iter().map(|path| path.to_string_lossy().into_owned())
}

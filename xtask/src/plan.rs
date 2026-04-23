use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::model::{CommandSpec, DynResult};

/// Builds the ordered command plan for `cargo xtask check`.
pub(crate) fn check_plan(repo_root: &Path) -> DynResult<Vec<CommandSpec>> {
    let scripts = shell_script_paths(repo_root)?;
    let semver_release_type = semver_release_type(repo_root)?;
    let mut plan = Vec::new();

    if !scripts.is_empty() {
        plan.push(CommandSpec::new(
            "bash",
            std::iter::once("-n".to_owned()).chain(path_strings(&scripts)),
            false,
            false,
        ));
        plan.push(CommandSpec::new(
            "shellcheck",
            path_strings(&scripts),
            false,
            false,
        ));
    }

    plan.push(CommandSpec::new("cargo", ["fmt", "--check"], false, false));
    plan.push(CommandSpec::new(
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
        true,
    ));
    plan.push(CommandSpec::new(
        "cargo",
        [
            "outdated",
            "--workspace",
            "--root-deps-only",
            "--exit-code",
            "1",
        ],
        false,
        false,
    ));
    plan.push(CommandSpec::new(
        "cargo",
        [
            "outdated",
            "--manifest-path",
            fuzz_manifest_path(repo_root).to_string_lossy().as_ref(),
            "--root-deps-only",
            "--exit-code",
            "1",
        ],
        false,
        false,
    ));
    plan.push(CommandSpec::new(
        "cargo",
        ["audit", "-D", "warnings"],
        false,
        false,
    ));
    plan.push(CommandSpec::new(
        "cargo",
        [
            "audit",
            "--file",
            fuzz_lockfile_path(repo_root).to_string_lossy().as_ref(),
            "-D",
            "warnings",
        ],
        false,
        false,
    ));
    plan.push(CommandSpec::new(
        "cargo",
        ["deny", "check", "advisories", "bans", "licenses", "sources"],
        false,
        false,
    ));
    plan.push(
        CommandSpec::new(
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
            true,
        )
        .with_envs([(
            "CARGO_TARGET_DIR",
            semver_scratch_dir(repo_root).to_string_lossy().into_owned(),
        )]),
    );
    plan.push(CommandSpec::new(
        "cargo",
        [
            "check",
            "--manifest-path",
            fuzz_manifest_path(repo_root).to_string_lossy().as_ref(),
            "--bins",
            "--locked",
        ],
        false,
        true,
    ));
    plan.push(CommandSpec::new(
        "cargo",
        [
            "nextest",
            "run",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
        ],
        false,
        true,
    ));
    plan.push(CommandSpec::new(
        "cargo",
        ["test", "--workspace", "--doc", "--all-features", "--locked"],
        false,
        true,
    ));
    plan.push(CommandSpec::new(
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
        true,
    ));
    plan.push(CommandSpec::new(
        release_binary_path(repo_root),
        ["--version"],
        true,
        false,
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

/// Reads the workspace version from the root manifest.
pub(crate) fn workspace_version(repo_root: &Path) -> DynResult<String> {
    workspace_version_from_manifest(&fs::read_to_string(repo_root.join("Cargo.toml"))?)
}

/// Maps the local Git tag state to the release type used by semver-checks.
pub(crate) fn semver_release_type(repo_root: &Path) -> DynResult<String> {
    let version = workspace_version(repo_root)?;
    let has_release_tag = release_tag_exists(repo_root, &version)?;
    Ok(semver_release_type_from_git_tag(has_release_tag))
}

/// Extracts the workspace version from a root manifest string.
pub(crate) fn workspace_version_from_manifest(manifest: &str) -> DynResult<String> {
    toml::from_str::<toml::Value>(manifest)?
        .get("workspace")
        .and_then(toml::Value::as_table)
        .and_then(|workspace| workspace.get("package"))
        .and_then(toml::Value::as_table)
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| "workspace version not found in Cargo.toml".into())
}

/// Determines whether the current workspace version already exists as a local Git tag.
pub(crate) fn release_tag_exists(repo_root: &Path, version: &str) -> DynResult<bool> {
    if !repo_root.join(".git").exists() {
        return Ok(false);
    }

    let status = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("rev-parse")
        .arg("-q")
        .arg("--verify")
        .arg(format!("refs/tags/v{version}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    Ok(status.success())
}

/// Determines the release type semver-checks should enforce.
pub(crate) fn semver_release_type_from_git_tag(has_release_tag: bool) -> String {
    if has_release_tag {
        "minor".to_owned()
    } else {
        "major".to_owned()
    }
}

/// Adds a workspace stub carrying inherited package, dependency, and lint truth.
pub(crate) fn with_workspace_stub(cargo_toml: &str, workspace_manifest: &str) -> DynResult<String> {
    if cargo_toml.contains("\n[workspace]\n") {
        return Ok(cargo_toml.to_owned());
    }

    let workspace_manifest = toml::from_str::<toml::Value>(workspace_manifest)?;
    let workspace_table = workspace_manifest
        .get("workspace")
        .and_then(toml::Value::as_table)
        .ok_or("workspace table not found in Cargo.toml")?;

    let mut stub_workspace = toml::map::Map::new();
    for key in ["package", "dependencies", "lints"] {
        if let Some(value) = workspace_table.get(key) {
            stub_workspace.insert(key.to_owned(), value.clone());
        }
    }

    let mut root = toml::map::Map::new();
    root.insert("workspace".to_owned(), toml::Value::Table(stub_workspace));
    let workspace_stub = toml::to_string(&toml::Value::Table(root))?;
    Ok(format!("{cargo_toml}\n{workspace_stub}"))
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

/// Returns whether one command spec is the semver gate step.
pub(crate) fn is_semver_check_spec(spec: &CommandSpec) -> bool {
    spec.program == Path::new("cargo")
        && matches!(spec.args.first().map(String::as_str), Some("semver-checks"))
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

fn path_strings(paths: &[PathBuf]) -> impl Iterator<Item = String> + '_ {
    paths.iter().map(|path| path.to_string_lossy().into_owned())
}

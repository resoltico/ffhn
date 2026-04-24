use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::model::DynResult;

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

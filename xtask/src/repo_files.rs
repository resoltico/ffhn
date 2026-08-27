use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::model::DynResult;

#[cfg(test)]
const ROOT_PUBLIC_MARKDOWN: &[&str] = &["README.md", "CONTRIBUTING.md", "changelog.md"];
#[cfg(test)]
const AFAD_MANAGED_MARKDOWN_ROOTS: &[&str] = &["docs", "fuzz"];
#[cfg(test)]
const ROOT_MAINTAINED_REPO_FILES: &[&str] = &[
    "AGENTS.md",
    "Cargo.toml",
    "Cargo.lock",
    "README.md",
    "CONTRIBUTING.md",
    "changelog.md",
    "check.sh",
    "rust-toolchain.toml",
];
#[cfg(test)]
const MAINTAINED_REPO_OWNED_FILE_ROOTS: &[&str] = &[
    ".codex",
    ".devcontainer",
    ".github/workflows",
    "crates/ffhn-cli/src",
    "crates/ffhn-core/src",
    "docs",
    "fuzz",
    "scripts",
    "xtask/src",
];
// Coverage instruments the workspace crates only; the standalone fuzz package has its own cargo
// invocation and is intentionally outside that measured set.
const COVERED_RUST_SOURCE_ROOTS: &[&str] =
    &["crates/ffhn-core/src", "crates/ffhn-cli/src", "xtask/src"];
// Source-shape governance covers every maintained Rust compilation unit, including the standalone
// fuzz harnesses that are compiled through `fuzz/Cargo.toml`.
const SOURCE_SHAPE_RUST_SOURCE_ROOTS: &[&str] = &[
    "crates/ffhn-core/src",
    "crates/ffhn-cli/src",
    "xtask/src",
    "fuzz/fuzz_targets",
];
const MAINTAINED_RUST_TEST_ROOTS: &[&str] = &["crates/ffhn-cli/tests", "xtask/tests"];

#[cfg(test)]
pub(crate) fn public_markdown_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for root_markdown in ROOT_PUBLIC_MARKDOWN {
        let path = repo_root.join(root_markdown);
        if path.is_file() {
            paths.push(path);
        }
    }

    paths.extend(afad_managed_markdown_paths(repo_root)?);
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
pub(crate) fn afad_managed_markdown_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for relative_root in AFAD_MANAGED_MARKDOWN_ROOTS {
        let directory = repo_root.join(relative_root);
        if directory.is_dir() {
            collect_markdown_paths(&directory, &mut paths)?;
        }
    }

    paths.sort();
    Ok(paths)
}

#[cfg(test)]
pub(crate) fn maintained_repo_owned_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for root_file in ROOT_MAINTAINED_REPO_FILES {
        let path = repo_root.join(root_file);
        if path.is_file() {
            paths.push(path);
        }
    }

    for relative_root in MAINTAINED_REPO_OWNED_FILE_ROOTS {
        let directory = repo_root.join(relative_root);
        if directory.is_dir() {
            collect_regular_files(&directory, &mut paths)?;
        }
    }

    paths.sort();
    paths.dedup();
    Ok(paths)
}

#[cfg(test)]
pub(crate) fn maintained_rust_source_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    Ok(maintained_rust_source_entries(repo_root)?
        .into_iter()
        .map(|(path, _)| path)
        .collect())
}

pub(crate) fn maintained_rust_source_entries(
    repo_root: &Path,
) -> DynResult<Vec<(PathBuf, String)>> {
    let mut entries = Vec::new();

    for relative_root in COVERED_RUST_SOURCE_ROOTS {
        let directory = repo_root.join(relative_root);
        if directory.is_dir() {
            collect_rust_source_entries(repo_root, &directory, &mut entries)?;
        }
    }

    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(entries)
}

/// Returns every maintained Rust source file, including unit and integration test modules.
pub(crate) fn maintained_rust_source_entries_including_tests(
    repo_root: &Path,
) -> DynResult<Vec<(PathBuf, String)>> {
    let mut entries = Vec::new();

    for relative_root in SOURCE_SHAPE_RUST_SOURCE_ROOTS
        .iter()
        .chain(MAINTAINED_RUST_TEST_ROOTS)
    {
        let directory = repo_root.join(relative_root);
        if directory.is_dir() {
            collect_all_rust_source_entries(repo_root, &directory, &mut entries)?;
        }
    }

    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries.dedup_by(|left, right| left.0 == right.0);
    Ok(entries)
}

fn collect_rust_source_entries(
    repo_root: &Path,
    directory: &Path,
    entries: &mut Vec<(PathBuf, String)>,
) -> DynResult<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_entries(repo_root, &path, entries)?;
        } else if path.extension() == Some(OsStr::new("rs"))
            && !path
                .components()
                .any(|component| component.as_os_str() == OsStr::new("tests"))
            && path.file_name() != Some(OsStr::new("tests.rs"))
        {
            let relative_path = workspace_relative_path(repo_root, &path)?;
            entries.push((path, relative_path));
        }
    }

    Ok(())
}

fn collect_all_rust_source_entries(
    repo_root: &Path,
    directory: &Path,
    entries: &mut Vec<(PathBuf, String)>,
) -> DynResult<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_all_rust_source_entries(repo_root, &path, entries)?;
        } else if path.extension() == Some(OsStr::new("rs")) {
            let relative_path = workspace_relative_path(repo_root, &path)?;
            entries.push((path, relative_path));
        }
    }

    Ok(())
}

/// Produces the canonical, platform-neutral path spelling used by repository policy.
///
/// The filesystem uses `\\` as a separator on Windows, while the source-shape policy is a
/// repository contract and therefore always uses `/`. Building the representation from path
/// components instead of replacing characters preserves a literal backslash if one appears in a
/// Unix filename, so the fail-closed policy check reports that anomalous name rather than
/// silently reinterpreting it as a separator.
fn workspace_relative_path(repo_root: &Path, path: &Path) -> DynResult<String> {
    let relative_path = path
        .strip_prefix(repo_root)
        .expect("repo file discovery only walks inside the workspace root");

    canonical_workspace_relative_path(relative_path)
}

fn canonical_workspace_relative_path(relative_path: &Path) -> DynResult<String> {
    let mut components = Vec::new();
    for component in relative_path.components() {
        match component {
            Component::Normal(component) => components.push(component.to_string_lossy()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "repository file discovery produced a non-normal workspace-relative path: {}",
                    relative_path.display()
                )
                .into());
            }
        }
    }

    if components.is_empty() {
        return Err("repository file discovery produced an empty workspace-relative path".into());
    }

    Ok(components.join("/"))
}

#[cfg(test)]
fn collect_markdown_paths(directory: &Path, paths: &mut Vec<PathBuf>) -> DynResult<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_paths(&path, paths)?;
        } else if path.extension() == Some(OsStr::new("md")) {
            paths.push(path);
        }
    }

    Ok(())
}

#[cfg(test)]
fn collect_regular_files(directory: &Path, paths: &mut Vec<PathBuf>) -> DynResult<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if !matches!(
                path.file_name().and_then(OsStr::to_str),
                Some("target" | "dist" | "lock" | "snapshots")
            ) {
                collect_regular_files(&path, paths)?;
            }
        } else if path.is_file() {
            let ignored_metadata_file = path
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| name == ".DS_Store" || name.starts_with("._"));
            if ignored_metadata_file {
                continue;
            }
            paths.push(path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;

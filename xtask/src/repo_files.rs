use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::DynResult;

#[cfg(test)]
const ROOT_PUBLIC_MARKDOWN: &[&str] = &["README.md", "CONTRIBUTING.md", "changelog.md"];
#[cfg(test)]
const AFAD_MANAGED_MARKDOWN_ROOTS: &[&str] = &["docs", "examples", "fuzz"];
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
    "examples",
    "fuzz",
    "scripts",
    "xtask/src",
];
const MAINTAINED_RUST_SOURCE_ROOTS: &[&str] =
    &["crates/ffhn-core/src", "crates/ffhn-cli/src", "xtask/src"];

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

    paths.extend(watchlist_target_config_paths(repo_root)?);
    paths.sort();
    paths.dedup();
    Ok(paths)
}

#[cfg(test)]
pub(crate) fn watchlist_target_config_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let watchlist_dir = repo_root.join("watchlist");

    if watchlist_dir.is_dir() {
        for entry in fs::read_dir(&watchlist_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let target_file = path.join("target.toml");
                if target_file.is_file() {
                    paths.push(target_file);
                }
            }
        }
    }

    paths.sort();
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

    for relative_root in MAINTAINED_RUST_SOURCE_ROOTS {
        let directory = repo_root.join(relative_root);
        if directory.is_dir() {
            collect_rust_source_entries(repo_root, &directory, &mut entries)?;
        }
    }

    entries.sort_by(|left, right| left.0.cmp(&right.0));
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
            let relative_path = path
                .strip_prefix(repo_root)
                .expect("repo file discovery only walks inside the workspace root")
                .to_string_lossy()
                .into_owned();
            entries.push((path, relative_path));
        }
    }

    Ok(())
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

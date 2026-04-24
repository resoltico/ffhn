use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::DynResult;

#[cfg(test)]
const ROOT_PUBLIC_MARKDOWN: &[&str] = &["README.md", "CONTRIBUTING.md", "changelog.md"];
#[cfg(test)]
const AFAD_MANAGED_MARKDOWN_ROOTS: &[&str] = &["docs", "examples", "fuzz"];
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

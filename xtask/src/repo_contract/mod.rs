use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::DynResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AfadFrontmatter {
    pub(crate) afad: String,
    pub(crate) version: String,
}

pub(crate) fn public_markdown_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for file in ["README.md", "CONTRIBUTING.md", "changelog.md"] {
        let path = repo_root.join(file);
        if path.is_file() {
            paths.push(path);
        }
    }

    for directory in [
        repo_root.join("docs"),
        repo_root.join("examples"),
        repo_root.join("fuzz"),
    ] {
        if directory.is_dir() {
            collect_markdown_paths(&directory, &mut paths)?;
        }
    }

    paths.sort();
    Ok(paths)
}

pub(crate) fn afad_frontmatter(path: &Path) -> DynResult<Option<AfadFrontmatter>> {
    let text = fs::read_to_string(path)?;
    parse_afad_frontmatter(&text)
        .map_err(|error| format!("{} has invalid AFAD frontmatter: {error}", path.display()).into())
}

pub(crate) fn protocol_afad_version(repo_root: &Path) -> DynResult<String> {
    let path = repo_root.join(".codex/PROTOCOL_AFAD.md");
    let text = fs::read_to_string(&path)?;

    parse_protocol_afad_version(&text).map_err(|error| {
        format!("{} has invalid protocol metadata: {error}", path.display()).into()
    })
}

pub(crate) fn public_target_example_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    let mut paths = Vec::new();

    let examples_dir = repo_root.join("examples");
    if examples_dir.is_dir() {
        for entry in fs::read_dir(&examples_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension() == Some(OsStr::new("toml")) {
                paths.push(path);
            }
        }
    }

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

pub(crate) fn user_facing_source_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for directory in [
        repo_root.join("crates/ffhn-core/src"),
        repo_root.join("crates/ffhn-cli/src"),
        repo_root.join("xtask/src"),
    ] {
        if directory.is_dir() {
            collect_rust_source_paths(&directory, &mut paths)?;
        }
    }

    paths.sort();
    Ok(paths)
}

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

fn collect_rust_source_paths(directory: &Path, paths: &mut Vec<PathBuf>) -> DynResult<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_paths(&path, paths)?;
        } else if path.extension() == Some(OsStr::new("rs"))
            && !path
                .components()
                .any(|component| component.as_os_str() == OsStr::new("tests"))
            && path.file_name() != Some(OsStr::new("tests.rs"))
        {
            paths.push(path);
        }
    }

    Ok(())
}

fn parse_afad_frontmatter(text: &str) -> Result<Option<AfadFrontmatter>, String> {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return Ok(None);
    }

    let mut afad = None;
    let mut version = None;
    let mut closed = false;

    for line in lines {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        if let Some(value) = frontmatter_value(line, "afad") {
            afad = Some(value);
        }
        if let Some(value) = frontmatter_value(line, "version") {
            version = Some(value);
        }
    }

    if afad.is_none() && version.is_none() {
        return Ok(None);
    }
    if !closed {
        return Err("frontmatter block is not terminated".to_owned());
    }

    Ok(Some(AfadFrontmatter {
        afad: afad.ok_or_else(|| "missing afad field".to_owned())?,
        version: version.ok_or_else(|| "missing version field".to_owned())?,
    }))
}

fn parse_protocol_afad_version(text: &str) -> Result<String, String> {
    for line in text.lines() {
        if let Some(version) = line.trim().strip_prefix("VERSION:") {
            let version = version.trim();
            if version.is_empty() {
                return Err("VERSION line is empty".to_owned());
            }
            return Ok(version.to_owned());
        }
    }

    Err("missing VERSION line".to_owned())
}

fn frontmatter_value(line: &str, key: &str) -> Option<String> {
    let trimmed = line.trim();
    let value = trimmed.strip_prefix(&format!("{key}:"))?.trim();
    Some(value.trim_matches('"').to_owned())
}

#[cfg(test)]
mod tests;

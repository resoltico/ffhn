use std::fs;
use std::path::{Path, PathBuf};

use crate::model::DynResult;
use crate::repo_files::{
    afad_managed_markdown_paths as repo_afad_managed_markdown_paths, maintained_rust_source_paths,
    public_markdown_paths as repo_public_markdown_paths,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AfadFrontmatter {
    pub(crate) afad: String,
}

pub(crate) fn public_markdown_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    repo_public_markdown_paths(repo_root)
}

pub(crate) fn afad_managed_markdown_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    repo_afad_managed_markdown_paths(repo_root)
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

pub(crate) fn user_facing_source_paths(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    maintained_rust_source_paths(repo_root)
}

fn parse_afad_frontmatter(text: &str) -> Result<Option<AfadFrontmatter>, String> {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return Ok(None);
    }

    let mut afad = None;
    let mut saw_version = false;
    let mut closed = false;

    for line in lines {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        if let Some(value) = frontmatter_value(line, "afad") {
            afad = Some(value);
        }
        if frontmatter_value(line, "version").is_some() {
            saw_version = true;
        }
    }

    if afad.is_none() && !saw_version {
        return Ok(None);
    }
    if !closed {
        return Err("frontmatter block is not terminated".to_owned());
    }
    if saw_version {
        return Err(
            "version field is not allowed in AFAD frontmatter; Cargo.toml is the canonical release version"
                .to_owned(),
        );
    }

    Ok(Some(AfadFrontmatter {
        afad: afad.ok_or_else(|| "missing afad field".to_owned())?,
    }))
}

fn parse_protocol_afad_version(text: &str) -> Result<String, String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(version) = trimmed.strip_prefix("**Version:**") {
            let version = version.trim().trim_matches('`').trim_matches('"');
            if version.is_empty() {
                return Err("**Version:** line is empty".to_owned());
            }
            return Ok(version.to_owned());
        }
    }

    Err("missing **Version:** line".to_owned())
}

fn frontmatter_value(line: &str, key: &str) -> Option<String> {
    let trimmed = line.trim();
    let value = trimmed.strip_prefix(&format!("{key}:"))?.trim();
    Some(value.trim_matches('"').to_owned())
}

#[cfg(test)]
mod tests;

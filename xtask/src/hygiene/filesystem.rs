//! Filesystem traversal, artifact-root recognition, and byte formatting.

use std::fs;
use std::path::{Path, PathBuf};

use crate::model::DynResult;

use super::types::{ARTIFACT_MANIFEST_NAME, CACHEDIR_TAG_NAME, GIB, HygieneEntry, MIB};

pub(super) fn repo_tmp_cargo_roots(repo_root: &Path) -> DynResult<Vec<PathBuf>> {
    let tmp_root = repo_root.join("tmp");
    if !tmp_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut roots = fs::read_dir(&tmp_root)
        .map_err(|error| {
            format!(
                "failed to inspect repository temporary root {}: {error}",
                tmp_root.display()
            )
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| looks_like_cargo_target_dir(path))
        .collect::<Vec<_>>();
    roots.sort();
    Ok(roots)
}

pub(super) fn looks_like_cargo_target_dir(path: &Path) -> bool {
    [
        ".fingerprint",
        ".rustc_info.json",
        "debug",
        "release",
        "dist",
        "package",
        "CACHEDIR.TAG",
    ]
    .iter()
    .any(|component| path.join(component).exists())
}

pub(super) fn dir_size_bytes(path: &Path) -> DynResult<u64> {
    dir_size_bytes_excluding_roots(path, &[])
}

pub(super) fn dir_size_bytes_excluding_roots(
    path: &Path,
    skipped_roots: &[PathBuf],
) -> DynResult<u64> {
    if skipped_roots.iter().any(|root| path == root.as_path()) {
        return Ok(0);
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => {
            return Err(format!(
                "failed to read hygiene metadata {}: {error}",
                path.display()
            )
            .into());
        }
    };
    if metadata.file_type().is_symlink() {
        return Ok(0);
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Ok(0);
    }

    let entries = fs::read_dir(path).map_err(|error| {
        format!(
            "failed to read hygiene directory {}: {error}",
            path.display()
        )
    })?;
    entries.into_iter().try_fold(0u64, |total, entry| {
        let entry = entry?;
        dir_size_bytes_excluding_roots(&entry.path(), skipped_roots)
            .map(|entry_bytes| total + entry_bytes)
    })
}

pub(super) fn deduplicate_root_set(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort();
    paths.dedup();
    let mut pruned = Vec::new();

    'candidate: for path in paths {
        for existing in &pruned {
            if path.starts_with(existing) {
                continue 'candidate;
            }
        }
        pruned.retain(|existing: &PathBuf| !existing.starts_with(&path));
        pruned.push(path);
    }

    pruned
}

pub(super) fn missing_managed_markers(path: &Path) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if !path.join(CACHEDIR_TAG_NAME).is_file() {
        missing.push(CACHEDIR_TAG_NAME);
    }
    if !path.join(ARTIFACT_MANIFEST_NAME).is_file() {
        missing.push(ARTIFACT_MANIFEST_NAME);
    }
    missing
}

pub(super) fn missing_managed_markers_for_entry(entry: &HygieneEntry) -> Vec<String> {
    let mut missing = missing_managed_markers(Path::new(&entry.path))
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();

    if entry.id == "managed-coverage-target" || entry.id == "managed-coverage-build" {
        let nested_path = Path::new(&entry.path).join("llvm-cov-target");
        missing.extend(
            missing_managed_markers(&nested_path)
                .into_iter()
                .map(|marker| format!("llvm-cov-target/{marker}")),
        );
    }

    missing
}

pub(super) fn format_bytes(bytes: u64) -> String {
    if bytes >= GIB {
        return format!("{:.1} GiB", bytes as f64 / GIB as f64);
    }
    if bytes >= MIB {
        return format!("{:.1} MiB", bytes as f64 / MIB as f64);
    }
    if bytes >= 1024 {
        return format!("{:.1} KiB", bytes as f64 / 1024.0);
    }

    format!("{bytes} B")
}

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::canonical::normalize_line_endings;
use crate::stable_json::sha256_hex;
use crate::{CoreError, RelativeArtifactPath, SnapshotReference, SnapshotSlot, TargetPaths};

use super::super::state::SnapshotArtifacts;
use super::super::storage::{now_utc, write_exact_text};

static SNAPSHOT_WORK_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug)]
pub(super) struct StagedHistorySnapshot {
    pub(super) reference: SnapshotReference,
    pub(super) staged_dir: PathBuf,
}

fn relative_artifact_path(path: impl Into<String>) -> RelativeArtifactPath {
    RelativeArtifactPath::new(path).expect("hard-coded FFHN artifact paths must be valid")
}

pub(super) fn clear_dir_if_exists(path: &Path) -> Result<(), CoreError> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|error| CoreError::io(path, error))?;
    }
    Ok(())
}

pub(super) fn current_snapshot_reference(
    canonical_text: &str,
    outer_html: &str,
    captured_at: &str,
) -> SnapshotReference {
    SnapshotReference {
        slot: SnapshotSlot::Current,
        canonical_text_sha256: sha256_hex(canonical_text.as_bytes()),
        outer_html_sha256: sha256_hex(outer_html.as_bytes()),
        extraction_record_path: relative_artifact_path("snapshots/current/extraction.json"),
        canonical_text_path: relative_artifact_path("snapshots/current/canonical.txt"),
        outer_html_path: relative_artifact_path("snapshots/current/outer.html"),
        captured_at: captured_at.to_owned(),
    }
}

pub(super) fn stage_current_snapshot(
    paths: &TargetPaths,
    canonical_text: &str,
    outer_html: &str,
    extraction_json: &str,
) -> Result<(PathBuf, SnapshotReference), CoreError> {
    let captured_at = now_utc()?;
    let staged_dir = unique_snapshot_work_dir(paths, "current-stage");
    write_snapshot_dir(&staged_dir, canonical_text, outer_html, extraction_json)?;
    Ok((
        staged_dir,
        current_snapshot_reference(canonical_text, outer_html, &captured_at),
    ))
}

pub(super) fn stage_history_snapshot(
    paths: &TargetPaths,
    current: &SnapshotArtifacts,
) -> Result<StagedHistorySnapshot, CoreError> {
    let snapshot_key = history_snapshot_key(&current.reference);
    let staged_dir = unique_snapshot_work_dir(paths, "history-stage");
    write_snapshot_artifacts(&staged_dir, current)?;
    let reference = SnapshotReference {
        slot: SnapshotSlot::History,
        canonical_text_sha256: current.reference.canonical_text_sha256.clone(),
        outer_html_sha256: current.reference.outer_html_sha256.clone(),
        extraction_record_path: relative_artifact_path(format!(
            "snapshots/history/{snapshot_key}/extraction.json"
        )),
        canonical_text_path: relative_artifact_path(format!(
            "snapshots/history/{snapshot_key}/canonical.txt"
        )),
        outer_html_path: relative_artifact_path(format!(
            "snapshots/history/{snapshot_key}/outer.html"
        )),
        captured_at: current.reference.captured_at.clone(),
    };
    Ok(StagedHistorySnapshot {
        reference,
        staged_dir,
    })
}

pub(super) fn snapshot_reference_dir(target_dir: &Path, reference: &SnapshotReference) -> PathBuf {
    let relative_dir = reference
        .canonical_text_path
        .as_path()
        .parent()
        .unwrap_or_else(|| Path::new(""));
    target_dir.join(relative_dir)
}

fn write_snapshot_artifacts(dir: &Path, snapshot: &SnapshotArtifacts) -> Result<(), CoreError> {
    write_snapshot_dir(
        dir,
        &snapshot.canonical_text,
        &snapshot.outer_html,
        &snapshot.extraction_json,
    )
}

pub(super) fn write_snapshot_dir(
    dir: &Path,
    canonical_text: &str,
    outer_html: &str,
    extraction_json: &str,
) -> Result<(), CoreError> {
    fs::create_dir_all(dir).map_err(|error| CoreError::io(dir, error))?;
    let canonical_text = normalize_line_endings(canonical_text);
    let outer_html = normalize_line_endings(outer_html);
    write_exact_text(dir.join("canonical.txt"), &canonical_text)?;
    write_exact_text(dir.join("outer.html"), &outer_html)?;
    write_exact_text(dir.join("extraction.json"), extraction_json)?;
    Ok(())
}

fn history_snapshot_key(reference: &SnapshotReference) -> String {
    let compact_time = reference
        .captured_at
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    format!("{compact_time}-{}", &reference.canonical_text_sha256[..12])
}

pub(super) fn unique_snapshot_work_dir(paths: &TargetPaths, prefix: &str) -> PathBuf {
    let snapshots_dir = paths.snapshots_dir();
    let process_id = std::process::id();
    loop {
        let suffix = SNAPSHOT_WORK_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = snapshots_dir.join(format!(".{prefix}-{process_id}-{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }
}

use std::fs;
use std::path::Path;

use crate::canonical::normalize_line_endings;
use crate::stable_json::sha256_hex;
use crate::{CoreError, SnapshotReference, SnapshotSlot, TargetPaths};

use super::super::state::SnapshotArtifacts;
use super::super::storage::{now_utc, write_exact_text};

pub(super) fn clear_dir_if_exists(path: &Path) -> Result<(), CoreError> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|error| CoreError::io(path, error))?;
    }
    Ok(())
}

pub(super) fn write_new_current_snapshot(
    paths: &TargetPaths,
    canonical_text: &str,
    outer_html: &str,
    extraction_json: &str,
) -> Result<SnapshotReference, CoreError> {
    let captured_at = now_utc()?;
    write_current_snapshot(
        &paths.current_snapshot_dir(),
        canonical_text,
        outer_html,
        extraction_json,
    )?;
    Ok(SnapshotReference {
        slot: SnapshotSlot::Current,
        canonical_text_sha256: sha256_hex(canonical_text.as_bytes()),
        outer_html_sha256: sha256_hex(outer_html.as_bytes()),
        extraction_record_path: "snapshots/current/extraction.json".to_owned(),
        canonical_text_path: "snapshots/current/canonical.txt".to_owned(),
        outer_html_path: "snapshots/current/outer.html".to_owned(),
        captured_at,
    })
}

pub(super) fn archive_current_snapshot(
    paths: &TargetPaths,
    current: &SnapshotArtifacts,
) -> Result<SnapshotReference, CoreError> {
    let snapshot_key = history_snapshot_key(&current.reference);
    let history_dir = paths.history_snapshot_dir(&snapshot_key);
    write_snapshot_artifacts(&history_dir, current)?;
    Ok(SnapshotReference {
        slot: SnapshotSlot::History,
        canonical_text_sha256: current.reference.canonical_text_sha256.clone(),
        outer_html_sha256: current.reference.outer_html_sha256.clone(),
        extraction_record_path: format!("snapshots/history/{snapshot_key}/extraction.json"),
        canonical_text_path: format!("snapshots/history/{snapshot_key}/canonical.txt"),
        outer_html_path: format!("snapshots/history/{snapshot_key}/outer.html"),
        captured_at: current.reference.captured_at.clone(),
    })
}

pub(super) fn prune_history(
    paths: &TargetPaths,
    snapshot_history: &mut Vec<SnapshotReference>,
    history_limit: usize,
) -> Result<(), CoreError> {
    let max_history_entries = history_limit.saturating_sub(1);
    let drain_from = max_history_entries.min(snapshot_history.len());
    for removed in snapshot_history.drain(drain_from..) {
        let history_path = paths.target_dir().join(
            removed
                .canonical_text_path
                .split('/')
                .take(3)
                .collect::<Vec<_>>()
                .join("/"),
        );
        clear_dir_if_exists(&history_path)?;
    }
    Ok(())
}

fn write_current_snapshot(
    dir: &Path,
    canonical_text: &str,
    outer_html: &str,
    extraction_json: &str,
) -> Result<(), CoreError> {
    write_snapshot_dir(dir, canonical_text, outer_html, extraction_json)
}

fn write_snapshot_artifacts(dir: &Path, snapshot: &SnapshotArtifacts) -> Result<(), CoreError> {
    write_snapshot_dir(
        dir,
        &snapshot.canonical_text,
        &snapshot.outer_html,
        &snapshot.extraction_json,
    )
}

fn write_snapshot_dir(
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

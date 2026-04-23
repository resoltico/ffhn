use std::fs;
use std::path::PathBuf;

use crate::{CoreError, SnapshotReference, StateDocument, TargetPaths};

use super::super::storage::write_json;
use super::snapshot_store::{
    clear_dir_if_exists, snapshot_reference_dir, unique_snapshot_work_dir,
};

pub(super) struct SnapshotPersistPlan {
    pub(super) current_snapshot: Option<SnapshotReference>,
    pub(super) snapshot_history: Vec<SnapshotReference>,
    staged_current_dir: Option<PathBuf>,
    archived_snapshot: Option<SnapshotReference>,
    pruned_snapshots: Vec<SnapshotReference>,
    clear_history_after_success: bool,
}

impl SnapshotPersistPlan {
    pub(super) fn unchanged(
        current_snapshot: Option<SnapshotReference>,
        snapshot_history: Vec<SnapshotReference>,
    ) -> Self {
        Self {
            current_snapshot,
            snapshot_history,
            staged_current_dir: None,
            archived_snapshot: None,
            pruned_snapshots: Vec::new(),
            clear_history_after_success: false,
        }
    }

    pub(super) fn with_staged_current(
        current_snapshot: SnapshotReference,
        snapshot_history: Vec<SnapshotReference>,
        staged_current_dir: PathBuf,
        archived_snapshot: Option<SnapshotReference>,
        pruned_snapshots: Vec<SnapshotReference>,
        clear_history_after_success: bool,
    ) -> Self {
        Self {
            current_snapshot: Some(current_snapshot),
            snapshot_history,
            staged_current_dir: Some(staged_current_dir),
            archived_snapshot,
            pruned_snapshots,
            clear_history_after_success,
        }
    }

    pub(super) fn commit(
        self,
        paths: &TargetPaths,
        state: &StateDocument,
    ) -> Result<(), CoreError> {
        let Some(staged_current_dir) = self.staged_current_dir.as_ref() else {
            write_json(paths.state_file(), state)?;
            self.cleanup_after_success(paths, None);
            return Ok(());
        };

        let backup_current_dir = swap_in_staged_current(paths, staged_current_dir)?;
        match write_json(paths.state_file(), state) {
            Ok(()) => {
                self.cleanup_after_success(paths, backup_current_dir.as_ref());
                Ok(())
            }
            Err(error) => {
                rollback_current_swap(paths, backup_current_dir.as_ref())?;
                self.cleanup_after_rollback(paths);
                Err(error)
            }
        }
    }

    fn cleanup_after_success(&self, paths: &TargetPaths, backup_current_dir: Option<&PathBuf>) {
        if let Some(backup_current_dir) = backup_current_dir {
            let _ = clear_dir_if_exists(backup_current_dir);
        }
        for snapshot in &self.pruned_snapshots {
            let _ = clear_dir_if_exists(&snapshot_reference_dir(&paths.target_dir(), snapshot));
        }
        if self.clear_history_after_success {
            let _ = clear_dir_if_exists(&paths.history_snapshots_dir());
        }
    }

    fn cleanup_after_rollback(&self, paths: &TargetPaths) {
        if let Some(archived_snapshot) = &self.archived_snapshot {
            let _ = clear_dir_if_exists(&snapshot_reference_dir(
                &paths.target_dir(),
                archived_snapshot,
            ));
        }
        if let Some(staged_current_dir) = &self.staged_current_dir {
            let _ = clear_dir_if_exists(staged_current_dir);
        }
    }
}

fn swap_in_staged_current(
    paths: &TargetPaths,
    staged_current_dir: &PathBuf,
) -> Result<Option<PathBuf>, CoreError> {
    fs::create_dir_all(paths.snapshots_dir())
        .map_err(|error| CoreError::io(paths.snapshots_dir(), error))?;

    let current_dir = paths.current_snapshot_dir();
    let backup_current_dir = current_dir
        .exists()
        .then(|| unique_snapshot_work_dir(paths, "current-backup"));

    if let Some(backup_current_dir) = &backup_current_dir {
        fs::rename(&current_dir, backup_current_dir)
            .map_err(|error| CoreError::io(&current_dir, error))?;
    }

    if let Err(error) = fs::rename(staged_current_dir, &current_dir) {
        if let Some(backup_current_dir) = &backup_current_dir {
            let _ = fs::rename(backup_current_dir, &current_dir);
        }
        return Err(CoreError::io(staged_current_dir, error));
    }

    Ok(backup_current_dir)
}

fn rollback_current_swap(
    paths: &TargetPaths,
    backup_current_dir: Option<&PathBuf>,
) -> Result<(), CoreError> {
    let current_dir = paths.current_snapshot_dir();
    clear_dir_if_exists(&current_dir)?;
    if let Some(backup_current_dir) = backup_current_dir {
        fs::rename(backup_current_dir, &current_dir)
            .map_err(|error| CoreError::io(backup_current_dir, error))?;
    }
    Ok(())
}

use std::fs;
use std::io;
use std::path::Path;
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

fn rename_path(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(test)]
    {
        if let Some(error) = injected_rename_error(from) {
            return Err(error);
        }
    }

    fs::rename(from, to)
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
        rename_path(&current_dir, backup_current_dir)
            .map_err(|error| CoreError::io(&current_dir, error))?;
    }

    if let Err(error) = rename_path(staged_current_dir, &current_dir) {
        if let Some(backup_current_dir) = &backup_current_dir {
            let _ = rename_path(backup_current_dir, &current_dir);
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
        rename_path(backup_current_dir, &current_dir)
            .map_err(|error| CoreError::io(backup_current_dir, error))?;
    }
    Ok(())
}

#[cfg(test)]
use std::cell::RefCell;

#[cfg(test)]
#[derive(Clone)]
struct InjectedRenameError {
    source: PathBuf,
    kind: io::ErrorKind,
}

#[cfg(test)]
thread_local! {
    static INJECTED_RENAME_ERROR: RefCell<Option<InjectedRenameError>> = const { RefCell::new(None) };
}

#[cfg(test)]
fn injected_rename_error(from: &Path) -> Option<io::Error> {
    INJECTED_RENAME_ERROR.with(|injected| {
        let mut injected = injected.borrow_mut();
        let current = injected.as_ref()?;
        if current.source != from {
            return None;
        }
        let kind = current.kind;
        *injected = None;
        Some(io::Error::new(
            kind,
            format!("injected rename failure for {}", from.display()),
        ))
    })
}

#[cfg(test)]
fn with_rename_error_injected<T>(
    source: impl Into<PathBuf>,
    kind: io::ErrorKind,
    operation: impl FnOnce() -> T,
) -> T {
    INJECTED_RENAME_ERROR.with(|injected| {
        let mut injected = injected.borrow_mut();
        assert!(injected.is_none(), "rename injection should not nest");
        *injected = Some(InjectedRenameError {
            source: source.into(),
            kind,
        });
    });
    let result = operation();
    INJECTED_RENAME_ERROR.with(|injected| *injected.borrow_mut() = None);
    result
}

#[cfg(test)]
mod tests {
    use super::super::snapshot_store::snapshot_reference_dir;
    use super::*;
    use crate::{RelativeArtifactPath, SnapshotSlot, stable_json::sha256_hex};
    use tempfile::tempdir;

    fn artifact_path(path: impl Into<String>) -> RelativeArtifactPath {
        RelativeArtifactPath::new(path).expect("relative artifact path")
    }

    fn snapshot_reference(slot: SnapshotSlot, name: &str) -> SnapshotReference {
        SnapshotReference {
            slot,
            canonical_text_sha256: sha256_hex(name.as_bytes()),
            outer_html_sha256: sha256_hex(format!("{name}-outer").as_bytes()),
            extraction_record_path: artifact_path(format!("snapshots/{name}/extraction.json")),
            canonical_text_path: artifact_path(format!("snapshots/{name}/canonical.txt")),
            outer_html_path: artifact_path(format!("snapshots/{name}/outer.html")),
            captured_at: "2026-04-24T12:00:00Z".to_owned(),
        }
    }

    #[test]
    fn cleanup_after_rollback_removes_archived_and_staged_snapshot_dirs() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        let staged_current_dir = paths.snapshots_dir().join(".current-stage-test");
        let archived_snapshot = snapshot_reference(SnapshotSlot::History, "history/archived");
        let archived_dir = snapshot_reference_dir(&paths.target_dir(), &archived_snapshot);
        fs::create_dir_all(&staged_current_dir).expect("create staged current");
        fs::create_dir_all(&archived_dir).expect("create archived snapshot");

        let plan = SnapshotPersistPlan::with_staged_current(
            snapshot_reference(SnapshotSlot::Current, "current"),
            Vec::new(),
            staged_current_dir.clone(),
            Some(archived_snapshot),
            Vec::new(),
            false,
        );

        plan.cleanup_after_rollback(&paths);

        assert!(!staged_current_dir.exists());
        assert!(!archived_dir.exists());
    }

    #[test]
    fn swap_in_staged_current_restores_the_previous_current_snapshot_when_rename_fails() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        let current_dir = paths.current_snapshot_dir();
        let staged_current_dir = paths.snapshots_dir().join(".current-stage-test");
        fs::create_dir_all(&current_dir).expect("create current dir");
        fs::create_dir_all(&staged_current_dir).expect("create staged dir");
        fs::write(current_dir.join("canonical.txt"), "before").expect("write current snapshot");
        fs::write(staged_current_dir.join("canonical.txt"), "after")
            .expect("write staged snapshot");

        let error = with_rename_error_injected(
            &staged_current_dir,
            io::ErrorKind::PermissionDenied,
            || swap_in_staged_current(&paths, &staged_current_dir),
        )
        .expect_err("staged current rename should fail");

        assert!(matches!(error, CoreError::Io { .. }));
        assert_eq!(
            fs::read_to_string(current_dir.join("canonical.txt")).expect("restored current"),
            "before"
        );
        assert!(staged_current_dir.exists());
    }

    #[test]
    fn swap_in_staged_current_preserves_staged_snapshot_when_rename_fails_without_a_backup() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        let current_dir = paths.current_snapshot_dir();
        let staged_current_dir = paths.snapshots_dir().join(".current-stage-test");
        fs::create_dir_all(&staged_current_dir).expect("create staged dir");
        fs::write(staged_current_dir.join("canonical.txt"), "after")
            .expect("write staged snapshot");

        let error = with_rename_error_injected(
            &staged_current_dir,
            io::ErrorKind::PermissionDenied,
            || swap_in_staged_current(&paths, &staged_current_dir),
        )
        .expect_err("staged current rename should fail");

        assert!(matches!(error, CoreError::Io { .. }));
        assert!(!current_dir.exists());
        assert!(staged_current_dir.exists());
    }

    #[test]
    fn rollback_current_swap_restores_backup_current_snapshots() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        let current_dir = paths.current_snapshot_dir();
        let backup_current_dir = paths.snapshots_dir().join(".current-backup-test");
        fs::create_dir_all(&current_dir).expect("create current dir");
        fs::create_dir_all(&backup_current_dir).expect("create backup dir");
        fs::write(current_dir.join("canonical.txt"), "staged").expect("write staged current");
        fs::write(backup_current_dir.join("canonical.txt"), "before").expect("write backup");

        rollback_current_swap(&paths, Some(&backup_current_dir)).expect("rollback current swap");

        assert_eq!(
            fs::read_to_string(current_dir.join("canonical.txt")).expect("restored current"),
            "before"
        );
        assert!(!backup_current_dir.exists());
    }

    #[test]
    fn cleanup_after_rollback_tolerates_missing_archived_and_staged_snapshots() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");

        SnapshotPersistPlan::unchanged(None, Vec::new()).cleanup_after_rollback(&paths);
        rollback_current_swap(&paths, None).expect("rollback without backup");
    }
}

use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::{CoreError, SnapshotReference, StateDocument, TargetPaths};

use super::super::storage::write_json;
use super::snapshot_store::{
    StagedHistorySnapshot, clear_dir_if_exists, snapshot_reference_dir, unique_snapshot_work_dir,
};

pub(super) struct SnapshotPersistPlan {
    pub(super) current_snapshot: Option<SnapshotReference>,
    pub(super) snapshot_history: Vec<SnapshotReference>,
    staged_current_dir: Option<PathBuf>,
    staged_history_snapshots: Vec<StagedHistorySnapshot>,
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
            staged_history_snapshots: Vec::new(),
            pruned_snapshots: Vec::new(),
            clear_history_after_success: false,
        }
    }

    pub(super) fn with_staged_current(
        current_snapshot: SnapshotReference,
        snapshot_history: Vec<SnapshotReference>,
        staged_current_dir: PathBuf,
        staged_history_snapshots: Vec<StagedHistorySnapshot>,
        pruned_snapshots: Vec<SnapshotReference>,
        clear_history_after_success: bool,
    ) -> Self {
        Self {
            current_snapshot: Some(current_snapshot),
            snapshot_history,
            staged_current_dir: Some(staged_current_dir),
            staged_history_snapshots,
            pruned_snapshots,
            clear_history_after_success,
        }
    }

    pub(super) fn commit(
        self,
        paths: &TargetPaths,
        state: &StateDocument,
    ) -> Result<(), CoreError> {
        let backup_current_dir = match self.staged_current_dir.as_ref() {
            Some(staged_current_dir) => Some(swap_in_staged_current(paths, staged_current_dir)?),
            None => None,
        }
        .flatten();

        if let Err(error) = publish_staged_history_snapshots(paths, &self.staged_history_snapshots)
        {
            return Err(self.rollback_failed_commit(paths, backup_current_dir.as_ref(), error));
        }

        match write_json(paths.state_file(), state) {
            Ok(()) => {
                self.cleanup_after_success(paths, backup_current_dir.as_ref());
                Ok(())
            }
            Err(error) => {
                Err(self.rollback_failed_commit(paths, backup_current_dir.as_ref(), error))
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

    fn cleanup_after_rollback(&self, paths: &TargetPaths) -> Vec<CoreError> {
        let mut errors = Vec::new();
        for staged_history_snapshot in &self.staged_history_snapshots {
            if let Err(error) = clear_dir_if_exists(&staged_history_snapshot.staged_dir) {
                errors.push(error);
            }
            if let Err(error) = clear_dir_if_exists(&snapshot_reference_dir(
                &paths.target_dir(),
                &staged_history_snapshot.reference,
            )) {
                errors.push(error);
            }
        }
        if let Some(staged_current_dir) = &self.staged_current_dir
            && let Err(error) = clear_dir_if_exists(staged_current_dir)
        {
            errors.push(error);
        }
        errors
    }

    fn rollback_failed_commit(
        &self,
        paths: &TargetPaths,
        backup_current_dir: Option<&PathBuf>,
        primary_error: CoreError,
    ) -> CoreError {
        let rollback_error = if self.staged_current_dir.is_some() {
            rollback_current_swap(paths, backup_current_dir).err()
        } else {
            None
        };
        let cleanup_errors = self.cleanup_after_rollback(paths);
        if rollback_error.is_none() && cleanup_errors.is_empty() {
            return primary_error;
        }

        CoreError::persist_transaction(primary_error, rollback_error, cleanup_errors)
    }
}

fn publish_staged_history_snapshots(
    paths: &TargetPaths,
    staged_history_snapshots: &[StagedHistorySnapshot],
) -> Result<(), CoreError> {
    for snapshot in staged_history_snapshots {
        let history_dir = snapshot_reference_dir(&paths.target_dir(), &snapshot.reference);
        let parent = history_dir
            .parent()
            .expect("snapshot history directories are always nested beneath the target root");
        fs::create_dir_all(parent).map_err(|error| CoreError::io(parent, error))?;
        rename_path(&snapshot.staged_dir, &history_dir)
            .map_err(|error| CoreError::io(&snapshot.staged_dir, error))?;
    }
    Ok(())
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
    use crate::{
        RelativeArtifactPath, STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION, SnapshotSlot, StateDocument,
        StoredBaseline, TargetId, stable_json::sha256_hex,
    };
    use tempfile::tempdir;

    fn artifact_path(path: impl Into<String>) -> RelativeArtifactPath {
        RelativeArtifactPath::new(path).expect("relative artifact path")
    }

    fn snapshot_reference(slot: SnapshotSlot, name: &str) -> SnapshotReference {
        SnapshotReference {
            slot,
            compare_digest_sha256: sha256_hex(name.as_bytes()),
            outer_html_sha256: sha256_hex(format!("{name}-outer").as_bytes()),
            extraction_record_path: artifact_path(format!("snapshots/{name}/extraction.json")),
            compare_path: artifact_path(format!("snapshots/{name}/compare.txt")),
            outer_html_path: artifact_path(format!("snapshots/{name}/outer.html")),
            captured_at: "2026-04-24T12:00:00Z".to_owned(),
        }
    }

    fn pending_state(target: &str) -> StateDocument {
        StateDocument {
            schema_name: STATE_SCHEMA_NAME.to_owned(),
            schema_version: STATE_SCHEMA_VERSION,
            target_id: TargetId::new(target).expect("target id"),
            monitoring_contract_digest_sha256:
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            baseline: StoredBaseline::Pending,
            last_run: None,
            extensions: None,
        }
    }

    fn expect_persist_transaction(
        error: CoreError,
    ) -> (
        String,
        Box<CoreError>,
        Option<Box<CoreError>>,
        Vec<CoreError>,
    ) {
        match error {
            CoreError::PersistTransaction {
                summary,
                primary,
                rollback,
                cleanup,
            } => (summary, primary, rollback, cleanup),
            other => panic!("expected composite persist transaction error, got {other}"),
        }
    }

    #[test]
    fn cleanup_after_rollback_removes_archived_and_staged_snapshot_dirs() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        let staged_current_dir = paths.snapshots_dir().join(".current-stage-test");
        let archived_snapshot = snapshot_reference(SnapshotSlot::History, "history/archived");
        let staged_history_dir = paths.snapshots_dir().join(".history-stage-test");
        let archived_dir = snapshot_reference_dir(&paths.target_dir(), &archived_snapshot);
        fs::create_dir_all(&staged_current_dir).expect("create staged current");
        fs::create_dir_all(&staged_history_dir).expect("create staged history");
        fs::create_dir_all(&archived_dir).expect("create archived snapshot");

        let plan = SnapshotPersistPlan::with_staged_current(
            snapshot_reference(SnapshotSlot::Current, "current"),
            Vec::new(),
            staged_current_dir.clone(),
            vec![StagedHistorySnapshot {
                reference: archived_snapshot,
                staged_dir: staged_history_dir.clone(),
            }],
            Vec::new(),
            false,
        );

        assert!(plan.cleanup_after_rollback(&paths).is_empty());

        assert!(!staged_current_dir.exists());
        assert!(!staged_history_dir.exists());
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
        fs::write(current_dir.join("compare.txt"), "before").expect("write current snapshot");
        fs::write(staged_current_dir.join("compare.txt"), "after").expect("write staged snapshot");

        let error = with_rename_error_injected(
            &staged_current_dir,
            io::ErrorKind::PermissionDenied,
            || swap_in_staged_current(&paths, &staged_current_dir),
        )
        .expect_err("staged current rename should fail");

        assert!(matches!(error, CoreError::Io { .. }));
        assert_eq!(
            fs::read_to_string(current_dir.join("compare.txt")).expect("restored current"),
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
        fs::write(staged_current_dir.join("compare.txt"), "after").expect("write staged snapshot");

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
        fs::write(current_dir.join("compare.txt"), "staged").expect("write staged current");
        fs::write(backup_current_dir.join("compare.txt"), "before").expect("write backup");

        rollback_current_swap(&paths, Some(&backup_current_dir)).expect("rollback current swap");

        assert_eq!(
            fs::read_to_string(current_dir.join("compare.txt")).expect("restored current"),
            "before"
        );
        assert!(!backup_current_dir.exists());
    }

    #[test]
    fn cleanup_after_rollback_tolerates_missing_archived_and_staged_snapshots() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");

        assert!(
            SnapshotPersistPlan::unchanged(None, Vec::new())
                .cleanup_after_rollback(&paths)
                .is_empty()
        );
        rollback_current_swap(&paths, None).expect("rollback without backup");
    }

    #[test]
    fn rollback_failed_commit_preserves_primary_failure_when_rollback_also_fails() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        let current_dir = paths.current_snapshot_dir();
        let staged_current_dir = paths.snapshots_dir().join(".current-stage-test");
        let backup_current_dir = paths.snapshots_dir().join(".current-backup-test");
        fs::create_dir_all(&current_dir).expect("create current dir");
        fs::create_dir_all(&staged_current_dir).expect("create staged dir");
        fs::create_dir_all(&backup_current_dir).expect("create backup dir");

        let plan = SnapshotPersistPlan::with_staged_current(
            snapshot_reference(SnapshotSlot::Current, "current"),
            Vec::new(),
            staged_current_dir.clone(),
            Vec::new(),
            Vec::new(),
            false,
        );
        let primary_error = CoreError::io(
            paths.state_file(),
            io::Error::from(io::ErrorKind::PermissionDenied),
        );

        let error = with_rename_error_injected(
            &backup_current_dir,
            io::ErrorKind::PermissionDenied,
            || plan.rollback_failed_commit(&paths, Some(&backup_current_dir), primary_error),
        );

        let (summary, primary, rollback, cleanup) = expect_persist_transaction(error);
        assert!(summary.contains("primary persist failure: filesystem error at"));
        assert!(summary.contains("state.json"));
        assert!(summary.contains("rollback failure: filesystem error at"));
        assert!(summary.contains(".current-backup-test"));
        assert!(matches!(*primary, CoreError::Io { .. }));
        assert!(matches!(rollback, Some(error) if matches!(*error, CoreError::Io { .. })));
        assert!(cleanup.is_empty());
        assert!(!staged_current_dir.exists());
    }

    #[test]
    #[should_panic(expected = "expected composite persist transaction error")]
    fn expect_persist_transaction_panics_for_non_composite_errors() {
        let _ = expect_persist_transaction(CoreError::contract("not composite"));
    }

    #[test]
    fn cleanup_after_rollback_collects_all_cleanup_failures() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        fs::create_dir_all(paths.snapshots_dir()).expect("create snapshots dir");

        let staged_current_path = paths.snapshots_dir().join(".current-stage-file");
        fs::write(&staged_current_path, "not a directory").expect("write staged current file");

        let history_reference = snapshot_reference(SnapshotSlot::History, "history/problem");
        let staged_history_path = paths.snapshots_dir().join(".history-stage-file");
        fs::write(&staged_history_path, "not a directory").expect("write staged history file");

        let archived_path = snapshot_reference_dir(&paths.target_dir(), &history_reference);
        fs::create_dir_all(archived_path.parent().expect("archived parent"))
            .expect("create archived parent");
        fs::write(&archived_path, "not a directory").expect("write archived file");

        let plan = SnapshotPersistPlan {
            current_snapshot: None,
            snapshot_history: Vec::new(),
            staged_current_dir: Some(staged_current_path.clone()),
            staged_history_snapshots: vec![StagedHistorySnapshot {
                reference: history_reference.clone(),
                staged_dir: staged_history_path.clone(),
            }],
            pruned_snapshots: Vec::new(),
            clear_history_after_success: false,
        };

        let errors = plan.cleanup_after_rollback(&paths);
        assert_eq!(errors.len(), 3);
        assert!(matches!(errors[0], CoreError::Io { .. }));
        assert!(matches!(errors[1], CoreError::Io { .. }));
        assert!(matches!(errors[2], CoreError::Io { .. }));
        assert!(staged_current_path.exists());
        assert!(staged_history_path.exists());
        assert!(archived_path.exists());
    }

    #[test]
    fn commit_rolls_back_history_publish_failures_without_staged_current_swap() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        let history_reference = snapshot_reference(SnapshotSlot::History, "history/archived");
        let staged_history_dir = paths.snapshots_dir().join(".history-stage-test");
        fs::create_dir_all(&staged_history_dir).expect("create staged history");
        fs::write(staged_history_dir.join("compare.txt"), "history").expect("write staged file");

        let plan = SnapshotPersistPlan {
            current_snapshot: None,
            snapshot_history: Vec::new(),
            staged_current_dir: None,
            staged_history_snapshots: vec![StagedHistorySnapshot {
                reference: history_reference.clone(),
                staged_dir: staged_history_dir.clone(),
            }],
            pruned_snapshots: Vec::new(),
            clear_history_after_success: false,
        };

        let error = with_rename_error_injected(
            &staged_history_dir,
            io::ErrorKind::PermissionDenied,
            || plan.commit(&paths, &pending_state("demo")),
        )
        .expect_err("history publish failure should surface");

        assert!(matches!(error, CoreError::Io { .. }));
        assert!(!staged_history_dir.exists());
        assert!(
            !snapshot_reference_dir(&paths.target_dir(), &history_reference).exists(),
            "failed history publish must not leave a visible archive"
        );
    }

    #[test]
    fn rollback_failed_commit_returns_a_composite_error_when_cleanup_alone_fails() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        fs::create_dir_all(paths.snapshots_dir()).expect("create snapshots dir");
        let staged_current_path = paths.snapshots_dir().join(".current-stage-file");
        fs::write(&staged_current_path, "not a directory").expect("write staged current file");

        let plan = SnapshotPersistPlan {
            current_snapshot: None,
            snapshot_history: Vec::new(),
            staged_current_dir: Some(staged_current_path.clone()),
            staged_history_snapshots: Vec::new(),
            pruned_snapshots: Vec::new(),
            clear_history_after_success: false,
        };
        let primary_error = CoreError::io(
            paths.state_file(),
            io::Error::from(io::ErrorKind::PermissionDenied),
        );

        let (summary, primary, rollback, cleanup) =
            expect_persist_transaction(plan.rollback_failed_commit(&paths, None, primary_error));
        assert!(summary.contains("primary persist failure: filesystem error at"));
        assert!(matches!(*primary, CoreError::Io { .. }));
        assert!(rollback.is_none());
        assert_eq!(cleanup.len(), 1);
        assert!(matches!(cleanup[0], CoreError::Io { .. }));
    }

    #[test]
    fn rollback_failed_commit_preserves_rollback_and_cleanup_failures_together() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        fs::create_dir_all(paths.snapshots_dir()).expect("create snapshots dir");

        let staged_current_path = paths.snapshots_dir().join(".current-stage-file");
        fs::write(&staged_current_path, "not a directory").expect("write staged current file");
        let backup_current_dir = paths.snapshots_dir().join(".current-backup-test");
        fs::create_dir_all(&backup_current_dir).expect("create backup dir");

        let plan = SnapshotPersistPlan {
            current_snapshot: None,
            snapshot_history: Vec::new(),
            staged_current_dir: Some(staged_current_path.clone()),
            staged_history_snapshots: Vec::new(),
            pruned_snapshots: Vec::new(),
            clear_history_after_success: false,
        };
        let primary_error = CoreError::io(
            paths.state_file(),
            io::Error::from(io::ErrorKind::PermissionDenied),
        );

        let error = with_rename_error_injected(
            &backup_current_dir,
            io::ErrorKind::PermissionDenied,
            || plan.rollback_failed_commit(&paths, Some(&backup_current_dir), primary_error),
        );

        let (summary, primary, rollback, cleanup) = expect_persist_transaction(error);
        assert!(summary.contains("primary persist failure: filesystem error at"));
        assert!(summary.contains("rollback failure: filesystem error at"));
        assert!(summary.contains("cleanup failures: filesystem error at"));
        assert!(matches!(*primary, CoreError::Io { .. }));
        assert!(matches!(rollback, Some(error) if matches!(*error, CoreError::Io { .. })));
        assert_eq!(cleanup.len(), 1);
        assert!(matches!(cleanup[0], CoreError::Io { .. }));
    }
}

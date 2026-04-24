use std::path::{Path, PathBuf};

use crate::CoreError;
use crate::model::TargetId;

/// Resolved filesystem layout for one FFHN target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetPaths {
    watch_root: PathBuf,
    target_id: TargetId,
}

impl TargetPaths {
    /// Builds one target path set from a target id that is already expected to be contract-valid.
    ///
    /// This constructor panics when `target_id` violates FFHN's durable target-id contract.
    /// Boundary-facing code should prefer [`TargetPaths::try_new`].
    pub fn new(watch_root: impl Into<PathBuf>, target_id: impl AsRef<str>) -> Self {
        Self {
            watch_root: watch_root.into(),
            target_id: TargetId::new(target_id.as_ref())
                .expect("FFHN target ids must be validated before constructing TargetPaths"),
        }
    }

    /// Builds one target path set from a raw target id by validating FFHN's durable id contract.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when `target_id` violates FFHN's target-id contract.
    pub fn try_new(
        watch_root: impl Into<PathBuf>,
        target_id: impl TryInto<TargetId, Error = CoreError>,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            watch_root: watch_root.into(),
            target_id: target_id.try_into()?,
        })
    }

    /// Returns the configured watch root.
    pub fn watch_root(&self) -> &Path {
        &self.watch_root
    }

    /// Returns the target id this path set was created for.
    pub fn target_id(&self) -> &str {
        self.target_id.as_str()
    }

    /// Returns the validated target id value.
    pub fn validated_target_id(&self) -> &TargetId {
        &self.target_id
    }

    /// Returns the target directory under the watch root.
    pub fn target_dir(&self) -> PathBuf {
        self.watch_root.join(self.target_id.as_str())
    }

    /// Returns the target TOML path.
    pub fn target_file(&self) -> PathBuf {
        self.target_dir().join("target.toml")
    }

    /// Returns the state JSON path.
    pub fn state_file(&self) -> PathBuf {
        self.target_dir().join("state.json")
    }

    /// Returns the last-run report path.
    pub fn last_run_file(&self) -> PathBuf {
        self.target_dir().join("last_run.json")
    }

    /// Returns the lock directory.
    pub fn lock_dir(&self) -> PathBuf {
        self.target_dir().join("lock")
    }

    /// Returns the run-lock path.
    pub fn run_lock_file(&self) -> PathBuf {
        self.lock_dir().join("run.lock")
    }

    /// Returns the snapshots directory.
    pub fn snapshots_dir(&self) -> PathBuf {
        self.target_dir().join("snapshots")
    }

    /// Returns the current snapshot directory.
    pub fn current_snapshot_dir(&self) -> PathBuf {
        self.snapshots_dir().join("current")
    }

    /// Returns the previous snapshot directory.
    pub fn history_snapshots_dir(&self) -> PathBuf {
        self.snapshots_dir().join("history")
    }

    /// Returns one retained history snapshot directory.
    pub fn history_snapshot_dir(&self, snapshot_key: &str) -> PathBuf {
        self.history_snapshots_dir().join(snapshot_key)
    }
}

#[cfg(test)]
mod tests {
    use super::TargetPaths;
    use crate::TargetId;
    use tempfile::tempdir;

    #[test]
    fn target_paths_build_the_canonical_target_layout() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), TargetId::new("demo").expect("target id"));

        assert_eq!(paths.watch_root(), temp.path());
        assert_eq!(paths.target_id(), "demo");
        assert_eq!(paths.validated_target_id().as_str(), "demo");
        assert_eq!(paths.target_dir(), temp.path().join("demo"));
        assert_eq!(
            paths.target_file(),
            temp.path().join("demo").join("target.toml")
        );
        assert_eq!(
            paths.state_file(),
            temp.path().join("demo").join("state.json")
        );
        assert_eq!(
            paths.last_run_file(),
            temp.path().join("demo").join("last_run.json")
        );
        assert_eq!(paths.lock_dir(), temp.path().join("demo").join("lock"));
        assert_eq!(
            paths.run_lock_file(),
            temp.path().join("demo").join("lock").join("run.lock")
        );
        assert_eq!(
            paths.snapshots_dir(),
            temp.path().join("demo").join("snapshots")
        );
        assert_eq!(
            paths.current_snapshot_dir(),
            temp.path().join("demo").join("snapshots").join("current")
        );
        assert_eq!(
            paths.history_snapshots_dir(),
            temp.path().join("demo").join("snapshots").join("history")
        );
        assert_eq!(
            paths.history_snapshot_dir("2026-04-24T12:00:00Z"),
            temp.path()
                .join("demo")
                .join("snapshots")
                .join("history")
                .join("2026-04-24T12:00:00Z")
        );
    }

    #[test]
    fn target_paths_reject_invalid_target_ids_at_the_boundary() {
        let temp = tempdir().expect("tempdir");

        assert!(TargetPaths::try_new(temp.path(), "../escape").is_err());
    }
}

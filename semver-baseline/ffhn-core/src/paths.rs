use std::path::{Path, PathBuf};

/// Resolved filesystem layout for one FFHN target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetPaths {
    watch_root: PathBuf,
    target_id: String,
}

impl TargetPaths {
    /// Builds one target path set from the watch root and target id.
    pub fn new(watch_root: impl Into<PathBuf>, target_id: impl Into<String>) -> Self {
        Self {
            watch_root: watch_root.into(),
            target_id: target_id.into(),
        }
    }

    /// Returns the configured watch root.
    pub fn watch_root(&self) -> &Path {
        &self.watch_root
    }

    /// Returns the target id this path set was created for.
    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    /// Returns the target directory under the watch root.
    pub fn target_dir(&self) -> PathBuf {
        self.watch_root.join(&self.target_id)
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

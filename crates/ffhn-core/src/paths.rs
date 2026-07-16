use std::path::{Path, PathBuf};
use std::{fs, io};

use crate::{CoreError, TargetId};

/// Resolved v2 filesystem layout for one FFHN target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetPaths {
    watch_root: PathBuf,
    target_id: TargetId,
}

impl TargetPaths {
    /// Builds paths after validating one durable target id.
    pub fn try_new(
        watch_root: impl Into<PathBuf>,
        target_id: impl TryInto<TargetId, Error = CoreError>,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            watch_root: watch_root.into(),
            target_id: target_id.try_into()?,
        })
    }

    /// Returns the containing watch root.
    pub fn watch_root(&self) -> &Path {
        &self.watch_root
    }
    /// Returns the durable target id.
    pub fn target_id(&self) -> &str {
        self.target_id.as_str()
    }
    /// Returns the target configuration directory.
    pub fn target_dir(&self) -> PathBuf {
        self.watch_root.join(self.target_id.as_str())
    }
    /// Returns the v2 target TOML document.
    pub fn target_file(&self) -> PathBuf {
        self.target_dir().join("target.toml")
    }
    /// Returns the isolated v2 storage root.
    pub fn storage_root(&self) -> PathBuf {
        self.target_dir().join(".ffhn")
    }
    /// Returns the v2 state artifact.
    pub fn state_file(&self) -> PathBuf {
        self.storage_root().join("state.json")
    }
    /// Returns the stable lock directory outside target storage.
    pub fn lock_root(&self) -> PathBuf {
        self.watch_root.join(".ffhn-locks")
    }
    /// Returns the target lock path. It survives blind storage reset.
    pub fn run_lock_file(&self) -> PathBuf {
        self.lock_root()
            .join(format!("{}.lock", self.target_id.as_str()))
    }

    pub(crate) fn require_watch_root_directory(&self) -> Result<(), CoreError> {
        let metadata = fs::metadata(&self.watch_root).map_err(|error| {
            let source = if error.kind() == io::ErrorKind::NotFound {
                io::Error::new(io::ErrorKind::NotFound, "watch root does not exist")
            } else {
                error
            };
            CoreError::io(&self.watch_root, source)
        })?;
        if metadata.is_dir() {
            Ok(())
        } else {
            Err(CoreError::io(
                &self.watch_root,
                io::Error::other("watch root is not a directory"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn exposes_complete_v2_layout_and_rejects_missing_or_file_watch_roots() {
        let temporary = tempdir().expect("temporary directory");
        let paths = TargetPaths::try_new(temporary.path(), "demo").expect("paths");
        assert_eq!(paths.watch_root(), temporary.path());
        assert_eq!(paths.target_id(), "demo");
        assert!(paths.target_dir().ends_with("demo"));
        assert!(paths.target_file().ends_with("target.toml"));
        assert!(paths.storage_root().ends_with(".ffhn"));
        assert!(paths.state_file().ends_with("state.json"));
        assert!(paths.run_lock_file().ends_with("demo.lock"));
        paths
            .require_watch_root_directory()
            .expect("directory root");
        let missing =
            TargetPaths::try_new(temporary.path().join("missing"), "demo").expect("paths");
        assert!(missing.require_watch_root_directory().is_err());
        let file = temporary.path().join("file");
        fs::write(&file, "file").expect("file");
        let file_paths = TargetPaths::try_new(file, "demo").expect("paths");
        assert!(file_paths.require_watch_root_directory().is_err());
        let nested_in_file =
            TargetPaths::try_new(file_paths.watch_root().join("child"), "demo").expect("paths");
        assert!(nested_in_file.require_watch_root_directory().is_err());
    }
}

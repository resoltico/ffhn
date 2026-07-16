use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use crate::{CoreError, StateDocument, TargetDocument, TargetPaths};

/// Reset is intentionally the only path that accepts an arbitrary root node.
pub(super) fn blind_remove_storage_root(path: &std::path::Path) -> Result<bool, CoreError> {
    require_existing_parent_is_directory(path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(path).map_err(|error| CoreError::io(path, error))?;
            Ok(true)
        }
        Ok(_) => {
            fs::remove_file(path).map_err(|error| CoreError::io(path, error))?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(CoreError::io(path, error)),
    }
}

pub(super) fn load_target(paths: &TargetPaths) -> Result<TargetDocument, CoreError> {
    let text = fs::read_to_string(paths.target_file())
        .map_err(|error| CoreError::io(paths.target_file(), error))?;
    let target: TargetDocument = toml::from_str(&text)?;
    if target.target_id() != paths.target_id() {
        return Err(CoreError::contract(
            "target_id does not match target directory",
        ));
    }
    Ok(target)
}

/// Loads state only from an actual FFHN-owned directory and an actual regular state file.
pub(super) fn load_state(paths: &TargetPaths) -> Result<Option<StateDocument>, CoreError> {
    if checked_storage_root(paths)?.is_none() {
        return Ok(None);
    }
    let path = paths.state_file();
    if !checked_state_file(&path)? {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).map_err(|error| CoreError::io(&path, error))?;
    let state: StateDocument = serde_json::from_str(&text)?;
    state.validate()?;
    if state.target_id() != paths.target_id() {
        return Err(CoreError::contract(
            "state target_id does not match target directory",
        ));
    }
    Ok(Some(state))
}

/// Writes state only into an actual FFHN-owned directory and over an absent or regular file.
///
/// A successful return is the live-run durability boundary: state bytes are synchronized before
/// replacement, the installed replacement is synchronized afterward, and Unix also synchronizes
/// the directory metadata that names it. Callers must not deliver an outbox record before this
/// boundary succeeds.
pub(super) fn write_state(paths: &TargetPaths, state: &StateDocument) -> Result<(), CoreError> {
    write_state_with_durability(paths, state, &mut PlatformDurability)
}

fn write_state_with_durability<D: StateDurability>(
    paths: &TargetPaths,
    state: &StateDocument,
    durability: &mut D,
) -> Result<(), CoreError> {
    let parent = ensure_storage_root(paths, durability)?;
    let path = paths.state_file();
    checked_state_file(&path)?;
    let bytes = crate::stable_json::stable_json(state)?;
    let mut temporary =
        NamedTempFile::new_in(&parent).map_err(|error| CoreError::io(&parent, error))?;
    temporary
        .write_all(bytes.as_bytes())
        .map_err(|error| CoreError::io(&path, error))?;
    temporary
        .write_all(b"\n")
        .map_err(|error| CoreError::io(&path, error))?;
    durability
        .sync_file(DurabilityStage::StagedStateFile, temporary.as_file())
        .map_err(|error| CoreError::io(&path, error))?;
    let installed = temporary
        .persist(&path)
        .map_err(|error| CoreError::io(&path, error.error))?;
    durability
        .sync_file(DurabilityStage::InstalledStateFile, &installed)
        .map_err(|error| CoreError::io(&path, error))?;
    durability
        .sync_directory(DurabilityStage::StorageRootDirectory, &parent)
        .map_err(|error| CoreError::io(&parent, error))?;
    Ok(())
}

fn ensure_storage_root<D: StateDurability>(
    paths: &TargetPaths,
    durability: &mut D,
) -> Result<PathBuf, CoreError> {
    let root = paths.storage_root();
    if checked_storage_root(paths)?.is_none() {
        match fs::create_dir(&root) {
            Ok(()) => durability
                .sync_directory(DurabilityStage::NewStorageRootParent, &paths.target_dir())
                .map_err(|error| CoreError::io(paths.target_dir(), error))?,
            Err(error) => return Err(CoreError::io(&root, error)),
        }
    }
    checked_storage_root(paths)?
        .ok_or_else(|| CoreError::internal("new storage root disappeared before validation"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DurabilityStage {
    NewStorageRootParent,
    StagedStateFile,
    InstalledStateFile,
    StorageRootDirectory,
}

trait StateDurability {
    fn sync_file(&mut self, stage: DurabilityStage, file: &File) -> io::Result<()>;

    fn sync_directory(&mut self, stage: DurabilityStage, path: &Path) -> io::Result<()>;
}

struct PlatformDurability;

impl StateDurability for PlatformDurability {
    fn sync_file(&mut self, _stage: DurabilityStage, file: &File) -> io::Result<()> {
        file.sync_all()
    }

    fn sync_directory(&mut self, _stage: DurabilityStage, path: &Path) -> io::Result<()> {
        sync_directory(path)
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    // The installed file is still synchronized on every platform. Directory-handle synchronization
    // is not exposed by Rust's safe cross-platform API, so this boundary relies on final-file sync.
    Ok(())
}

fn checked_storage_root(paths: &TargetPaths) -> Result<Option<PathBuf>, CoreError> {
    match fs::symlink_metadata(paths.target_dir()) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            return Err(CoreError::contract(
                "FFHN target directory must be a directory",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(CoreError::io(paths.target_dir(), error)),
    }
    let root = paths.storage_root();
    match fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(CoreError::contract(
            "FFHN storage root must be a non-symlink directory; run `ffhn reset --target <ID>` to clear it",
        )),
        Ok(metadata) if metadata.is_dir() => Ok(Some(root)),
        Ok(_) => Err(CoreError::contract("FFHN storage root must be a directory")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(CoreError::io(&root, error)),
    }
}

fn require_existing_parent_is_directory(path: &Path) -> Result<(), CoreError> {
    let mut current = path.parent();
    while let Some(parent) = current {
        match fs::symlink_metadata(parent) {
            Ok(metadata) if metadata.is_dir() => return Ok(()),
            Ok(_) => {
                return Err(CoreError::contract(
                    "FFHN storage root parent must be a directory",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => current = parent.parent(),
            Err(error) => return Err(CoreError::io(parent, error)),
        }
    }
    Ok(())
}

/// Returns whether the state entry exists, rejecting links and non-regular nodes.
fn checked_state_file(path: &std::path::Path) -> Result<bool, CoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(CoreError::contract(
            "FFHN state file must be a regular file, not a symlink",
        )),
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err(CoreError::contract(
            "FFHN state file must be a regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(CoreError::io(path, error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RecordingDurability {
        fail_at: Option<DurabilityStage>,
        calls: Vec<DurabilityStage>,
    }

    impl RecordingDurability {
        fn succeeding() -> Self {
            Self {
                fail_at: None,
                calls: Vec::new(),
            }
        }

        fn failing_at(stage: DurabilityStage) -> Self {
            Self {
                fail_at: Some(stage),
                calls: Vec::new(),
            }
        }

        fn record(&mut self, stage: DurabilityStage) -> io::Result<()> {
            self.calls.push(stage);
            if self.fail_at == Some(stage) {
                return Err(io::Error::other("durability synchronization refused"));
            }
            Ok(())
        }
    }

    impl StateDurability for RecordingDurability {
        fn sync_file(&mut self, stage: DurabilityStage, _file: &File) -> io::Result<()> {
            self.record(stage)
        }

        fn sync_directory(&mut self, stage: DurabilityStage, _path: &Path) -> io::Result<()> {
            self.record(stage)
        }
    }

    fn fixture() -> (tempfile::TempDir, TargetPaths, StateDocument) {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let paths = TargetPaths::try_new(temporary.path(), "demo").expect("target paths");
        fs::create_dir(paths.target_dir()).expect("target directory");
        let state = StateDocument::new(
            crate::TargetId::new("demo").expect("target id"),
            "a".repeat(64),
        );
        (temporary, paths, state)
    }

    #[test]
    fn state_commit_synchronizes_every_durability_boundary_before_success() {
        let (_temporary, paths, state) = fixture();
        let mut durability = RecordingDurability::succeeding();

        write_state_with_durability(&paths, &state, &mut durability).expect("durable state write");

        assert_eq!(load_state(&paths).expect("load state"), Some(state));
        assert_eq!(
            durability.calls,
            vec![
                DurabilityStage::NewStorageRootParent,
                DurabilityStage::StagedStateFile,
                DurabilityStage::InstalledStateFile,
                DurabilityStage::StorageRootDirectory,
            ]
        );
    }

    #[test]
    fn every_durability_failure_fails_closed() {
        for (stage, expected_calls) in [
            (
                DurabilityStage::NewStorageRootParent,
                vec![DurabilityStage::NewStorageRootParent],
            ),
            (
                DurabilityStage::StagedStateFile,
                vec![
                    DurabilityStage::NewStorageRootParent,
                    DurabilityStage::StagedStateFile,
                ],
            ),
            (
                DurabilityStage::InstalledStateFile,
                vec![
                    DurabilityStage::NewStorageRootParent,
                    DurabilityStage::StagedStateFile,
                    DurabilityStage::InstalledStateFile,
                ],
            ),
            (
                DurabilityStage::StorageRootDirectory,
                vec![
                    DurabilityStage::NewStorageRootParent,
                    DurabilityStage::StagedStateFile,
                    DurabilityStage::InstalledStateFile,
                    DurabilityStage::StorageRootDirectory,
                ],
            ),
        ] {
            let (_temporary, paths, state) = fixture();
            let mut durability = RecordingDurability::failing_at(stage);

            let error = write_state_with_durability(&paths, &state, &mut durability)
                .expect_err("synchronization failure must fail the state commit");

            assert!(
                error
                    .to_string()
                    .contains("durability synchronization refused")
            );
            assert_eq!(durability.calls, expected_calls);
        }
    }
}

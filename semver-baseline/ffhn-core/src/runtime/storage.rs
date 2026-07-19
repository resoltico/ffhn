use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tempfile::NamedTempFile;

mod target_decode;

use target_decode::decode_target;

use crate::{CoreError, StateDocument, TargetDocument, TargetPaths};

const RESET_REQUIRED_STATE_SCHEMA_ERROR: &str = "stored state schema is incompatible; run `ffhn reset --target <ID>` before running this target";

/// The deliberately minimal compatibility boundary for persisted state.
///
/// It must remain independent of [`StateDocument`]: a retired schema is rejected before current
/// domain decoding encounters fields or enum variants that are no longer part of FFHN's model.
#[derive(Deserialize)]
struct StateSchemaEnvelope {
    schema_name: String,
    schema_version: u32,
}

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
    let target = decode_target(&text)?;
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
    let envelope: StateSchemaEnvelope = serde_json::from_str(&text)?;
    if envelope.schema_name != crate::STATE_SCHEMA_NAME
        || envelope.schema_version != crate::STATE_SCHEMA_VERSION
    {
        return Err(CoreError::contract(RESET_REQUIRED_STATE_SCHEMA_ERROR));
    }
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
    fn legacy_state_schemas_are_rejected_before_current_domain_decoding() {
        let (_temporary, paths, state) = fixture();
        fs::create_dir(paths.storage_root()).expect("storage root");

        let mut ordinary_legacy = serde_json::to_value(&state).expect("state JSON");
        ordinary_legacy["schema_version"] = serde_json::json!(13);

        let mut retired_variant_legacy = ordinary_legacy.clone();
        retired_variant_legacy["source_health"] = serde_json::json!({
            "state": "suspect",
            "reason_class": "htmlcut_internal_failure",
            "consecutive_unresolved": 1,
            "first_unresolved_at": "2026-01-01T00:00:00Z",
            "last_details": null
        });

        let mut wrong_schema_name = serde_json::to_value(&state).expect("state JSON");
        wrong_schema_name["schema_name"] = serde_json::json!("ffhn.retired_state");

        let expected = format!("contract error: {RESET_REQUIRED_STATE_SCHEMA_ERROR}");
        for legacy in [ordinary_legacy, retired_variant_legacy, wrong_schema_name] {
            fs::write(
                paths.state_file(),
                serde_json::to_vec(&legacy).expect("legacy state bytes"),
            )
            .expect("write legacy state");

            let error = load_state(&paths).expect_err("legacy state must require reset");
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn malformed_state_json_remains_an_unreadable_state_error() {
        let (_temporary, paths, _state) = fixture();
        fs::create_dir(paths.storage_root()).expect("storage root");
        fs::write(paths.state_file(), "{").expect("write malformed state");

        assert!(matches!(load_state(&paths), Err(CoreError::Json(_))));
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

    #[test]
    fn storage_root_validation_treats_a_missing_target_directory_as_absent() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let missing = TargetPaths::try_new(temporary.path(), "missing").expect("target paths");
        assert_eq!(
            checked_storage_root(&missing).expect("missing target"),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn storage_root_validation_surfaces_non_directory_target_ancestors() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let blocked_watch_root = temporary.path().join("watch-root-file");
        fs::write(&blocked_watch_root, "not a directory").expect("watch root file");
        let unusable = TargetPaths::try_new(blocked_watch_root, "demo").expect("target paths");
        assert!(checked_storage_root(&unusable).is_err());
    }

    #[test]
    fn blind_reset_allows_missing_ancestors_and_parentless_paths() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        assert!(
            !blind_remove_storage_root(&temporary.path().join("missing").join("state"))
                .expect("missing ancestor is harmless")
        );
        assert!(require_existing_parent_is_directory(Path::new("")).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn inaccessible_storage_nodes_are_reported_instead_of_treated_as_absent() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let target_directory = temporary.path().join("demo");
        fs::create_dir(&target_directory).expect("target directory");
        let paths = TargetPaths::try_new(temporary.path(), "demo").expect("target paths");
        let original_permissions = fs::metadata(&target_directory)
            .expect("target metadata")
            .permissions();
        fs::set_permissions(&target_directory, fs::Permissions::from_mode(0o000))
            .expect("restrict target directory");
        let root_result = checked_storage_root(&paths);
        fs::set_permissions(&target_directory, original_permissions)
            .expect("restore target directory");
        assert!(root_result.is_err());

        let denied = temporary.path().join("denied");
        fs::create_dir(&denied).expect("denied directory");
        let original_permissions = fs::metadata(&denied)
            .expect("denied metadata")
            .permissions();
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o000))
            .expect("restrict denied directory");
        let removal_result = blind_remove_storage_root(&denied.join("state"));
        let parent_result =
            require_existing_parent_is_directory(&denied.join("nested").join("state"));
        fs::set_permissions(&denied, original_permissions).expect("restore denied directory");

        assert!(removal_result.is_err());
        assert!(parent_result.is_err());
    }
}

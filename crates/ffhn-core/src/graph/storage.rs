//! Capability-scoped trusted-root traversal for v11 graph storage.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::CoreError;

use super::{
    CommitManifest, GraphIdentity, GraphPaths, LineageManifest, MeasurementId, MeasurementState,
    SourceId, SourceIdentity, SourcePaths, SourceState,
};

#[path = "storage/durability.rs"]
mod durability;
pub(super) use durability::sync_directory;

/// Capability-scoped graph root whose descendants are opened only through verified directories.
pub struct TrustedGraphRoot {
    pub(super) paths: GraphPaths,
    pub(super) dir: Dir,
}

impl TrustedGraphRoot {
    /// Opens an existing, non-symlink graph root as the first trusted directory handle.
    pub fn open(paths: GraphPaths) -> Result<Self, CoreError> {
        require_real_directory(paths.root())?;
        let dir = Dir::open_ambient_dir(paths.root(), ambient_authority())
            .map_err(|error| CoreError::io(paths.root(), error))?;
        Ok(Self { paths, dir })
    }

    /// Returns the immutable path description for this graph root.
    pub fn paths(&self) -> &GraphPaths {
        &self.paths
    }

    /// Atomically installs the immutable graph identity at the graph-root authority path.
    pub fn write_graph_identity(&self, identity: &GraphIdentity) -> Result<(), CoreError> {
        identity.validate()?;
        atomic_write_json(
            &self.dir,
            ".ffhn-graph.json",
            identity,
            &self.paths.identity_file(),
        )
    }

    /// Reads the immutable graph identity without following its filesystem entry.
    pub fn read_graph_identity(&self) -> Result<Option<GraphIdentity>, CoreError> {
        read_json_regular(
            &self.dir,
            ".ffhn-graph.json",
            &self.paths.identity_file(),
            "graph identity",
        )
    }

    /// Opens an existing source directory through the verified `sources/` hierarchy.
    pub fn open_source(&self, source_id: SourceId) -> Result<TrustedSourceDir, CoreError> {
        let sources = open_real_child(
            &self.dir,
            "sources",
            &self.paths.sources_dir(),
            "graph sources root",
        )?;
        let paths = self.paths.source(source_id);
        let source_dir = open_real_child(
            &sources,
            paths.source_id().as_str(),
            &paths.source_dir(),
            "source directory",
        )?;
        Ok(TrustedSourceDir {
            paths,
            dir: source_dir,
        })
    }
}

/// Capability-scoped source directory used for lineage operations.
pub struct TrustedSourceDir {
    pub(super) paths: SourcePaths,
    pub(super) dir: Dir,
}

impl TrustedSourceDir {
    /// Returns the immutable path description for this source.
    pub fn paths(&self) -> &SourcePaths {
        &self.paths
    }

    /// Opens the existing non-symlink storage root for normal commits and delivery.
    pub fn open_storage(&self) -> Result<TrustedStorageDir, CoreError> {
        let dir = open_real_child(
            &self.dir,
            ".ffhn",
            &self.paths.storage_dir(),
            "source storage root",
        )?;
        Ok(TrustedStorageDir {
            paths: self.paths.clone(),
            dir,
        })
    }

    /// Creates an empty storage root only when no storage node currently exists.
    pub fn create_storage(&self) -> Result<TrustedStorageDir, CoreError> {
        require_missing_fs_entry(
            self.dir.symlink_metadata(".ffhn"),
            &self.paths.storage_dir(),
            "source storage root",
        )?;
        self.dir
            .create_dir(".ffhn")
            .map_err(|error| CoreError::io(self.paths.storage_dir(), error))?;
        self.open_storage()
    }

    /// Atomically installs the lineage-authority identity outside the storage swap scope.
    pub fn write_identity(&self, identity: &SourceIdentity) -> Result<(), CoreError> {
        identity.validate()?;
        atomic_write_json(
            &self.dir,
            ".ffhn-identity.json",
            identity,
            &self.paths.identity_file(),
        )
    }

    /// Reads the current lineage authority without following its filesystem entry.
    pub fn read_identity(&self) -> Result<Option<SourceIdentity>, CoreError> {
        read_json_regular(
            &self.dir,
            ".ffhn-identity.json",
            &self.paths.identity_file(),
            "source identity",
        )
    }

    /// Atomically writes the durable lineage-transition commit point.
    pub fn write_lineage_manifest(&self, manifest: &LineageManifest) -> Result<(), CoreError> {
        manifest.validate()?;
        atomic_write_json(
            &self.dir,
            ".ffhn-lineage.manifest",
            manifest,
            &self.paths.lineage_manifest_file(),
        )
    }

    /// Reads a pending lineage-transition manifest without following its filesystem entry.
    pub fn read_lineage_manifest(&self) -> Result<Option<LineageManifest>, CoreError> {
        read_json_regular(
            &self.dir,
            ".ffhn-lineage.manifest",
            &self.paths.lineage_manifest_file(),
            "lineage manifest",
        )
    }

    pub(crate) fn read_lineage_manifest_bytes(&self) -> Result<Option<Vec<u8>>, CoreError> {
        read_regular_file_bytes(
            &self.dir,
            ".ffhn-lineage.manifest",
            &self.paths.lineage_manifest_file(),
            "lineage manifest",
        )
    }
}

/// Capability-scoped storage root used for normal commits and delivery operations.
pub struct TrustedStorageDir {
    pub(super) paths: SourcePaths,
    pub(super) dir: Dir,
}

impl TrustedStorageDir {
    /// Returns the immutable source path description from which this root was opened.
    pub fn paths(&self) -> &SourcePaths {
        &self.paths
    }

    /// Atomically installs the durable source state at the fixed source-state path.
    pub fn write_source_state(&self, state: &SourceState) -> Result<(), CoreError> {
        state.validate()?;
        atomic_write_json(
            &self.dir,
            "source-state.json",
            state,
            &self.paths.source_state_file(),
        )
    }

    /// Reads the durable source state without following its filesystem entry.
    pub fn read_source_state(&self) -> Result<Option<SourceState>, CoreError> {
        read_json_regular(
            &self.dir,
            "source-state.json",
            &self.paths.source_state_file(),
            "source state",
        )
    }

    /// Returns the exact persisted source-state bytes' SHA-256 for a manifest prior guard.
    pub fn source_state_sha256(&self) -> Result<Option<String>, CoreError> {
        hash_regular_file(
            &self.dir,
            "source-state.json",
            &self.paths.source_state_file(),
            "source state",
        )
    }

    /// Atomically writes the normal-commit manifest as the durable commit point.
    pub fn write_commit_manifest(&self, manifest: &CommitManifest) -> Result<(), CoreError> {
        manifest.validate()?;
        atomic_write_json(
            &self.dir,
            "commit.manifest",
            manifest,
            &self.paths.commit_manifest_file(),
        )
    }

    /// Reads a pending normal-commit manifest without following its filesystem entry.
    pub fn read_commit_manifest(&self) -> Result<Option<CommitManifest>, CoreError> {
        read_json_regular(
            &self.dir,
            "commit.manifest",
            &self.paths.commit_manifest_file(),
            "commit manifest",
        )
    }

    pub(crate) fn read_commit_manifest_bytes(&self) -> Result<Option<Vec<u8>>, CoreError> {
        read_regular_file_bytes(
            &self.dir,
            "commit.manifest",
            &self.paths.commit_manifest_file(),
            "commit manifest",
        )
    }

    /// Atomically installs one measurement state in its identity-owned storage subtree.
    pub fn write_measurement_state(
        &self,
        measurement_id: &MeasurementId,
        state: &MeasurementState,
    ) -> Result<(), CoreError> {
        state.validate()?;
        let dir = self.create_measurement_dir(measurement_id)?;
        atomic_write_json(
            &dir,
            "state.json",
            state,
            &self
                .paths
                .measurement_storage_dir(measurement_id)
                .join("state.json"),
        )
    }

    /// Reads one measurement state from a no-follow identity-owned storage subtree.
    pub fn read_measurement_state(
        &self,
        measurement_id: &MeasurementId,
    ) -> Result<Option<MeasurementState>, CoreError> {
        let Some(dir) = self.open_measurement_dir(measurement_id)? else {
            return Ok(None);
        };
        read_json_regular(
            &dir,
            "state.json",
            &self
                .paths
                .measurement_storage_dir(measurement_id)
                .join("state.json"),
            "measurement state",
        )
    }

    /// Returns one measurement state's exact persisted-byte SHA-256 for a manifest prior guard.
    pub fn measurement_state_sha256(
        &self,
        measurement_id: &MeasurementId,
    ) -> Result<Option<String>, CoreError> {
        let Some(dir) = self.open_measurement_dir(measurement_id)? else {
            return Ok(None);
        };
        hash_regular_file(
            &dir,
            "state.json",
            &self
                .paths
                .measurement_storage_dir(measurement_id)
                .join("state.json"),
            "measurement state",
        )
    }

    fn open_measurement_dir(
        &self,
        measurement_id: &MeasurementId,
    ) -> Result<Option<Dir>, CoreError> {
        let measurements_path = self.paths.storage_dir().join("measurements");
        let measurements = match open_optional_real_child(
            &self.dir,
            "measurements",
            &measurements_path,
            "measurement storage root",
        )? {
            Some(dir) => dir,
            None => return Ok(None),
        };
        open_optional_real_child(
            &measurements,
            measurement_id.as_str(),
            &self.paths.measurement_storage_dir(measurement_id),
            "measurement storage directory",
        )
    }

    fn create_measurement_dir(&self, measurement_id: &MeasurementId) -> Result<Dir, CoreError> {
        let measurements_path = self.paths.storage_dir().join("measurements");
        let measurements = open_or_create_real_child(
            &self.dir,
            "measurements",
            &measurements_path,
            "measurement storage root",
        )?;
        open_or_create_real_child(
            &measurements,
            measurement_id.as_str(),
            &self.paths.measurement_storage_dir(measurement_id),
            "measurement storage directory",
        )
    }
}

fn read_json_regular<T: DeserializeOwned>(
    dir: &Dir,
    name: &str,
    full_path: &Path,
    role: &str,
) -> Result<Option<T>, CoreError> {
    let Some(metadata) = optional_fs_entry(dir.symlink_metadata(name), full_path)? else {
        return Ok(None);
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CoreError::contract(format!(
            "{role} must be a non-symlink regular file"
        )));
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = dir
        .open_with(name, &options)
        .map_err(|error| CoreError::io(full_path, error))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|error| CoreError::io(full_path, error))?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(CoreError::from)
}

fn hash_regular_file(
    dir: &Dir,
    name: &str,
    full_path: &Path,
    role: &str,
) -> Result<Option<String>, CoreError> {
    let Some(metadata) = optional_fs_entry(dir.symlink_metadata(name), full_path)? else {
        return Ok(None);
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CoreError::contract(format!(
            "{role} must be a non-symlink regular file"
        )));
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = dir
        .open_with(name, &options)
        .map_err(|error| CoreError::io(full_path, error))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| CoreError::io(full_path, error))?;
    Ok(Some(crate::stable_json::sha256_hex(&bytes)))
}

fn read_regular_file_bytes(
    dir: &Dir,
    name: &str,
    full_path: &Path,
    role: &str,
) -> Result<Option<Vec<u8>>, CoreError> {
    let Some(metadata) = optional_fs_entry(dir.symlink_metadata(name), full_path)? else {
        return Ok(None);
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CoreError::contract(format!(
            "{role} must be a non-symlink regular file"
        )));
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = dir
        .open_with(name, &options)
        .map_err(|error| CoreError::io(full_path, error))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| CoreError::io(full_path, error))?;
    Ok(Some(bytes))
}

fn atomic_write_json<T: Serialize>(
    dir: &Dir,
    final_name: &str,
    value: &T,
    full_path: &Path,
) -> Result<(), CoreError> {
    let bytes = crate::stable_json::stable_json(value)?;
    atomic_write_text(dir, final_name, &bytes, full_path)
}

pub(super) fn atomic_write_text(
    dir: &Dir,
    final_name: &str,
    text: &str,
    full_path: &Path,
) -> Result<(), CoreError> {
    let staged_name = format!(".ffhn-stage-{}", Uuid::new_v4());
    let result = (|| {
        let mut staged = dir
            .create(&staged_name)
            .map_err(|error| CoreError::io(full_path, error))?;
        staged
            .write_all(text.as_bytes())
            .map_err(|error| CoreError::io(full_path, error))?;
        staged
            .write_all(b"\n")
            .map_err(|error| CoreError::io(full_path, error))?;
        staged
            .sync_all()
            .map_err(|error| CoreError::io(full_path, error))?;
        dir.rename(&staged_name, dir, final_name)
            .map_err(|error| CoreError::io(full_path, error))?;
        sync_directory(dir, full_path)
    })();
    if result.is_err() {
        let _ = dir.remove_file(&staged_name);
    }
    result
}

pub(super) fn remove_regular_file(
    dir: &Dir,
    name: &str,
    full_path: &Path,
    role: &str,
) -> Result<(), CoreError> {
    let metadata = dir
        .symlink_metadata(name)
        .map_err(|error| CoreError::io(full_path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CoreError::contract(format!(
            "{role} must be a non-symlink regular file"
        )));
    }
    dir.remove_file(name)
        .map_err(|error| CoreError::io(full_path, error))?;
    sync_directory(dir, full_path)
}

pub(super) fn remove_tombstone(dir: &Dir, full_path: &Path) -> Result<(), CoreError> {
    match optional_fs_entry(dir.symlink_metadata(".ffhn-tombstone"), full_path)? {
        Some(metadata) if metadata.is_dir() => dir
            .remove_dir_all(".ffhn-tombstone")
            .map_err(|error| CoreError::io(full_path, error))?,
        Some(_) => dir
            .remove_file(".ffhn-tombstone")
            .map_err(|error| CoreError::io(full_path, error))?,
        None => {}
    }
    sync_directory(dir, full_path)
}

fn require_real_directory(path: &Path) -> Result<(), CoreError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| CoreError::io(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CoreError::contract(
            "trusted graph-root component must be a non-symlink directory",
        ));
    }
    Ok(())
}

pub(super) fn open_real_child(
    parent: &Dir,
    child_name: &str,
    full_path: &Path,
    role: &str,
) -> Result<Dir, CoreError> {
    let metadata = parent
        .symlink_metadata(child_name)
        .map_err(|error| CoreError::io(full_path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CoreError::contract(format!(
            "{role} must be a non-symlink directory"
        )));
    }
    parent
        .open_dir_nofollow(child_name)
        .map_err(|error| CoreError::io(full_path, error))
}

pub(super) fn open_optional_real_child(
    parent: &Dir,
    child_name: &str,
    full_path: &Path,
    role: &str,
) -> Result<Option<Dir>, CoreError> {
    match optional_fs_entry(parent.symlink_metadata(child_name), full_path)? {
        Some(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(
            CoreError::contract(format!("{role} must be a non-symlink directory")),
        ),
        Some(_) => parent
            .open_dir_nofollow(child_name)
            .map(Some)
            .map_err(|error| CoreError::io(full_path, error)),
        None => Ok(None),
    }
}

pub(super) fn optional_fs_entry<T>(
    result: std::io::Result<T>,
    full_path: &Path,
) -> Result<Option<T>, CoreError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(CoreError::io(full_path, error)),
    }
}

pub(super) fn require_missing_fs_entry<T>(
    result: std::io::Result<T>,
    full_path: &Path,
    role: &str,
) -> Result<(), CoreError> {
    match optional_fs_entry(result, full_path)? {
        Some(_) => Err(CoreError::contract(format!(
            "{role} already exists and cannot be created again"
        ))),
        None => Ok(()),
    }
}

pub(super) fn open_or_create_real_child(
    parent: &Dir,
    child_name: &str,
    full_path: &Path,
    role: &str,
) -> Result<Dir, CoreError> {
    match open_optional_real_child(parent, child_name, full_path, role)? {
        Some(dir) => Ok(dir),
        None => {
            parent
                .create_dir(child_name)
                .map_err(|error| CoreError::io(full_path, error))?;
            open_real_child(parent, child_name, full_path, role)
        }
    }
}

#[cfg(test)]
#[path = "storage/tests.rs"]
mod tests;

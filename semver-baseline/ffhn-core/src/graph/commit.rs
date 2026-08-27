//! Crash-atomic normal-commit staging, replay, and recovery.

use std::io::{Read, Write};

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use serde::Serialize;

use crate::CoreError;

use super::{
    CommitManifest, CommitOperation, CommitRecovery, ManifestPath, SourceIdentity,
    TrustedSourceDir, TrustedStorageDir,
    storage::{optional_fs_entry, remove_regular_file, require_missing_fs_entry, sync_directory},
};

impl TrustedSourceDir {
    /// Durably starts and completes one normal source-generation commit.
    pub fn apply_normal_commit(&self, manifest: &CommitManifest) -> Result<(), CoreError> {
        self.require_commit_source(manifest)?;
        let storage = self.open_storage()?;
        storage.write_commit_manifest(manifest)?;
        self.recover_normal_commit()
    }

    /// Replays one pending normal commit only when its manifest matches lineage and generation.
    pub fn recover_normal_commit(&self) -> Result<(), CoreError> {
        let storage = self.open_storage()?;
        let Some(manifest) = storage.read_commit_manifest()? else {
            storage.discard_uncommitted_staging()?;
            return Ok(());
        };
        self.require_commit_source(&manifest)?;
        let mut identity = self
            .read_identity()?
            .ok_or_else(|| CoreError::contract("normal commit requires a source identity"))?;
        let current = storage
            .read_source_state()?
            .ok_or_else(|| CoreError::contract("normal commit requires source state"))?;
        if manifest.recover_against(&identity, current.generation()) != CommitRecovery::Apply {
            return Err(CoreError::contract(
                "commit manifest does not match observed source lineage or generation",
            ));
        }
        apply_identity_additions(&mut identity, &manifest)?;
        self.write_identity(&identity)?;
        storage.replay_commit_operations(manifest.entries())?;
        let installed = storage
            .read_source_state()?
            .ok_or_else(|| CoreError::contract("commit removed source state"))?;
        if installed.generation() != manifest.generation()
            || installed.source_instance_id() != manifest.source_instance_id()
        {
            return Err(CoreError::contract(
                "commit operations did not install the manifest generation source state",
            ));
        }
        remove_regular_file(
            &storage.dir,
            "commit.manifest",
            &storage.paths.commit_manifest_file(),
            "commit manifest",
        )
    }

    fn require_commit_source(&self, manifest: &CommitManifest) -> Result<(), CoreError> {
        if manifest.source_id() == self.paths.source_id() {
            Ok(())
        } else {
            Err(CoreError::contract(
                "commit manifest source_id does not match the source directory",
            ))
        }
    }
}

impl TrustedStorageDir {
    /// Writes and synchronizes staged bytes at a validated storage-relative path.
    pub fn stage_bytes(&self, path: &ManifestPath, bytes: &[u8]) -> Result<String, CoreError> {
        if !path.is_staged() {
            return Err(CoreError::contract(
                "normal-commit bytes must be staged inside the reserved staged directory",
            ));
        }
        let (parent, name, full_path) = self.open_manifest_parent(path, true)?;
        require_missing_fs_entry(
            parent.symlink_metadata(&name),
            &full_path,
            "commit staged path",
        )?;
        let mut file = parent
            .create(&name)
            .map_err(|error| CoreError::io(&full_path, error))?;
        file.write_all(bytes)
            .map_err(|error| CoreError::io(&full_path, error))?;
        file.sync_all()
            .map_err(|error| CoreError::io(&full_path, error))?;
        sync_directory(&parent, &full_path)?;
        Ok(crate::stable_json::sha256_hex(bytes))
    }

    /// Stages stable JSON plus its required trailing newline for a normal source-generation commit.
    pub fn stage_json<T: Serialize>(
        &self,
        path: &ManifestPath,
        value: &T,
    ) -> Result<String, CoreError> {
        let bytes = format!("{}\n", crate::stable_json::stable_json(value)?);
        self.stage_bytes(path, bytes.as_bytes())
    }

    /// Returns the exact current SHA-256 for a validated storage-relative file path.
    pub fn existing_file_sha256(&self, path: &ManifestPath) -> Result<Option<String>, CoreError> {
        let (parent, name, full_path) = self.open_manifest_parent(path, false)?;
        read_regular_bytes(&parent, &name, &full_path)
            .map(|bytes| bytes.map(|bytes| crate::stable_json::sha256_hex(&bytes)))
    }

    fn discard_uncommitted_staging(&self) -> Result<(), CoreError> {
        match optional_fs_entry(
            self.dir.symlink_metadata("staged"),
            &self.paths.storage_dir().join("staged"),
        )? {
            Some(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(
                CoreError::contract("reserved commit staging root must be a non-symlink directory"),
            ),
            Some(_) => {
                map_staging_removal(
                    self.dir.remove_dir_all("staged"),
                    &self.paths.storage_dir().join("staged"),
                )?;
                sync_directory(&self.dir, &self.paths.storage_dir().join("staged"))
            }
            None => Ok(()),
        }
    }

    fn replay_commit_operations(&self, operations: &[CommitOperation]) -> Result<(), CoreError> {
        for operation in operations {
            match operation {
                CommitOperation::Install {
                    final_path,
                    staged_path,
                    expected_prior_sha256,
                    result_sha256,
                } => self.replay_install(
                    final_path,
                    staged_path,
                    expected_prior_sha256.as_deref(),
                    result_sha256,
                )?,
                CommitOperation::Remove {
                    final_path,
                    expected_prior_sha256,
                } => self.replay_remove(final_path, expected_prior_sha256)?,
            }
        }
        Ok(())
    }

    fn replay_install(
        &self,
        final_path: &ManifestPath,
        staged_path: &ManifestPath,
        expected_prior_sha256: Option<&str>,
        result_sha256: &str,
    ) -> Result<(), CoreError> {
        let (final_parent, final_name, final_full_path) =
            self.open_manifest_parent(final_path, true)?;
        if let Some(bytes) = read_regular_bytes(&final_parent, &final_name, &final_full_path)? {
            let actual_hash = crate::stable_json::sha256_hex(&bytes);
            if actual_hash == result_sha256 {
                return Ok(());
            }
            if expected_prior_sha256 != Some(actual_hash.as_str()) {
                return Err(CoreError::contract(
                    "commit install final bytes do not match the expected prior hash",
                ));
            }
        } else if expected_prior_sha256.is_some() {
            return Err(CoreError::contract(
                "commit install requires an expected prior file that is absent",
            ));
        }
        let (staged_parent, staged_name, staged_full_path) =
            self.open_manifest_parent(staged_path, false)?;
        let staged = read_regular_bytes(&staged_parent, &staged_name, &staged_full_path)?
            .ok_or_else(|| CoreError::contract("commit staged file is absent"))?;
        if crate::stable_json::sha256_hex(&staged) != result_sha256 {
            return Err(CoreError::contract(
                "commit staged file does not match the result hash",
            ));
        }
        staged_parent
            .rename(&staged_name, &final_parent, &final_name)
            .map_err(|error| CoreError::io(&final_full_path, error))?;
        sync_directory(&staged_parent, &staged_full_path)?;
        sync_directory(&final_parent, &final_full_path)
    }

    fn replay_remove(
        &self,
        final_path: &ManifestPath,
        expected_prior_sha256: &str,
    ) -> Result<(), CoreError> {
        let (parent, name, full_path) = self.open_manifest_parent(final_path, true)?;
        let Some(bytes) = read_regular_bytes(&parent, &name, &full_path)? else {
            return Ok(());
        };
        if crate::stable_json::sha256_hex(&bytes) != expected_prior_sha256 {
            return Err(CoreError::contract(
                "commit remove final bytes do not match the expected prior hash",
            ));
        }
        parent
            .remove_file(&name)
            .map_err(|error| CoreError::io(&full_path, error))?;
        sync_directory(&parent, &full_path)
    }

    fn open_manifest_parent(
        &self,
        path: &ManifestPath,
        create: bool,
    ) -> Result<(Dir, String, std::path::PathBuf), CoreError> {
        let mut components = path.as_str().split('/').collect::<Vec<_>>();
        let name = components
            .pop()
            .ok_or_else(|| CoreError::contract("manifest path has no file name"))?
            .to_owned();
        let mut dir = Dir::reopen_dir(&self.dir)
            .map_err(|error| CoreError::io(self.paths.storage_dir(), error))?;
        let mut full_path = self.paths.storage_dir();
        for component in components {
            full_path.push(component);
            dir = if create {
                open_or_create_child(&dir, component, &full_path)?
            } else {
                open_existing_child(&dir, component, &full_path)?
            };
        }
        let file_path = full_path.join(&name);
        Ok((dir, name, file_path))
    }
}

fn map_staging_removal(
    result: std::io::Result<()>,
    path: &std::path::Path,
) -> Result<(), CoreError> {
    result.map_err(|error| CoreError::io(path, error))
}

fn apply_identity_additions(
    identity: &mut SourceIdentity,
    manifest: &CommitManifest,
) -> Result<(), CoreError> {
    for (measurement_id, instance_id) in manifest.identity_measurement_additions() {
        match identity.measurements().get(measurement_id) {
            Some(known) if known.measurement_instance_id() == instance_id => {}
            Some(_) => {
                return Err(CoreError::contract(
                    "commit identity addition conflicts with existing measurement lineage",
                ));
            }
            None => identity
                .register_measurement_instance(measurement_id.clone(), instance_id.clone())?,
        }
    }
    Ok(())
}

fn open_existing_child(
    parent: &Dir,
    name: &str,
    full_path: &std::path::Path,
) -> Result<Dir, CoreError> {
    let metadata = parent
        .symlink_metadata(name)
        .map_err(|error| CoreError::io(full_path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CoreError::contract(
            "manifest path parent must be a non-symlink directory",
        ));
    }
    parent
        .open_dir_nofollow(name)
        .map_err(|error| CoreError::io(full_path, error))
}

fn open_or_create_child(
    parent: &Dir,
    name: &str,
    full_path: &std::path::Path,
) -> Result<Dir, CoreError> {
    match optional_fs_entry(parent.symlink_metadata(name), full_path)? {
        Some(_) => open_existing_child(parent, name, full_path),
        None => {
            parent
                .create_dir(name)
                .map_err(|error| CoreError::io(full_path, error))?;
            open_existing_child(parent, name, full_path)
        }
    }
}

fn read_regular_bytes(
    dir: &Dir,
    name: &str,
    full_path: &std::path::Path,
) -> Result<Option<Vec<u8>>, CoreError> {
    let Some(metadata) = optional_fs_entry(dir.symlink_metadata(name), full_path)? else {
        return Ok(None);
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CoreError::contract(
            "manifest path must name a non-symlink regular file",
        ));
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

#[cfg(test)]
#[path = "commit/tests.rs"]
mod tests;

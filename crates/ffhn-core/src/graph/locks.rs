//! OS advisory locks rooted at trusted graph and source directories.

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::OpenOptions;
use fs2::FileExt;

use crate::CoreError;

use super::storage::optional_fs_entry;
use super::{TrustedGraphRoot, TrustedSourceDir};

/// Held exclusive graph-wide agent lease. Dropping it releases the OS advisory lock.
pub struct GraphLease {
    _file: std::fs::File,
}

/// Held exclusive source writer lock. Dropping it releases the OS advisory lock.
pub struct SourceWriteLease {
    _file: std::fs::File,
}

/// Held shared source-reader lock. Dropping it releases the OS advisory lock.
pub struct SourceReadLease {
    _file: std::fs::File,
}

impl TrustedGraphRoot {
    /// Attempts to claim the singleton graph lease without following or replacing its lock entry.
    pub fn try_acquire_agent_lease(&self) -> Result<Option<GraphLease>, CoreError> {
        open_and_try_lock(
            &self.dir,
            ".ffhn-agent.lock",
            &self.paths.agent_lock_file(),
            "graph agent lock",
            LockMode::Exclusive,
        )
        .map(|file| file.map(|_file| GraphLease { _file }))
    }
}

impl TrustedSourceDir {
    /// Attempts to claim this source's exclusive writer lock without blocking an agent worker.
    pub fn try_acquire_write_lease(&self) -> Result<Option<SourceWriteLease>, CoreError> {
        open_and_try_lock(
            &self.dir,
            ".ffhn.lock",
            &self.paths.lock_file(),
            "source lock",
            LockMode::Exclusive,
        )
        .map(|file| file.map(|_file| SourceWriteLease { _file }))
    }

    /// Attempts to claim this source's shared reader lock without observing a torn generation.
    pub fn try_acquire_read_lease(&self) -> Result<Option<SourceReadLease>, CoreError> {
        open_and_try_lock(
            &self.dir,
            ".ffhn.lock",
            &self.paths.lock_file(),
            "source lock",
            LockMode::Shared,
        )
        .map(|file| file.map(|_file| SourceReadLease { _file }))
    }
}

#[derive(Clone, Copy)]
enum LockMode {
    Exclusive,
    Shared,
}

fn open_and_try_lock(
    dir: &cap_std::fs::Dir,
    name: &str,
    full_path: &std::path::Path,
    role: &str,
    mode: LockMode,
) -> Result<Option<std::fs::File>, CoreError> {
    match optional_fs_entry(dir.symlink_metadata(name), full_path)? {
        Some(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(CoreError::contract(format!(
                "{role} must be a non-symlink regular file"
            )));
        }
        Some(_) | None => {}
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .follow(FollowSymlinks::No);
    let file = dir
        .open_with(name, &options)
        .map_err(|error| CoreError::io(full_path, error))?
        .into_std();
    let lock = match mode {
        LockMode::Exclusive => file.try_lock_exclusive(),
        LockMode::Shared => file.try_lock_shared().map_err(std::io::Error::from),
    };
    if classify_lock_result(lock).map_err(|error| CoreError::io(full_path, error))? {
        Ok(Some(file))
    } else {
        Ok(None)
    }
}

fn classify_lock_result(result: std::io::Result<()>) -> std::io::Result<bool> {
    match result {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "locks/tests.rs"]
mod tests;

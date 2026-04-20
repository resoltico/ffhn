use std::fs::{self, File, OpenOptions};

use fs2::FileExt;

use crate::{CoreError, TargetPaths};

#[derive(Debug)]
pub(crate) struct RunLock {
    file: File,
}

impl Drop for RunLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub(crate) fn try_lock_exclusive(paths: &TargetPaths) -> Result<RunLock, CoreError> {
    fs::create_dir_all(paths.lock_dir()).map_err(|error| CoreError::io(paths.lock_dir(), error))?;
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(paths.run_lock_file())
        .map_err(|error| CoreError::io(paths.run_lock_file(), error))?;
    if file.try_lock_exclusive().is_err() {
        return Err(CoreError::htmlcut("run lock unavailable"));
    }
    Ok(RunLock { file })
}

pub(crate) fn lock_shared(paths: &TargetPaths) -> Result<RunLock, CoreError> {
    fs::create_dir_all(paths.lock_dir()).map_err(|error| CoreError::io(paths.lock_dir(), error))?;
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(paths.run_lock_file())
        .map_err(|error| CoreError::io(paths.run_lock_file(), error))?;
    file.lock_shared()
        .map_err(|error| CoreError::io(paths.run_lock_file(), error))?;
    Ok(RunLock { file })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn exclusive_lock_blocks_another_exclusive_lock() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");

        let _first = try_lock_exclusive(&paths).expect("first exclusive lock");
        assert!(try_lock_exclusive(&paths).is_err());
    }

    #[test]
    fn shared_locks_can_coexist() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");

        let _first = lock_shared(&paths).expect("first shared lock");
        let _second = lock_shared(&paths).expect("second shared lock");
    }
}

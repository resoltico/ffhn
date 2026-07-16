use std::fs::{self, File, OpenOptions};
use std::thread;
use std::time::Duration;

use fs2::FileExt;

use crate::{CoreError, TargetPaths};

pub(super) struct TargetLock(File);

impl Drop for TargetLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

#[derive(Debug)]
pub(super) enum LockError {
    Unavailable,
    Io(CoreError),
}

pub(super) fn lock_exclusive(paths: &TargetPaths) -> Result<TargetLock, LockError> {
    let path = paths.run_lock_file();
    let file = open_lock_file(&path).map_err(LockError::Io)?;
    match file.try_lock_exclusive() {
        Ok(()) => Ok(TargetLock(file)),
        Err(error) if lock_is_unavailable(&error) => Err(LockError::Unavailable),
        Err(error) => Err(LockError::Io(CoreError::io(path, error))),
    }
}

pub(super) fn lock_shared(paths: &TargetPaths) -> Result<TargetLock, CoreError> {
    let path = paths.run_lock_file();
    let file = open_lock_file(&path)?;
    loop {
        match FileExt::try_lock_shared(&file) {
            Ok(()) => return Ok(TargetLock(file)),
            Err(error)
                if lock_is_unavailable(&error)
                    || error.kind() == std::io::ErrorKind::Interrupted =>
            {
                thread::sleep(Duration::from_millis(5))
            }
            Err(error) => return Err(CoreError::io(&path, error)),
        }
    }
}

/// Opens a normal FFHN-owned lock file, rejecting pre-existing device, directory, and link nodes.
///
/// Advisory locking is permissive on several platforms (notably Linux accepts a FIFO), so the
/// node check is part of the lock contract rather than an incidental platform restriction.
fn open_lock_file(path: &std::path::Path) -> Result<File, CoreError> {
    let parent = path.parent().expect("lock path parent");
    fs::create_dir_all(parent).map_err(|error| CoreError::io(parent, error))?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => {
            return Err(CoreError::contract(
                "FFHN run lock must be a regular file, not a link or special filesystem node",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(CoreError::io(path, error)),
    }
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|error| CoreError::io(path, error))
}

fn lock_is_unavailable(error: &std::io::Error) -> bool {
    if error.kind() == std::io::ErrorKind::WouldBlock {
        return true;
    }

    #[cfg(windows)]
    {
        matches!(error.raw_os_error(), Some(32 | 33))
    }

    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::lock_is_unavailable;

    #[test]
    fn windows_lock_sharing_codes_are_classified_as_unavailable() {
        for code in [32, 33] {
            assert!(lock_is_unavailable(&std::io::Error::from_raw_os_error(
                code
            )));
        }
        assert!(!lock_is_unavailable(&std::io::Error::from_raw_os_error(5)));
    }
}

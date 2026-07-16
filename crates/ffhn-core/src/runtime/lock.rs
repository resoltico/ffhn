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
    let parent = path.parent().expect("lock path parent");
    fs::create_dir_all(parent).map_err(|error| LockError::Io(CoreError::io(parent, error)))?;
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| LockError::Io(CoreError::io(&path, error)))?;
    match file.try_lock_exclusive() {
        Ok(()) => Ok(TargetLock(file)),
        Err(error) if lock_is_unavailable(&error) => Err(LockError::Unavailable),
        Err(error) => Err(LockError::Io(CoreError::io(path, error))),
    }
}

pub(super) fn lock_shared(paths: &TargetPaths) -> Result<TargetLock, CoreError> {
    let path = paths.run_lock_file();
    let parent = path.parent().expect("lock path parent");
    fs::create_dir_all(parent).map_err(|error| CoreError::io(parent, error))?;
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| CoreError::io(&path, error))?;
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

use std::fs::{self, File, OpenOptions};
use std::io;
#[cfg(test)]
use std::{cell::RefCell, thread_local};

use fs2::FileExt;

use crate::{CoreError, TargetPaths};

#[derive(Debug)]
pub(crate) struct RunLock {
    file: File,
}

#[derive(Debug)]
pub(crate) enum ExclusiveLockError {
    Unavailable,
    Io(CoreError),
}

#[cfg(test)]
thread_local! {
    static TRY_LOCK_EXCLUSIVE_OVERRIDE: RefCell<Option<io::ErrorKind>> = const { RefCell::new(None) };
}

impl Drop for RunLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub(crate) fn try_lock_exclusive(paths: &TargetPaths) -> Result<RunLock, ExclusiveLockError> {
    fs::create_dir_all(paths.lock_dir())
        .map_err(|error| ExclusiveLockError::Io(CoreError::io(paths.lock_dir(), error)))?;
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(paths.run_lock_file())
        .map_err(|error| ExclusiveLockError::Io(CoreError::io(paths.run_lock_file(), error)))?;
    match attempt_exclusive_lock(&file) {
        Ok(()) => Ok(RunLock { file }),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            Err(ExclusiveLockError::Unavailable)
        }
        Err(error) => Err(ExclusiveLockError::Io(CoreError::io(
            paths.run_lock_file(),
            error,
        ))),
    }
}

fn attempt_exclusive_lock(file: &File) -> io::Result<()> {
    #[cfg(test)]
    if let Some(kind) = TRY_LOCK_EXCLUSIVE_OVERRIDE.with(|state| *state.borrow()) {
        return Err(io::Error::from(kind));
    }

    file.try_lock_exclusive()
}

#[cfg(test)]
pub(crate) fn with_exclusive_lock_error_injected<T>(
    kind: io::ErrorKind,
    callback: impl FnOnce() -> T,
) -> T {
    struct ResetGuard;

    impl Drop for ResetGuard {
        fn drop(&mut self) {
            TRY_LOCK_EXCLUSIVE_OVERRIDE.with(|state| *state.borrow_mut() = None);
        }
    }

    TRY_LOCK_EXCLUSIVE_OVERRIDE.with(|state| {
        let mut state = state.borrow_mut();
        assert!(state.is_none(), "exclusive lock override already set");
        *state = Some(kind);
    });

    let _reset = ResetGuard;
    callback()
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
        assert!(matches!(
            try_lock_exclusive(&paths),
            Err(ExclusiveLockError::Unavailable)
        ));
    }

    #[test]
    fn exclusive_lock_keeps_filesystem_errors_distinct_from_contention() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        std::fs::create_dir_all(paths.target_dir()).expect("create target dir");
        std::fs::write(paths.lock_dir(), "blocked").expect("block lock path");

        assert!(matches!(
            try_lock_exclusive(&paths),
            Err(ExclusiveLockError::Io(CoreError::Io { .. }))
        ));
    }

    #[test]
    fn exclusive_lock_surfaces_non_contention_lock_failures() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");

        with_exclusive_lock_error_injected(io::ErrorKind::Other, || {
            assert!(matches!(
                try_lock_exclusive(&paths),
                Err(ExclusiveLockError::Io(CoreError::Io { .. }))
            ));
        });
    }

    #[test]
    fn shared_locks_can_coexist() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");

        let _first = lock_shared(&paths).expect("first shared lock");
        let _second = lock_shared(&paths).expect("second shared lock");
    }
}

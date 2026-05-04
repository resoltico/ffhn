use std::fs::{self, File, OpenOptions};
use std::io;
use std::time::Duration;
#[cfg(test)]
use std::{cell::RefCell, collections::VecDeque, thread_local};

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
    static LOCK_SHARED_OVERRIDE: RefCell<VecDeque<io::ErrorKind>> = const { RefCell::new(VecDeque::new()) };
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

    loop {
        match attempt_shared_lock(&file) {
            Ok(()) => break,
            // Some cross-process filesystems surface shared-lock contention as WouldBlock
            // instead of sleeping inside lock_shared(); retry until the stable view is available.
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(CoreError::io(paths.run_lock_file(), error)),
        }
    }

    Ok(RunLock { file })
}

fn attempt_shared_lock(file: &File) -> io::Result<()> {
    #[cfg(test)]
    if let Some(kind) = LOCK_SHARED_OVERRIDE.with(|state| state.borrow_mut().pop_front()) {
        return Err(io::Error::from(kind));
    }

    file.lock_shared()
}

#[cfg(test)]
pub(crate) fn with_shared_lock_errors_injected<T>(
    kinds: &[io::ErrorKind],
    callback: impl FnOnce() -> T,
) -> T {
    struct ResetGuard;

    impl Drop for ResetGuard {
        fn drop(&mut self) {
            LOCK_SHARED_OVERRIDE.with(|state| state.borrow_mut().clear());
        }
    }

    LOCK_SHARED_OVERRIDE.with(|state| {
        let mut state = state.borrow_mut();
        assert!(state.is_empty(), "shared lock override already set");
        state.extend(kinds.iter().copied());
    });

    let _reset = ResetGuard;
    callback()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // Windows allows within-process re-locking on the same file region; cross-process locking
    // still works, but cannot be tested inside a single test binary.
    #[cfg(unix)]
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

    #[test]
    fn shared_lock_retries_retryable_failures_until_it_succeeds() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");

        with_shared_lock_errors_injected(
            &[io::ErrorKind::WouldBlock, io::ErrorKind::Interrupted],
            || {
                let _lock = lock_shared(&paths).expect("shared lock after retry");
            },
        );
    }

    #[test]
    fn shared_lock_surfaces_non_retryable_failures() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");

        with_shared_lock_errors_injected(&[io::ErrorKind::Other], || {
            assert!(matches!(lock_shared(&paths), Err(CoreError::Io { .. })));
        });
    }
}

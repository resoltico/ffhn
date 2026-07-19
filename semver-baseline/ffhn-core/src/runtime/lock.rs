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
    /// Contention preserves the native error until the caller chooses its public carrier.
    Unavailable(std::io::Error),
    Io(CoreError),
}

pub(super) fn lock_exclusive(paths: &TargetPaths) -> Result<TargetLock, LockError> {
    let path = paths.run_lock_file();
    let file = open_lock_file(&path).map_err(LockError::Io)?;
    match file.try_lock_exclusive() {
        Ok(()) => Ok(TargetLock(file)),
        Err(error) => Err(exclusive_lock_error(&path, error)),
    }
}

pub(super) fn lock_shared(paths: &TargetPaths) -> Result<TargetLock, CoreError> {
    lock_shared_with_attempt(
        paths,
        FileExt::try_lock_shared,
        pause_after_shared_lock_retry,
    )
}

/// Acquires the shared lock while observing each actual transient acquisition failure.
///
/// This is test-only because production does not expose synchronization progress. The observer
/// runs only after the operating-system lock operation returned a classified retryable failure,
/// which lets the status concurrency scenario coordinate on a real blocked acquisition rather
/// than on scheduler timing before the attempt.
#[cfg(test)]
pub(in crate::runtime) fn lock_shared_with_retry_observer_for_test<Observe>(
    paths: &TargetPaths,
    mut observe_retry: Observe,
) -> Result<TargetLock, CoreError>
where
    Observe: FnMut(),
{
    lock_shared_with_attempt(paths, FileExt::try_lock_shared, || {
        observe_retry();
        pause_after_shared_lock_retry();
    })
}

/// Acquires a shared lock, retrying only a classified transient acquisition failure.
///
/// The production caller supplies the platform lock operation and the bounded retry pause. Keeping
/// those two effects at this private boundary lets the lock contract prove that a retry occurs
/// before another acquisition attempt, without relying on scheduler timing or a platform's
/// process-local advisory-lock semantics.
fn lock_shared_with_attempt<Acquire, Pause>(
    paths: &TargetPaths,
    mut acquire: Acquire,
    mut pause: Pause,
) -> Result<TargetLock, CoreError>
where
    Acquire: FnMut(&File) -> std::io::Result<()>,
    Pause: FnMut(),
{
    let path = paths.run_lock_file();
    let file = open_lock_file(&path)?;
    loop {
        match classify_shared_lock_attempt(acquire(&file), &path)? {
            SharedLockAttempt::Acquired => return Ok(TargetLock(file)),
            SharedLockAttempt::Retry => pause(),
        }
    }
}

fn pause_after_shared_lock_retry() {
    thread::sleep(Duration::from_millis(5));
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

fn exclusive_lock_error(path: &std::path::Path, error: std::io::Error) -> LockError {
    if lock_is_unavailable(&error) {
        LockError::Unavailable(error)
    } else {
        LockError::Io(CoreError::io(path, error))
    }
}

fn shared_lock_should_retry(error: &std::io::Error) -> bool {
    lock_is_unavailable(error) || error.kind() == std::io::ErrorKind::Interrupted
}

#[derive(Debug)]
enum SharedLockAttempt {
    Acquired,
    Retry,
}

fn classify_shared_lock_attempt(
    result: std::io::Result<()>,
    path: &std::path::Path,
) -> Result<SharedLockAttempt, CoreError> {
    match result {
        Ok(()) => Ok(SharedLockAttempt::Acquired),
        Err(error) if shared_lock_should_retry(&error) => Ok(SharedLockAttempt::Retry),
        Err(error) => Err(CoreError::io(path, error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn lock_error_classification_preserves_contention_and_io_failures() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let path = temporary.path().join("demo.lock");

        for outcome in [
            exclusive_lock_error(&path, std::io::Error::from(std::io::ErrorKind::WouldBlock)),
            exclusive_lock_error(&path, std::io::Error::other("lock backend refused")),
        ] {
            match outcome {
                LockError::Unavailable(error) => {
                    assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
                }
                LockError::Io(error) => {
                    assert!(error.to_string().contains("lock backend refused"));
                }
            }
        }
        assert!(matches!(
            classify_shared_lock_attempt(Ok(()), &path),
            Ok(SharedLockAttempt::Acquired)
        ));
        assert!(matches!(
            classify_shared_lock_attempt(
                Err(std::io::Error::from(std::io::ErrorKind::WouldBlock)),
                &path
            ),
            Ok(SharedLockAttempt::Retry)
        ));
        assert!(matches!(
            classify_shared_lock_attempt(
                Err(std::io::Error::from(std::io::ErrorKind::Interrupted)),
                &path
            ),
            Ok(SharedLockAttempt::Retry)
        ));
        assert!(
            classify_shared_lock_attempt(
                Err(std::io::Error::other("shared lock backend refused")),
                &path
            )
            .expect_err("non-retryable shared lock errors must surface")
            .to_string()
            .contains("shared lock backend refused")
        );
    }

    #[test]
    fn shared_lock_retries_once_before_acquiring_the_same_regular_file() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let paths = TargetPaths::try_new(temporary.path(), "demo").expect("paths");
        let attempts = Cell::new(0_u8);
        let pauses = Cell::new(0_u8);

        let lock = lock_shared_with_attempt(
            &paths,
            |file| {
                let attempt = attempts.get();
                attempts.set(attempt + 1);
                if attempt == 0 {
                    Err(std::io::Error::from(std::io::ErrorKind::WouldBlock))
                } else {
                    FileExt::try_lock_shared(file)
                }
            },
            || pauses.set(pauses.get() + 1),
        )
        .expect("shared lock after retry");

        assert_eq!(attempts.get(), 2);
        assert_eq!(pauses.get(), 1);
        drop(lock);
    }

    #[test]
    fn shared_lock_retry_pause_is_the_production_bounded_wait() {
        pause_after_shared_lock_retry();
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_unknown_lock_errors_are_not_contention() {
        assert!(!lock_is_unavailable(&std::io::Error::other(
            "unrelated lock failure"
        )));
    }

    #[cfg(windows)]
    #[test]
    fn windows_lock_sharing_codes_are_classified_as_unavailable() {
        for code in [32, 33] {
            assert!(lock_is_unavailable(&std::io::Error::from_raw_os_error(
                code
            )));
        }
        assert!(!lock_is_unavailable(&std::io::Error::from_raw_os_error(5)));
    }

    #[cfg(unix)]
    #[test]
    fn lock_node_metadata_failures_are_not_hidden() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let lock_root = temporary.path().join("locked");
        fs::create_dir(&lock_root).expect("lock root");
        let original_permissions = fs::metadata(&lock_root)
            .expect("lock root metadata")
            .permissions();
        fs::set_permissions(&lock_root, fs::Permissions::from_mode(0o000))
            .expect("restrict lock root");
        let error = open_lock_file(&lock_root.join("demo.lock"));
        fs::set_permissions(&lock_root, original_permissions).expect("restore lock root");

        assert!(error.is_err());
    }
}

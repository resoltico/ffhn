//! Directory durability boundary shared by graph-state commit protocols.

use std::path::Path;

use cap_std::fs::Dir;

use crate::CoreError;

#[cfg(test)]
thread_local! {
    static DIRECTORY_SYNC_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(in crate::graph) fn sync_directory(dir: &Dir, path: &Path) -> Result<(), CoreError> {
    #[cfg(test)]
    DIRECTORY_SYNC_COUNT.set(DIRECTORY_SYNC_COUNT.get() + 1);

    // Windows does not provide directory-handle flush semantics through cap-std. File payloads
    // are still synchronized before their atomic replacement; attempting a directory `sync_all`
    // here instead turns successful writes into `AccessDenied` failures on supported Windows.
    #[cfg(windows)]
    {
        let _ = (dir, path);
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let handle = dir.open(".").map_err(|error| CoreError::io(path, error))?;
        handle
            .sync_all()
            .map_err(|error| CoreError::io(path, error))
    }
}

#[cfg(test)]
pub(super) fn take_directory_sync_count() -> usize {
    DIRECTORY_SYNC_COUNT.replace(0)
}

//! Host-neutral fixtures for core unit tests.

/// Returns an absolute file path that the current host recognizes as native.
pub(crate) fn absolute_file_path(file_name: &str) -> String {
    #[cfg(windows)]
    {
        format!("C:/ffhn-test/{file_name}")
    }

    #[cfg(not(windows))]
    {
        format!("/tmp/ffhn-test/{file_name}")
    }
}

/// A native absolute executable path used only by configuration-model tests.
#[cfg(windows)]
pub(crate) const PROCESS_PROGRAM: &str = "C:/Windows/System32/cmd.exe";

/// A native absolute executable path used only by configuration-model tests.
#[cfg(not(windows))]
pub(crate) const PROCESS_PROGRAM: &str = "/bin/sh";

/// Arguments for a successful no-op process on the current host.
#[cfg(windows)]
pub(crate) const SUCCESSFUL_PROCESS_ARGS: &[&str] = &["/C", "exit 0"];

/// Arguments for a successful no-op process on the current host.
#[cfg(not(windows))]
pub(crate) const SUCCESSFUL_PROCESS_ARGS: &[&str] = &["-c", "exit 0"];

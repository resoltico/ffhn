//! Closed operating-system I/O classification for serialized FFHN diagnostics.

use std::io;

use serde::{Deserialize, Serialize};

/// FFHN-owned classification for I/O error kinds observed at native boundaries.
///
/// The mapping intentionally classifies every `std::io::ErrorKind` that FFHN handles directly.
/// Any future non-exhaustive standard-library variant maps to [`Self::Other`] rather than a
/// rendered foreign string.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IoErrorClass {
    /// A named resource was absent.
    NotFound,
    /// Access was denied.
    PermissionDenied,
    /// A connection was refused.
    ConnectionRefused,
    /// A connection was reset.
    ConnectionReset,
    /// The host could not be reached.
    HostUnreachable,
    /// The network destination could not be reached.
    NetworkUnreachable,
    /// A connection was aborted.
    ConnectionAborted,
    /// No connection was present.
    NotConnected,
    /// The address is already in use.
    AddrInUse,
    /// The address is unavailable.
    AddrNotAvailable,
    /// The network is down.
    NetworkDown,
    /// A pipe was broken.
    BrokenPipe,
    /// The resource already exists.
    AlreadyExists,
    /// The operation would block.
    WouldBlock,
    /// A path component was not a directory.
    NotADirectory,
    /// A path named a directory where a file was required.
    IsADirectory,
    /// A directory was not empty.
    DirectoryNotEmpty,
    /// The filesystem is read-only.
    ReadOnlyFilesystem,
    /// A stale network file handle was observed.
    StaleNetworkFileHandle,
    /// Input was invalid.
    InvalidInput,
    /// Data was invalid.
    InvalidData,
    /// The operation timed out.
    TimedOut,
    /// A write made no progress.
    WriteZero,
    /// Storage is full.
    StorageFull,
    /// The resource cannot seek.
    NotSeekable,
    /// A quota was exceeded.
    QuotaExceeded,
    /// A file was too large.
    FileTooLarge,
    /// The resource is busy.
    ResourceBusy,
    /// An executable file is busy.
    ExecutableFileBusy,
    /// A deadlock was detected.
    Deadlock,
    /// A filesystem boundary was crossed.
    CrossesDevices,
    /// Too many symbolic links were followed.
    TooManyLinks,
    /// A filename was invalid.
    InvalidFilename,
    /// The argument list was too long.
    ArgumentListTooLong,
    /// The operation was interrupted.
    Interrupted,
    /// The operation is unsupported.
    Unsupported,
    /// End-of-file arrived unexpectedly.
    UnexpectedEof,
    /// Memory allocation failed.
    OutOfMemory,
    /// A recognized generic OS error occurred.
    Other,
}

impl IoErrorClass {
    /// Classifies the supplied I/O error without rendering it into the public schema.
    pub(crate) fn from_error(error: &io::Error) -> Self {
        Self::from_kind(error.kind())
    }

    /// Maps all currently handled standard-library error kinds to the closed FFHN vocabulary.
    pub(crate) const fn from_kind(kind: io::ErrorKind) -> Self {
        match kind {
            io::ErrorKind::NotFound => Self::NotFound,
            io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            io::ErrorKind::ConnectionRefused => Self::ConnectionRefused,
            io::ErrorKind::ConnectionReset => Self::ConnectionReset,
            io::ErrorKind::HostUnreachable => Self::HostUnreachable,
            io::ErrorKind::NetworkUnreachable => Self::NetworkUnreachable,
            io::ErrorKind::ConnectionAborted => Self::ConnectionAborted,
            io::ErrorKind::NotConnected => Self::NotConnected,
            io::ErrorKind::AddrInUse => Self::AddrInUse,
            io::ErrorKind::AddrNotAvailable => Self::AddrNotAvailable,
            io::ErrorKind::NetworkDown => Self::NetworkDown,
            io::ErrorKind::BrokenPipe => Self::BrokenPipe,
            io::ErrorKind::AlreadyExists => Self::AlreadyExists,
            io::ErrorKind::WouldBlock => Self::WouldBlock,
            io::ErrorKind::NotADirectory => Self::NotADirectory,
            io::ErrorKind::IsADirectory => Self::IsADirectory,
            io::ErrorKind::DirectoryNotEmpty => Self::DirectoryNotEmpty,
            io::ErrorKind::ReadOnlyFilesystem => Self::ReadOnlyFilesystem,
            io::ErrorKind::StaleNetworkFileHandle => Self::StaleNetworkFileHandle,
            io::ErrorKind::InvalidInput => Self::InvalidInput,
            io::ErrorKind::InvalidData => Self::InvalidData,
            io::ErrorKind::TimedOut => Self::TimedOut,
            io::ErrorKind::WriteZero => Self::WriteZero,
            io::ErrorKind::StorageFull => Self::StorageFull,
            io::ErrorKind::NotSeekable => Self::NotSeekable,
            io::ErrorKind::QuotaExceeded => Self::QuotaExceeded,
            io::ErrorKind::FileTooLarge => Self::FileTooLarge,
            io::ErrorKind::ResourceBusy => Self::ResourceBusy,
            io::ErrorKind::ExecutableFileBusy => Self::ExecutableFileBusy,
            io::ErrorKind::Deadlock => Self::Deadlock,
            io::ErrorKind::CrossesDevices => Self::CrossesDevices,
            io::ErrorKind::TooManyLinks => Self::TooManyLinks,
            io::ErrorKind::InvalidFilename => Self::InvalidFilename,
            io::ErrorKind::ArgumentListTooLong => Self::ArgumentListTooLong,
            io::ErrorKind::Interrupted => Self::Interrupted,
            io::ErrorKind::Unsupported => Self::Unsupported,
            io::ErrorKind::UnexpectedEof => Self::UnexpectedEof,
            io::ErrorKind::OutOfMemory => Self::OutOfMemory,
            io::ErrorKind::Other => Self::Other,
            _ => Self::Other,
        }
    }

    /// Returns the stable report-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::ConnectionRefused => "connection_refused",
            Self::ConnectionReset => "connection_reset",
            Self::HostUnreachable => "host_unreachable",
            Self::NetworkUnreachable => "network_unreachable",
            Self::ConnectionAborted => "connection_aborted",
            Self::NotConnected => "not_connected",
            Self::AddrInUse => "addr_in_use",
            Self::AddrNotAvailable => "addr_not_available",
            Self::NetworkDown => "network_down",
            Self::BrokenPipe => "broken_pipe",
            Self::AlreadyExists => "already_exists",
            Self::WouldBlock => "would_block",
            Self::NotADirectory => "not_a_directory",
            Self::IsADirectory => "is_a_directory",
            Self::DirectoryNotEmpty => "directory_not_empty",
            Self::ReadOnlyFilesystem => "read_only_filesystem",
            Self::StaleNetworkFileHandle => "stale_network_file_handle",
            Self::InvalidInput => "invalid_input",
            Self::InvalidData => "invalid_data",
            Self::TimedOut => "timed_out",
            Self::WriteZero => "write_zero",
            Self::StorageFull => "storage_full",
            Self::NotSeekable => "not_seekable",
            Self::QuotaExceeded => "quota_exceeded",
            Self::FileTooLarge => "file_too_large",
            Self::ResourceBusy => "resource_busy",
            Self::ExecutableFileBusy => "executable_file_busy",
            Self::Deadlock => "deadlock",
            Self::CrossesDevices => "crosses_devices",
            Self::TooManyLinks => "too_many_links",
            Self::InvalidFilename => "invalid_filename",
            Self::ArgumentListTooLong => "argument_list_too_long",
            Self::Interrupted => "interrupted",
            Self::Unsupported => "unsupported",
            Self::UnexpectedEof => "unexpected_eof",
            Self::OutOfMemory => "out_of_memory",
            Self::Other => "other",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::IoErrorClass;

    #[test]
    fn every_supported_native_kind_has_one_stable_closed_classification() {
        for (kind, expected, spelling) in [
            (io::ErrorKind::NotFound, IoErrorClass::NotFound, "not_found"),
            (
                io::ErrorKind::PermissionDenied,
                IoErrorClass::PermissionDenied,
                "permission_denied",
            ),
            (
                io::ErrorKind::ConnectionRefused,
                IoErrorClass::ConnectionRefused,
                "connection_refused",
            ),
            (
                io::ErrorKind::ConnectionReset,
                IoErrorClass::ConnectionReset,
                "connection_reset",
            ),
            (
                io::ErrorKind::HostUnreachable,
                IoErrorClass::HostUnreachable,
                "host_unreachable",
            ),
            (
                io::ErrorKind::NetworkUnreachable,
                IoErrorClass::NetworkUnreachable,
                "network_unreachable",
            ),
            (
                io::ErrorKind::ConnectionAborted,
                IoErrorClass::ConnectionAborted,
                "connection_aborted",
            ),
            (
                io::ErrorKind::NotConnected,
                IoErrorClass::NotConnected,
                "not_connected",
            ),
            (
                io::ErrorKind::AddrInUse,
                IoErrorClass::AddrInUse,
                "addr_in_use",
            ),
            (
                io::ErrorKind::AddrNotAvailable,
                IoErrorClass::AddrNotAvailable,
                "addr_not_available",
            ),
            (
                io::ErrorKind::NetworkDown,
                IoErrorClass::NetworkDown,
                "network_down",
            ),
            (
                io::ErrorKind::BrokenPipe,
                IoErrorClass::BrokenPipe,
                "broken_pipe",
            ),
            (
                io::ErrorKind::AlreadyExists,
                IoErrorClass::AlreadyExists,
                "already_exists",
            ),
            (
                io::ErrorKind::WouldBlock,
                IoErrorClass::WouldBlock,
                "would_block",
            ),
            (
                io::ErrorKind::NotADirectory,
                IoErrorClass::NotADirectory,
                "not_a_directory",
            ),
            (
                io::ErrorKind::IsADirectory,
                IoErrorClass::IsADirectory,
                "is_a_directory",
            ),
            (
                io::ErrorKind::DirectoryNotEmpty,
                IoErrorClass::DirectoryNotEmpty,
                "directory_not_empty",
            ),
            (
                io::ErrorKind::ReadOnlyFilesystem,
                IoErrorClass::ReadOnlyFilesystem,
                "read_only_filesystem",
            ),
            (
                io::ErrorKind::StaleNetworkFileHandle,
                IoErrorClass::StaleNetworkFileHandle,
                "stale_network_file_handle",
            ),
            (
                io::ErrorKind::InvalidInput,
                IoErrorClass::InvalidInput,
                "invalid_input",
            ),
            (
                io::ErrorKind::InvalidData,
                IoErrorClass::InvalidData,
                "invalid_data",
            ),
            (io::ErrorKind::TimedOut, IoErrorClass::TimedOut, "timed_out"),
            (
                io::ErrorKind::WriteZero,
                IoErrorClass::WriteZero,
                "write_zero",
            ),
            (
                io::ErrorKind::StorageFull,
                IoErrorClass::StorageFull,
                "storage_full",
            ),
            (
                io::ErrorKind::NotSeekable,
                IoErrorClass::NotSeekable,
                "not_seekable",
            ),
            (
                io::ErrorKind::QuotaExceeded,
                IoErrorClass::QuotaExceeded,
                "quota_exceeded",
            ),
            (
                io::ErrorKind::FileTooLarge,
                IoErrorClass::FileTooLarge,
                "file_too_large",
            ),
            (
                io::ErrorKind::ResourceBusy,
                IoErrorClass::ResourceBusy,
                "resource_busy",
            ),
            (
                io::ErrorKind::ExecutableFileBusy,
                IoErrorClass::ExecutableFileBusy,
                "executable_file_busy",
            ),
            (io::ErrorKind::Deadlock, IoErrorClass::Deadlock, "deadlock"),
            (
                io::ErrorKind::CrossesDevices,
                IoErrorClass::CrossesDevices,
                "crosses_devices",
            ),
            (
                io::ErrorKind::TooManyLinks,
                IoErrorClass::TooManyLinks,
                "too_many_links",
            ),
            (
                io::ErrorKind::InvalidFilename,
                IoErrorClass::InvalidFilename,
                "invalid_filename",
            ),
            (
                io::ErrorKind::ArgumentListTooLong,
                IoErrorClass::ArgumentListTooLong,
                "argument_list_too_long",
            ),
            (
                io::ErrorKind::Interrupted,
                IoErrorClass::Interrupted,
                "interrupted",
            ),
            (
                io::ErrorKind::Unsupported,
                IoErrorClass::Unsupported,
                "unsupported",
            ),
            (
                io::ErrorKind::UnexpectedEof,
                IoErrorClass::UnexpectedEof,
                "unexpected_eof",
            ),
            (
                io::ErrorKind::OutOfMemory,
                IoErrorClass::OutOfMemory,
                "out_of_memory",
            ),
            (io::ErrorKind::Other, IoErrorClass::Other, "other"),
        ] {
            assert_eq!(IoErrorClass::from_kind(kind), expected);
            assert_eq!(IoErrorClass::from_error(&io::Error::from(kind)), expected);
            assert_eq!(expected.as_str(), spelling);
        }
    }

    #[cfg(target_os = "macos")]
    const IN_PROGRESS_RAW_OS_ERROR: i32 = 36;
    #[cfg(target_os = "linux")]
    const IN_PROGRESS_RAW_OS_ERROR: i32 = 115;
    #[cfg(windows)]
    const IN_PROGRESS_RAW_OS_ERROR: i32 = 10_036;

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    fn a_platform_specific_future_native_kind_fails_closed_as_other() {
        let error = io::Error::from_raw_os_error(IN_PROGRESS_RAW_OS_ERROR);
        assert_eq!(IoErrorClass::from_error(&error), IoErrorClass::Other);
    }
}

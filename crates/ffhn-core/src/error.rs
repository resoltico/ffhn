use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Process-level FFHN errors raised before a structured schema document can be returned.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreError {
    /// Filesystem access failed.
    #[error("filesystem error at {path}: {source}")]
    Io {
        /// Path associated with the failure.
        path: PathBuf,
        /// Underlying I/O error.
        source: io::Error,
    },
    /// JSON serialization or deserialization failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// TOML parsing failed.
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),
    /// URL parsing failed.
    #[error("url parse error: {0}")]
    Url(#[from] url::ParseError),
    /// Time formatting failed.
    #[error("time formatting error: {0}")]
    TimeFormat(#[from] time::error::Format),
    /// Time parsing failed.
    #[error("time parsing error: {0}")]
    TimeParse(#[from] time::error::Parse),
    /// FFHN rejected contract data before a structured document could be emitted.
    #[error("contract error: {0}")]
    Contract(String),
    /// FFHN hit an internal invariant failure.
    #[error("internal error: {0}")]
    Internal(String),
}

impl CoreError {
    /// Builds one path-aware filesystem error.
    pub fn io(path: impl AsRef<Path>, source: io::Error) -> Self {
        Self::Io {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    /// Builds one FFHN contract error.
    pub fn contract(message: impl Into<String>) -> Self {
        Self::Contract(message.into())
    }

    /// Builds one internal FFHN invariant error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

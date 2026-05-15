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
    /// FFHN could not complete the HTMLCut interoperability boundary safely.
    #[error("htmlcut interop error: {0}")]
    HtmlcutInterop(String),
    /// FFHN hit an internal invariant failure.
    #[error("internal error: {0}")]
    Internal(String),
    /// FFHN could not complete one persistence transaction cleanly.
    #[error("{summary}")]
    PersistTransaction {
        /// Human-readable transaction summary preserving primary, rollback, and cleanup failures.
        summary: String,
        /// Primary failure that forced the transaction to unwind.
        primary: Box<CoreError>,
        /// Rollback failure when one occurred.
        rollback: Option<Box<CoreError>>,
        /// Best-effort cleanup failures captured after rollback.
        cleanup: Vec<CoreError>,
    },
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

    /// Builds one HTMLCut interoperability error.
    pub fn htmlcut_interop(message: impl Into<String>) -> Self {
        Self::HtmlcutInterop(message.into())
    }

    /// Builds one internal FFHN invariant error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    pub(crate) fn persist_transaction(
        primary: CoreError,
        rollback: Option<CoreError>,
        cleanup: Vec<CoreError>,
    ) -> Self {
        let mut summary = format!("primary persist failure: {primary}");
        if let Some(rollback_error) = &rollback {
            summary.push_str(&format!("; rollback failure: {rollback_error}"));
        }
        if !cleanup.is_empty() {
            let cleanup_summary = cleanup
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            summary.push_str(&format!("; cleanup failures: {cleanup_summary}"));
        }

        Self::PersistTransaction {
            summary,
            primary: Box::new(primary),
            rollback: rollback.map(Box::new),
            cleanup,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_persist_transaction(
        error: CoreError,
    ) -> (
        String,
        Box<CoreError>,
        Option<Box<CoreError>>,
        Vec<CoreError>,
    ) {
        match error {
            CoreError::PersistTransaction {
                summary,
                primary,
                rollback,
                cleanup,
            } => (summary, primary, rollback, cleanup),
            other => panic!("expected persist transaction error, got {other}"),
        }
    }

    #[test]
    fn persist_transaction_summary_omits_optional_sections_when_absent() {
        let error = CoreError::persist_transaction(
            CoreError::io(
                "/tmp/watch/demo/state.json",
                std::io::Error::other("write failed"),
            ),
            None,
            Vec::new(),
        );

        let (summary, primary, rollback, cleanup) = expect_persist_transaction(error);

        assert_eq!(
            summary,
            "primary persist failure: filesystem error at /tmp/watch/demo/state.json: write failed"
        );
        assert!(matches!(*primary, CoreError::Io { .. }));
        assert!(rollback.is_none());
        assert!(cleanup.is_empty());
    }

    #[test]
    #[should_panic(expected = "expected persist transaction error")]
    fn expect_persist_transaction_panics_for_non_composite_errors() {
        let _ = expect_persist_transaction(CoreError::contract("no composite error here"));
    }
}

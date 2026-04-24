use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Validated forward-slash relative artifact path stored inside FFHN documents.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelativeArtifactPath(String);

impl RelativeArtifactPath {
    /// Parses one FFHN-owned relative artifact path.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        validate_relative_artifact_path(&value)?;
        Ok(Self(value))
    }

    /// Returns the canonical serialized relative path.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the path view used for filesystem joins.
    pub fn as_path(&self) -> &Path {
        Path::new(self.as_str())
    }
}

impl TryFrom<String> for RelativeArtifactPath {
    type Error = CoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for RelativeArtifactPath {
    type Error = CoreError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl FromStr for RelativeArtifactPath {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl From<RelativeArtifactPath> for String {
    fn from(value: RelativeArtifactPath) -> Self {
        value.0
    }
}

impl AsRef<str> for RelativeArtifactPath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<Path> for RelativeArtifactPath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl fmt::Display for RelativeArtifactPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn validate_relative_artifact_path(path: &str) -> Result<(), CoreError> {
    super::super::validate::require_non_empty("artifact path", path)?;

    if path.starts_with('/') || path.starts_with('\\') {
        return Err(invalid_relative_artifact_path());
    }
    if has_windows_drive_prefix(path) || path.contains('\\') {
        return Err(invalid_relative_artifact_path());
    }

    for segment in path.split('/') {
        if segment.is_empty() || matches!(segment, "." | "..") {
            return Err(invalid_relative_artifact_path());
        }
    }

    Ok(())
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn invalid_relative_artifact_path() -> CoreError {
    CoreError::contract(
        "artifact paths must use forward-slash relative paths without empty, '.' , or '..' segments",
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::str::FromStr;

    use super::RelativeArtifactPath;

    #[test]
    fn relative_artifact_paths_accept_forward_slash_relative_files() {
        RelativeArtifactPath::new("snapshots/current/file.txt").expect("relative artifact path");
    }

    #[test]
    fn relative_artifact_paths_reject_unix_and_windows_absolute_or_escape_forms() {
        for invalid in [
            "",
            "../escape",
            "/absolute",
            "snapshots//current/file.txt",
            "snapshots/./current/file.txt",
            "snapshots/current/../file.txt",
            r"C:\outside\file.txt",
            r"D:outside/file.txt",
            r"\\server\share\file.txt",
            r"snapshots\current\file.txt",
        ] {
            assert!(
                RelativeArtifactPath::new(invalid).is_err(),
                "{invalid} should be rejected"
            );
        }
    }

    #[test]
    fn relative_artifact_paths_deserialize_rejects_invalid_values() {
        let parsed = serde_json::from_str::<RelativeArtifactPath>("\"snapshots/current/file.txt\"")
            .expect("relative path json");
        assert_eq!(parsed.as_str(), "snapshots/current/file.txt");
        assert!(
            serde_json::from_str::<RelativeArtifactPath>("\"C:\\\\outside\\\\file.txt\"").is_err()
        );
    }

    #[test]
    fn relative_artifact_paths_support_standard_string_conversions() {
        let parsed = RelativeArtifactPath::try_from("snapshots/current/file.txt")
            .expect("relative artifact path");
        assert_eq!(parsed.as_path(), Path::new("snapshots/current/file.txt"));
        assert_eq!(parsed.as_str(), "snapshots/current/file.txt");
        assert_eq!(
            <RelativeArtifactPath as AsRef<str>>::as_ref(&parsed),
            "snapshots/current/file.txt"
        );
        assert_eq!(
            <RelativeArtifactPath as AsRef<Path>>::as_ref(&parsed),
            Path::new("snapshots/current/file.txt")
        );
        assert_eq!(parsed.to_string(), "snapshots/current/file.txt");

        let from_string = RelativeArtifactPath::try_from("snapshots/current/file.txt".to_owned())
            .expect("owned relative artifact path");
        assert_eq!(from_string, parsed);

        let from_str =
            RelativeArtifactPath::from_str("snapshots/current/file.txt").expect("from str");
        assert_eq!(from_str, parsed);

        let serialized: String = parsed.clone().into();
        assert_eq!(serialized, "snapshots/current/file.txt");
    }

    #[test]
    fn windows_drive_prefix_detection_requires_ascii_letter_colon_prefix() {
        assert!(super::has_windows_drive_prefix(
            "C:/snapshots/current/file.txt"
        ));
        assert!(!super::has_windows_drive_prefix("x"));
        assert!(!super::has_windows_drive_prefix(
            "1:/snapshots/current/file.txt"
        ));
        assert!(!super::has_windows_drive_prefix(
            "snapshots/current/file.txt"
        ));
    }
}

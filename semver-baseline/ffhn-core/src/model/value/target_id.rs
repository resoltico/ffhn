use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::CoreError;

const TARGET_ID_PATTERN: &str = r"^[a-z0-9][a-z0-9_-]{0,63}$";
const RESERVED_TARGET_IDS: &[&str] = &[
    "aux", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "con", "lpt1",
    "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9", "nul", "prn",
];

/// Validated FFHN target id.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TargetId(String);

impl TargetId {
    /// Parses one target id under FFHN's durable filesystem contract.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        validate_target_id_value(&value)?;
        Ok(Self(value))
    }

    /// Returns the canonical target-id string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TargetId {
    type Error = CoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for TargetId {
    type Error = CoreError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl FromStr for TargetId {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl From<TargetId> for String {
    fn from(value: TargetId) -> Self {
        value.0
    }
}

impl AsRef<str> for TargetId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for TargetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn validate_target_id_value(target_id: &str) -> Result<(), CoreError> {
    super::super::validate::require_non_empty("target_id", target_id)?;
    if RESERVED_TARGET_IDS.contains(&target_id) {
        return Err(CoreError::contract(
            "target_id must not use a reserved filesystem device name",
        ));
    }
    let regex = target_id_regex()?;
    if !regex.is_match(target_id) {
        return Err(invalid_target_id_error());
    }
    if target_id.ends_with(['-', '_']) || target_id.contains("--") || target_id.contains("__") {
        return Err(invalid_target_id_error());
    }
    let mut previous_separator = false;
    for ch in target_id.chars() {
        let separator = matches!(ch, '-' | '_');
        if separator && previous_separator {
            return Err(invalid_target_id_error());
        }
        previous_separator = separator;
    }
    Ok(())
}

fn invalid_target_id_error() -> CoreError {
    CoreError::contract(
        "target_id must start with [a-z0-9], stay within 64 chars, and only use single internal '-' or '_' separators",
    )
}

fn target_id_regex() -> Result<&'static Regex, CoreError> {
    static TARGET_ID_REGEX: OnceLock<Result<Regex, String>> = OnceLock::new();
    TARGET_ID_REGEX
        .get_or_init(|| Regex::new(TARGET_ID_PATTERN).map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|error| CoreError::internal(format!("target id regex failed to compile: {error}")))
}

#[cfg(test)]
mod tests {
    use super::TargetId;

    #[test]
    fn target_id_validation_enforces_the_canonical_pattern() {
        TargetId::new("demo_1").expect("valid target id");
        TargetId::new("demo-1").expect("valid target id");
        assert!(TargetId::new("Demo").is_err());
        assert!(TargetId::new("").is_err());
        assert!(TargetId::new("demo__1").is_err());
        assert!(TargetId::new("demo--1").is_err());
        assert!(TargetId::new("demo-_1").is_err());
        assert!(TargetId::new("demo-").is_err());
        assert!(TargetId::new("demo_").is_err());
        assert!(TargetId::new("con").is_err());
    }

    #[test]
    fn target_id_deserialize_rejects_invalid_values() {
        let parsed = serde_json::from_str::<TargetId>("\"demo-1\"").expect("target id json");
        assert_eq!(parsed.as_str(), "demo-1");
        assert!(serde_json::from_str::<TargetId>("\"Demo\"").is_err());
    }
}

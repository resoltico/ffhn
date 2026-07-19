use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::CoreError;

use super::event::RouteFamily;

/// A stable target-local identifier for one delivery route.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RouteId(String);

impl RouteId {
    /// Parses one stable target-local route identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        validate_route_id(&value)?;
        Ok(Self(value))
    }

    /// Returns the canonical route identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RouteId {
    type Error = CoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<RouteId> for String {
    fn from(value: RouteId) -> Self {
        value.0
    }
}

impl FromStr for RouteId {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl AsRef<str> for RouteId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for RouteId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Bounded operational settings for pending delivery records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutboxPolicy {
    max_pending: usize,
    max_attempts: u32,
    base_backoff_ms: u64,
    max_backoff_ms: u64,
}

impl Default for OutboxPolicy {
    fn default() -> Self {
        Self {
            max_pending: 100,
            max_attempts: 5,
            base_backoff_ms: 1_000,
            max_backoff_ms: 300_000,
        }
    }
}

impl OutboxPolicy {
    /// Validates the bounded deterministic retry policy.
    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        if !(1..=100_000).contains(&self.max_pending) {
            return Err(CoreError::contract(
                "outbox.max_pending must be in 1..=100000",
            ));
        }
        if !(1..=100).contains(&self.max_attempts) {
            return Err(CoreError::contract(
                "outbox.max_attempts must be in 1..=100",
            ));
        }
        if !(1..=86_400_000).contains(&self.base_backoff_ms) {
            return Err(CoreError::contract(
                "outbox.base_backoff_ms must be in 1..=86400000",
            ));
        }
        if !(self.base_backoff_ms..=604_800_000).contains(&self.max_backoff_ms) {
            return Err(CoreError::contract(
                "outbox.max_backoff_ms must be at least base_backoff_ms and at most 604800000",
            ));
        }
        Ok(())
    }

    /// Returns the maximum number of pending records retained for one target.
    pub const fn max_pending(&self) -> usize {
        self.max_pending
    }

    /// Returns the number of failed attempts that terminally dead-letters a record.
    pub const fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    /// Returns the deterministic initial retry delay in milliseconds.
    pub const fn base_backoff_ms(&self) -> u64 {
        self.base_backoff_ms
    }

    /// Returns the deterministic maximum retry delay in milliseconds.
    pub const fn max_backoff_ms(&self) -> u64 {
        self.max_backoff_ms
    }
}

/// One target-local delivery route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryRoute {
    route_id: RouteId,
    route_family: RouteFamily,
    adapter: DeliveryAdapter,
}

impl DeliveryRoute {
    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        self.adapter.validate()
    }

    /// Returns the stable target-local route identifier.
    pub fn route_id(&self) -> &str {
        self.route_id.as_str()
    }

    /// Returns the family of events accepted by this route.
    pub const fn route_family(&self) -> RouteFamily {
        self.route_family
    }

    pub(crate) const fn id(&self) -> &RouteId {
        &self.route_id
    }

    pub(crate) const fn adapter(&self) -> &DeliveryAdapter {
        &self.adapter
    }
}

/// The supported durable-delivery adapter vocabulary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeliveryAdapter {
    /// Writes one immutable structured JSON payload and a newline to a process's standard input.
    ProcessStdin {
        /// Absolute executable path.
        program: String,
        /// Exact argument vector supplied to the executable.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
        /// Maximum delivery-process lifetime in milliseconds.
        timeout_ms: u64,
    },
}

impl DeliveryAdapter {
    pub(super) fn validate(&self) -> Result<(), CoreError> {
        match self {
            Self::ProcessStdin {
                program,
                args,
                timeout_ms,
            } => {
                require_text("routes.adapter.program", program)?;
                if !Path::new(program).is_absolute() {
                    return Err(CoreError::contract(
                        "routes.adapter.program must be an absolute path",
                    ));
                }
                for argument in args {
                    require_text("routes.adapter.args entry", argument)?;
                }
                if !(100..=60_000).contains(timeout_ms) {
                    return Err(CoreError::contract(
                        "routes.adapter.timeout_ms must be in 100..=60000",
                    ));
                }
                Ok(())
            }
        }
    }

    pub(crate) fn process_stdin(&self) -> (&str, &[String], u64) {
        match self {
            Self::ProcessStdin {
                program,
                args,
                timeout_ms,
            } => (program, args, *timeout_ms),
        }
    }
}

pub(crate) fn validate_routes(routes: &[DeliveryRoute]) -> Result<(), CoreError> {
    let mut route_ids = BTreeSet::new();
    for route in routes {
        if !route_ids.insert(route.id()) {
            return Err(CoreError::contract("route_id values must be unique"));
        }
        route.validate()?;
    }
    Ok(())
}

fn validate_route_id(value: &str) -> Result<(), CoreError> {
    if value.is_empty()
        || value.len() > 64
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
        || !value.as_bytes()[0].is_ascii_lowercase() && !value.as_bytes()[0].is_ascii_digit()
        || value.ends_with(['-', '_'])
        || value.contains("--")
        || value.contains("__")
        || value.contains("-_")
        || value.contains("_-")
    {
        return Err(CoreError::contract(
            "route_id must start with [a-z0-9], stay within 64 chars, and only use single internal '-' or '_' separators",
        ));
    }
    Ok(())
}

pub(super) fn require_text(field: &str, value: &str) -> Result<(), CoreError> {
    if value.trim().is_empty() {
        Err(CoreError::contract(format!("{field} must not be empty")))
    } else {
        Ok(())
    }
}

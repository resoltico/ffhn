//! Source-owned v11 graph configuration and fetch contract.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize};
use url::Url;

use crate::CoreError;

use super::{
    DeliveryPolicy, GraphRoute, GraphRouteFamily, SourceId, delivery_config::validate_delivery,
};

/// Canonical agent configuration schema name.
pub const AGENT_SCHEMA_NAME: &str = "ffhn.agent";
/// Canonical agent configuration schema version.
pub const AGENT_SCHEMA_VERSION: u32 = 1;
/// Canonical source configuration schema name.
pub const SOURCE_SCHEMA_NAME: &str = "ffhn.source";
/// Canonical source configuration schema version.
pub const SOURCE_SCHEMA_VERSION: u32 = 1;

/// Minimal graph-root agent configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentDocument {
    schema_name: String,
    schema_version: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentDocumentWire {
    schema_name: String,
    schema_version: u32,
}

impl<'de> Deserialize<'de> for AgentDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = AgentDocumentWire::deserialize(deserializer)?;
        let document = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
        };
        document.validate().map_err(serde::de::Error::custom)?;
        Ok(document)
    }
}

impl AgentDocument {
    /// Builds the current empty agent configuration document.
    pub fn new() -> Self {
        Self {
            schema_name: AGENT_SCHEMA_NAME.to_owned(),
            schema_version: AGENT_SCHEMA_VERSION,
        }
    }

    /// Validates the closed agent configuration envelope.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_name == AGENT_SCHEMA_NAME && self.schema_version == AGENT_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(CoreError::contract(
                "agent document is not a current FFHN agent document",
            ))
        }
    }
}

impl Default for AgentDocument {
    fn default() -> Self {
        Self::new()
    }
}

/// Full source configuration for one shared acquisition and its health policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDocument {
    schema_name: String,
    schema_version: u32,
    source_id: SourceId,
    display_name: String,
    enabled: bool,
    escalate_after: u32,
    fetch: SourceFetch,
    conditional: ConditionalRequests,
    schedule: SourceSchedule,
    #[serde(skip_serializing_if = "Option::is_none")]
    outbox: Option<DeliveryPolicy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    routes: Vec<GraphRoute>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceDocumentWire {
    schema_name: String,
    schema_version: u32,
    source_id: SourceId,
    display_name: String,
    enabled: bool,
    escalate_after: u32,
    fetch: SourceFetch,
    conditional: ConditionalRequests,
    schedule: SourceSchedule,
    #[serde(default)]
    outbox: Option<DeliveryPolicy>,
    #[serde(default)]
    routes: Vec<GraphRoute>,
}

impl<'de> Deserialize<'de> for SourceDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SourceDocumentWire::deserialize(deserializer)?;
        let document = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            source_id: wire.source_id,
            display_name: wire.display_name,
            enabled: wire.enabled,
            escalate_after: wire.escalate_after,
            fetch: wire.fetch,
            conditional: wire.conditional,
            schedule: wire.schedule,
            outbox: wire.outbox,
            routes: wire.routes,
        };
        document.validate().map_err(serde::de::Error::custom)?;
        Ok(document)
    }
}

impl SourceDocument {
    /// Returns the validated source identifier.
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    /// Returns the user-visible source display name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns whether the source is eligible for acquisition.
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the source fetch contract.
    pub const fn fetch(&self) -> &SourceFetch {
        &self.fetch
    }

    /// Returns the source schedule contract.
    pub const fn schedule(&self) -> &SourceSchedule {
        &self.schedule
    }

    /// Returns the source health escalation threshold.
    pub const fn escalate_after(&self) -> u32 {
        self.escalate_after
    }

    /// Returns whether direct HTTP conditional requests are enabled.
    pub const fn conditional_enabled(&self) -> bool {
        self.conditional.enabled
    }

    /// Returns the source-owned delivery policy when source routing is enabled.
    pub const fn outbox(&self) -> Option<&DeliveryPolicy> {
        self.outbox.as_ref()
    }

    /// Returns source routes in declared admission order.
    pub fn routes(&self) -> &[GraphRoute] {
        &self.routes
    }

    /// Computes the representation-affecting source digest used by every measurement MVD.
    pub fn source_representation_digest(&self) -> Result<String, CoreError> {
        self.validate()?;
        match &self.fetch {
            SourceFetch::Http {
                source_url,
                user_agent,
                accept,
                follow_redirects,
                max_redirects,
                headers,
                header_secrets,
                ..
            } => {
                #[derive(Serialize)]
                struct Header<'a> {
                    name: String,
                    value: &'a str,
                }
                #[derive(Serialize)]
                struct HeaderSecret<'a> {
                    name: String,
                    env: &'a str,
                    format: &'a str,
                    revision: u32,
                }
                #[derive(Serialize)]
                struct HttpDigest<'a> {
                    engine: &'static str,
                    address: &'a Url,
                    accept: &'a str,
                    user_agent: &'a str,
                    headers: Vec<Header<'a>>,
                    header_secrets: Vec<HeaderSecret<'a>>,
                    follow_redirects: bool,
                    max_redirects: u8,
                }
                let mut headers = headers
                    .iter()
                    .map(|(name, value)| Header {
                        name: name.to_ascii_lowercase(),
                        value,
                    })
                    .collect::<Vec<_>>();
                headers.sort_unstable_by(|left, right| left.name.cmp(&right.name));
                let mut header_secrets = header_secrets
                    .iter()
                    .map(|(name, secret)| HeaderSecret {
                        name: name.to_ascii_lowercase(),
                        env: &secret.env,
                        format: &secret.format,
                        revision: secret.revision,
                    })
                    .collect::<Vec<_>>();
                header_secrets.sort_unstable_by(|left, right| left.name.cmp(&right.name));
                crate::stable_json::stable_digest(&HttpDigest {
                    engine: "http",
                    address: source_url,
                    accept,
                    user_agent,
                    headers,
                    header_secrets,
                    follow_redirects: *follow_redirects,
                    max_redirects: *max_redirects,
                })
            }
            SourceFetch::File { file_path, .. } => {
                #[derive(Serialize)]
                struct FileDigest<'a> {
                    engine: &'static str,
                    address: &'a str,
                }
                crate::stable_json::stable_digest(&FileDigest {
                    engine: "file",
                    address: file_path,
                })
            }
        }
    }

    /// Validates every source-owned contract fact before runtime access.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_name != SOURCE_SCHEMA_NAME || self.schema_version != SOURCE_SCHEMA_VERSION {
            return Err(CoreError::contract(
                "source document is not a current FFHN source document",
            ));
        }
        require_nonblank("source.display_name", &self.display_name)?;
        if self.escalate_after == 0 {
            return Err(CoreError::contract(
                "source.escalate_after must be positive",
            ));
        }
        self.fetch.validate()?;
        if !self.fetch.is_http() && self.conditional.enabled {
            return Err(CoreError::contract(
                "conditional requests are valid only for HTTP sources",
            ));
        }
        self.schedule.validate()?;
        validate_delivery(
            self.outbox.as_ref(),
            &self.routes,
            &[GraphRouteFamily::OnSource],
        )
    }
}

/// HTTP or file acquisition contract owned by a source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "engine", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceFetch {
    /// One HTTP(S) GET representation.
    Http {
        /// Initial request URL.
        source_url: Url,
        /// Non-confidential fixed protocol parameter.
        user_agent: String,
        /// Non-confidential fixed protocol parameter.
        accept: String,
        /// Maximum accepted body bytes.
        max_bytes: usize,
        /// Whether standard redirects are followed manually.
        follow_redirects: bool,
        /// Maximum redirects when following is enabled.
        max_redirects: u8,
        /// Connect, read-idle, and total timeout bounds.
        timeouts: HttpTimeouts,
        /// Non-secret extensible request headers.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        headers: BTreeMap<String, String>,
        /// Environment-resolved extensible request headers.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        header_secrets: BTreeMap<String, FetchHeaderSecret>,
    },
    /// One absolute local file representation.
    File {
        /// Absolute file path.
        file_path: String,
        /// Maximum accepted file bytes.
        max_bytes: usize,
    },
}

impl SourceFetch {
    fn is_http(&self) -> bool {
        matches!(self, Self::Http { .. })
    }

    fn validate(&self) -> Result<(), CoreError> {
        match self {
            Self::Http {
                source_url,
                user_agent,
                accept,
                max_bytes,
                max_redirects,
                timeouts,
                headers,
                header_secrets,
                ..
            } => {
                if !matches!(source_url.scheme(), "http" | "https")
                    || source_url.username() != ""
                    || source_url.password().is_some()
                {
                    return Err(CoreError::contract(
                        "source.fetch.source_url must be an HTTP(S) URL without userinfo",
                    ));
                }
                require_nonblank("source.fetch.user_agent", user_agent)?;
                require_nonblank("source.fetch.accept", accept)?;
                validate_header_value("source.fetch.user_agent", user_agent)?;
                validate_header_value("source.fetch.accept", accept)?;
                validate_max_bytes(*max_bytes)?;
                if *max_redirects > 20 {
                    return Err(CoreError::contract(
                        "source.fetch.max_redirects must be in 0..=20",
                    ));
                }
                timeouts.validate()?;
                validate_header_tables(headers, header_secrets)
            }
            Self::File {
                file_path,
                max_bytes,
            } => {
                if !std::path::Path::new(file_path).is_absolute() {
                    return Err(CoreError::contract(
                        "source.fetch.file_path must be absolute",
                    ));
                }
                validate_max_bytes(*max_bytes)
            }
        }
    }
}

/// Independent HTTP timeout bounds in milliseconds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpTimeouts {
    /// Connection timeout in milliseconds.
    pub connect_ms: u64,
    /// Maximum idle period while reading a response body in milliseconds.
    pub read_idle_ms: u64,
    /// Whole-request timeout in milliseconds.
    pub total_ms: u64,
}

impl HttpTimeouts {
    fn validate(&self) -> Result<(), CoreError> {
        if self.connect_ms == 0 || self.read_idle_ms == 0 || self.total_ms == 0 {
            return Err(CoreError::contract("HTTP timeouts must be positive"));
        }
        Ok(())
    }
}

/// One environment-backed HTTP request header reference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FetchHeaderSecret {
    /// Environment-variable name resolved only at acquisition time.
    pub env: String,
    /// Template containing the resolved value exactly once.
    pub format: String,
    /// Non-secret operator-managed revision included in the SRD.
    pub revision: u32,
}

impl FetchHeaderSecret {
    fn validate(&self) -> Result<(), CoreError> {
        require_nonblank("source.fetch.header_secrets.env", &self.env)?;
        require_nonblank("source.fetch.header_secrets.format", &self.format)?;
        if self.format.matches("{value}").count() != 1 {
            return Err(CoreError::contract(
                "source.fetch.header_secrets.format must contain {value} exactly once",
            ));
        }
        if self.revision == 0 {
            return Err(CoreError::contract(
                "source.fetch.header_secrets.revision must be positive",
            ));
        }
        validate_header_value(
            "source.fetch.header_secrets.format",
            &self.format.replace("{value}", "value"),
        )?;
        Ok(())
    }
}

/// Conditional-request policy for an HTTP source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConditionalRequests {
    /// Whether validated source validators may be sent on eligible HTTP cycles.
    pub enabled: bool,
}

/// Fixed-interval source schedule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSchedule {
    /// Desired interval in milliseconds.
    pub interval_ms: u64,
    /// Minimum permitted interval in milliseconds.
    pub min_interval_ms: u64,
}

impl SourceSchedule {
    /// Returns the fixed source acquisition interval in milliseconds.
    pub const fn interval_ms(&self) -> u64 {
        self.interval_ms
    }

    /// Returns the configured lower cadence bound in milliseconds.
    pub const fn min_interval_ms(&self) -> u64 {
        self.min_interval_ms
    }

    fn validate(&self) -> Result<(), CoreError> {
        if self.min_interval_ms == 0 || self.interval_ms < self.min_interval_ms {
            return Err(CoreError::contract(
                "source.schedule.interval_ms must be at least a positive min_interval_ms",
            ));
        }
        Ok(())
    }
}

fn validate_header_tables(
    headers: &BTreeMap<String, String>,
    header_secrets: &BTreeMap<String, FetchHeaderSecret>,
) -> Result<(), CoreError> {
    let mut names = BTreeSet::new();
    for (name, value) in headers {
        let normalized = validate_header_name(name, false)?;
        require_nonblank("source.fetch.headers value", value)?;
        validate_header_value("source.fetch.headers value", value)?;
        if !names.insert(normalized) {
            return Err(CoreError::contract(
                "source fetch header names must be unique ignoring case",
            ));
        }
    }
    for (name, secret) in header_secrets {
        let normalized = validate_header_name(name, true)?;
        secret.validate()?;
        if !names.insert(normalized) {
            return Err(CoreError::contract(
                "source fetch header names must be unique ignoring case",
            ));
        }
    }
    Ok(())
}

fn validate_header_name(name: &str, secret: bool) -> Result<String, CoreError> {
    let normalized = ureq::http::HeaderName::from_bytes(name.as_bytes())
        .map_err(|_| CoreError::contract("source fetch header name is not a valid HTTP token"))?
        .as_str()
        .to_owned();
    let ffhn_owned = [
        "host",
        "content-length",
        "range",
        "if-none-match",
        "if-modified-since",
        "if-match",
        "if-unmodified-since",
        "if-range",
        "connection",
        "proxy-connection",
        "keep-alive",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
        "accept",
        "user-agent",
    ];
    if ffhn_owned.contains(&normalized.as_str()) {
        return Err(CoreError::contract(
            "source fetch header name is FFHN-owned",
        ));
    }
    if !secret && ["authorization", "proxy-authorization", "cookie"].contains(&normalized.as_str())
    {
        return Err(CoreError::contract(
            "credential header names require source.fetch.header_secrets",
        ));
    }
    Ok(normalized)
}

fn validate_header_value(field: &str, value: &str) -> Result<(), CoreError> {
    ureq::http::HeaderValue::from_bytes(value.as_bytes())
        .map(|_| ())
        .map_err(|_| CoreError::contract(format!("{field} is not a valid HTTP header value")))
}

fn require_nonblank(field: &str, value: &str) -> Result<(), CoreError> {
    if value.trim().is_empty() {
        Err(CoreError::contract(format!("{field} must not be blank")))
    } else {
        Ok(())
    }
}

fn validate_max_bytes(value: usize) -> Result<(), CoreError> {
    if (1_024..=104_857_600).contains(&value) {
        Ok(())
    } else {
        Err(CoreError::contract(
            "source.fetch.max_bytes must be in 1024..=104857600",
        ))
    }
}

#[cfg(test)]
#[path = "source/tests.rs"]
mod tests;

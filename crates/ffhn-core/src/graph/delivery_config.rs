//! Emitter-local delivery configuration and immutable-policy snapshot vocabulary.

use std::{collections::BTreeSet, fmt, path::Path, str::FromStr};

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::CoreError;

/// Stable identifier for one source- or measurement-local delivery route.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GraphRouteId(String);

impl GraphRouteId {
    /// Validates a local route identifier with the graph directory-ID grammar.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        if valid_id(&value) {
            Ok(Self(value))
        } else {
            Err(CoreError::contract(
                "route_id must use lowercase letters or digits separated only by one internal '-' or '_' and be at most 64 bytes",
            ))
        }
    }

    /// Returns the canonical route identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GraphRouteId {
    type Error = CoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<GraphRouteId> for String {
    fn from(value: GraphRouteId) -> Self {
        value.0
    }
}

impl FromStr for GraphRouteId {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl fmt::Display for GraphRouteId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Family of immutable events a graph route accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphRouteFamily {
    /// Condition trigger events owned by one measurement.
    OnCondition,
    /// Measurement lifecycle, extraction, and measurement-fault events.
    OnMeasurement,
    /// Source lifecycle, acquisition-health, and source-fault events.
    OnSource,
}

/// Immutable delivery policy copied into every newly admitted pending record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryPolicy {
    max_pending: usize,
    max_attempts: u32,
    base_backoff_ms: u64,
    max_backoff_ms: u64,
    jitter_ratio: String,
}

impl DeliveryPolicy {
    /// Validates the bounded retry and admission policy before it can be snapshotted.
    pub fn validate(&self) -> Result<(), CoreError> {
        if !(1..=100_000).contains(&self.max_pending)
            || !(1..=100).contains(&self.max_attempts)
            || !(1..=86_400_000).contains(&self.base_backoff_ms)
            || !(self.base_backoff_ms..=604_800_000).contains(&self.max_backoff_ms)
        {
            return Err(CoreError::contract(
                "delivery outbox policy is outside its bounded v11 range",
            ));
        }
        let jitter = Decimal::from_str(&self.jitter_ratio).map_err(|_| {
            CoreError::contract("outbox.jitter_ratio must be an invariant decimal in 0..=1")
        })?;
        if !(Decimal::ZERO..=Decimal::ONE).contains(&jitter) {
            return Err(CoreError::contract(
                "outbox.jitter_ratio must be an invariant decimal in 0..=1",
            ));
        }
        Ok(())
    }

    /// Returns the bounded pending-record capacity for one emitter.
    pub const fn max_pending(&self) -> usize {
        self.max_pending
    }

    /// Returns the terminal attempt bound carried by a newly admitted record.
    pub const fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    /// Returns the immutable base retry delay in milliseconds.
    pub const fn base_backoff_ms(&self) -> u64 {
        self.base_backoff_ms
    }

    /// Returns the immutable maximum retry delay in milliseconds.
    pub const fn max_backoff_ms(&self) -> u64 {
        self.max_backoff_ms
    }

    /// Parses the immutable deterministic jitter ratio.
    pub fn jitter_ratio(&self) -> Result<Decimal, CoreError> {
        Decimal::from_str(&self.jitter_ratio)
            .map_err(|_| CoreError::internal("validated delivery jitter ratio became undecodable"))
    }
}

/// One local route whose adapter is copied into newly admitted records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphRoute {
    route_id: GraphRouteId,
    route_family: GraphRouteFamily,
    adapter: GraphDeliveryAdapter,
}

impl GraphRoute {
    /// Returns the declared route identity.
    pub fn route_id(&self) -> &GraphRouteId {
        &self.route_id
    }

    /// Returns the event family the route accepts.
    pub const fn route_family(&self) -> GraphRouteFamily {
        self.route_family
    }

    /// Returns the complete adapter snapshot configured for this route.
    pub const fn adapter(&self) -> &GraphDeliveryAdapter {
        &self.adapter
    }

    /// Validates route identity and adapter configuration.
    pub fn validate(&self) -> Result<(), CoreError> {
        self.adapter.validate()
    }
}

/// Environment-backed HTTP delivery-header secret reference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryHeaderSecret {
    /// Environment variable resolved only at delivery attempt time.
    pub env: String,
    /// Template containing `{value}` exactly once.
    pub format: String,
}

/// Closed delivery adapter vocabulary owned by graph records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GraphDeliveryAdapter {
    /// Writes one event envelope and newline to an absolute executable's stdin.
    ProcessStdin {
        /// Absolute executable path.
        program: String,
        /// Exact argument vector.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
        /// Maximum child lifetime.
        timeout_ms: u64,
    },
    /// Posts one event envelope to a no-redirect HTTPS webhook.
    HttpWebhook {
        /// Absolute HTTPS endpoint.
        url: Url,
        /// Maximum delivery attempt lifetime.
        timeout_ms: u64,
        /// Header secrets resolved only at delivery attempt time.
        #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        header_secrets: std::collections::BTreeMap<String, DeliveryHeaderSecret>,
    },
}

impl GraphDeliveryAdapter {
    /// Validates adapter-specific configuration before it can be snapshotted into a record.
    pub fn validate(&self) -> Result<(), CoreError> {
        match self {
            Self::ProcessStdin {
                program,
                args,
                timeout_ms,
            } => {
                if program.trim().is_empty() || !Path::new(program).is_absolute() {
                    return Err(CoreError::contract(
                        "process_stdin program must be a nonblank absolute path",
                    ));
                }
                if args.iter().any(|argument| argument.trim().is_empty())
                    || !(100..=60_000).contains(timeout_ms)
                {
                    return Err(CoreError::contract(
                        "process_stdin args must be nonblank and timeout_ms in 100..=60000",
                    ));
                }
                Ok(())
            }
            Self::HttpWebhook {
                url,
                timeout_ms,
                header_secrets,
            } => {
                if url.scheme() != "https" || url.username() != "" || url.password().is_some() {
                    return Err(CoreError::contract(
                        "http_webhook url must be an HTTPS URL without userinfo",
                    ));
                }
                if !(100..=60_000).contains(timeout_ms) {
                    return Err(CoreError::contract(
                        "http_webhook timeout_ms must be in 100..=60000",
                    ));
                }
                let mut names = BTreeSet::new();
                for (name, secret) in header_secrets {
                    if webhook_header_name(name).is_none_or(|name| !names.insert(name))
                        || secret.env.trim().is_empty()
                        || secret.format.matches("{value}").count() != 1
                        || ureq::http::HeaderValue::from_bytes(
                            secret.format.replace("{value}", "value").as_bytes(),
                        )
                        .is_err()
                    {
                        return Err(CoreError::contract(
                            "http_webhook header secrets require unique permitted token names, nonblank env, and one {value}",
                        ));
                    }
                }
                Ok(())
            }
        }
    }
}

fn webhook_header_name(name: &str) -> Option<String> {
    let normalized = ureq::http::HeaderName::from_bytes(name.as_bytes())
        .ok()?
        .as_str()
        .to_owned();
    if [
        "host",
        "content-length",
        "content-type",
        "connection",
        "keep-alive",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ]
    .contains(&normalized.as_str())
    {
        None
    } else {
        Some(normalized)
    }
}

/// Validates source- or measurement-local route uniqueness and expected families.
pub(crate) fn validate_delivery(
    outbox: Option<&DeliveryPolicy>,
    routes: &[GraphRoute],
    allowed: &[GraphRouteFamily],
) -> Result<(), CoreError> {
    if outbox.is_some() != !routes.is_empty() {
        return Err(CoreError::contract(
            "outbox and routes must be configured together or both omitted",
        ));
    }
    if let Some(policy) = outbox {
        policy.validate()?;
    }
    let mut ids = BTreeSet::new();
    for route in routes {
        if !allowed.contains(&route.route_family()) || !ids.insert(route.route_id()) {
            return Err(CoreError::contract(
                "delivery route has an invalid family for its emitter or duplicates a route_id",
            ));
        }
        route.validate()?;
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
        && (value.as_bytes()[0].is_ascii_lowercase() || value.as_bytes()[0].is_ascii_digit())
        && !value.ends_with(['-', '_'])
        && !value.contains("--")
        && !value.contains("__")
        && !value.contains("-_")
        && !value.contains("_-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_identifier_grammar_does_not_allow_operator_path_or_separator_ambiguity() {
        for value in [
            "",
            "UPPER",
            "a--b",
            "a__b",
            "a-_b",
            "a_-b",
            "a/b",
            "a-",
            "a_",
            "_a",
            "-a",
            &"a".repeat(65),
        ] {
            assert!(GraphRouteId::new(value).is_err(), "{value}");
        }
        for valid in ["price-alert", "route_1", "1", &"a".repeat(64)] {
            assert!(GraphRouteId::new(valid).is_ok(), "{valid}");
        }
    }

    #[test]
    fn webhook_secret_headers_are_case_insensitively_unique_and_cannot_own_framing() {
        let duplicated: Result<GraphRoute, _> = toml::from_str(
            "route_id = \"webhook\"\nroute_family = \"on_source\"\n[adapter]\nkind = \"http_webhook\"\nurl = \"https://example.test/hook\"\ntimeout_ms = 1000\n[adapter.header_secrets]\nAuthorization = { env = \"ONE\", format = \"Bearer {value}\" }\nauthorization = { env = \"TWO\", format = \"Bearer {value}\" }\n",
        );
        assert!(duplicated.expect("structural route").validate().is_err());
        let framing: GraphRoute = toml::from_str(
            "route_id = \"webhook\"\nroute_family = \"on_source\"\n[adapter]\nkind = \"http_webhook\"\nurl = \"https://example.test/hook\"\ntimeout_ms = 1000\n[adapter.header_secrets]\nContent-Type = { env = \"TYPE\", format = \"{value}\" }\n",
        )
        .expect("structural route");
        assert!(framing.validate().is_err());
    }

    #[test]
    fn delivery_value_objects_cover_accessors_and_every_validation_family() {
        let route_id = GraphRouteId::new("route_1").expect("route id");
        assert_eq!(route_id.as_str(), "route_1");
        assert_eq!(route_id.to_string(), "route_1");
        assert_eq!("route_1".parse::<GraphRouteId>().expect("parsed"), route_id);
        assert_eq!(String::from(route_id.clone()), "route_1");
        assert!(serde_json::from_str::<GraphRouteId>("\"UPPER\"").is_err());

        let policy: DeliveryPolicy = toml::from_str(
            "max_pending = 2\nmax_attempts = 3\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0.5\"\n",
        )
        .expect("policy");
        policy.validate().expect("valid policy");
        assert_eq!(policy.max_pending(), 2);
        assert_eq!(policy.max_attempts(), 3);
        assert_eq!(policy.base_backoff_ms(), 10);
        assert_eq!(policy.max_backoff_ms(), 100);
        assert_eq!(policy.jitter_ratio().expect("jitter"), Decimal::new(5, 1));
        for invalid in [
            "max_pending = 0\nmax_attempts = 3\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n",
            "max_pending = 1\nmax_attempts = 101\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n",
            "max_pending = 1\nmax_attempts = 3\nbase_backoff_ms = 0\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n",
            "max_pending = 1\nmax_attempts = 3\nbase_backoff_ms = 100\nmax_backoff_ms = 99\njitter_ratio = \"0\"\n",
            "max_pending = 1\nmax_attempts = 3\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"invalid\"\n",
            "max_pending = 1\nmax_attempts = 3\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"1.1\"\n",
            "max_pending = 100001\nmax_attempts = 3\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n",
            "max_pending = 1\nmax_attempts = 0\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n",
            "max_pending = 1\nmax_attempts = 3\nbase_backoff_ms = 86400001\nmax_backoff_ms = 86400001\njitter_ratio = \"0\"\n",
            "max_pending = 1\nmax_attempts = 3\nbase_backoff_ms = 10\nmax_backoff_ms = 604800001\njitter_ratio = \"0\"\n",
        ] {
            let policy: DeliveryPolicy = toml::from_str(invalid).expect("structural policy");
            assert!(policy.validate().is_err());
        }

        let process = crate::graph::test_support::process_adapter_toml(true, 1_000);
        let route: GraphRoute = toml::from_str(&format!(
            "route_id = \"route\"\nroute_family = \"on_source\"\n[adapter]\n{process}"
        ))
        .expect("route");
        assert_eq!(route.route_id().as_str(), "route");
        assert_eq!(route.route_family(), GraphRouteFamily::OnSource);
        assert!(matches!(
            route.adapter(),
            GraphDeliveryAdapter::ProcessStdin { .. }
        ));
        route.validate().expect("valid route");

        for invalid in [
            "kind = \"process_stdin\"\nprogram = \"relative\"\ntimeout_ms = 1000\n",
            "kind = \"process_stdin\"\nprogram = \" \"\ntimeout_ms = 1000\n",
            "kind = \"process_stdin\"\nprogram = \"/absolute\"\nargs = [\" \" ]\ntimeout_ms = 1000\n",
            "kind = \"process_stdin\"\nprogram = \"/absolute\"\ntimeout_ms = 99\n",
            "kind = \"http_webhook\"\nurl = \"http://example.test\"\ntimeout_ms = 1000\n",
            "kind = \"http_webhook\"\nurl = \"https://user@example.test\"\ntimeout_ms = 1000\n",
            "kind = \"http_webhook\"\nurl = \"https://:pass@example.test\"\ntimeout_ms = 1000\n",
            "kind = \"http_webhook\"\nurl = \"https://example.test\"\ntimeout_ms = 99\n",
            "kind = \"http_webhook\"\nurl = \"https://example.test\"\ntimeout_ms = 1000\n[header_secrets]\nAuthorization = { env = \" \" , format = \"{value}\" }\n",
        ] {
            let adapter: GraphDeliveryAdapter =
                toml::from_str(invalid).expect("structural adapter");
            assert!(adapter.validate().is_err(), "{invalid}");
        }
        let webhook: GraphDeliveryAdapter = toml::from_str(
            "kind = \"http_webhook\"\nurl = \"https://example.test/hook\"\ntimeout_ms = 1000\n[header_secrets]\nAuthorization = { env = \"TOKEN\", format = \"Bearer {value}\" }\n",
        )
        .expect("webhook");
        webhook.validate().expect("valid webhook");
        for invalid in [
            "kind = \"http_webhook\"\nurl = \"https://example.test\"\ntimeout_ms = 1000\n[header_secrets]\n\"bad header\" = { env = \"TOKEN\", format = \"{value}\" }\n",
            "kind = \"http_webhook\"\nurl = \"https://example.test\"\ntimeout_ms = 1000\n[header_secrets]\nAuthorization = { env = \"TOKEN\", format = \"none\" }\n",
            "kind = \"http_webhook\"\nurl = \"https://example.test\"\ntimeout_ms = 1000\n[header_secrets]\nAuthorization = { env = \"TOKEN\", format = \"{value}{value}\" }\n",
            "kind = \"http_webhook\"\nurl = \"https://example.test\"\ntimeout_ms = 1000\n[header_secrets]\nAuthorization = { env = \"TOKEN\", format = \"{value}\\nInjected: yes\" }\n",
        ] {
            let adapter: GraphDeliveryAdapter = toml::from_str(invalid).expect("adapter");
            assert!(adapter.validate().is_err());
        }

        assert!(
            validate_delivery(
                None,
                std::slice::from_ref(&route),
                &[GraphRouteFamily::OnSource]
            )
            .is_err()
        );
        validate_delivery(
            Some(&policy),
            std::slice::from_ref(&route),
            &[GraphRouteFamily::OnSource],
        )
        .expect("valid delivery");
        assert!(validate_delivery(Some(&policy), &[], &[GraphRouteFamily::OnSource]).is_err());
        assert!(
            validate_delivery(
                Some(&policy),
                &[route.clone(), route.clone()],
                &[GraphRouteFamily::OnSource]
            )
            .is_err()
        );
        assert!(
            validate_delivery(Some(&policy), &[route], &[GraphRouteFamily::OnMeasurement]).is_err()
        );
    }
}

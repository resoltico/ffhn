use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use url::Url;

use super::{
    CanonicalizerSpec, CompareBasis, CompareConfig, Extensions, FetchConfig, FetchEngine,
    FileFetchConfig, NetworkFetchConfig, NotificationAdapter, NotificationEndpoint,
    NotificationRoute, RegexFlag, RunOutcome, SelectionConfig, SelectionKind, SelectionMatch,
    SelectionModeConfig, StorageConfig, TargetDocument, TargetKind, TargetSource, WhitespaceMode,
};
use crate::{CoreError, DelimiterMode, HttpMethod};

use super::super::defaults::{
    default_fetch_max_bytes, default_fetch_method, default_fetch_timeout_ms,
    default_follow_redirects, default_history_limit, default_notification_timeout_ms,
};
use crate::CanonicalizerKind;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawTargetSource {
    kind: TargetKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_url: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_path: Option<String>,
}

impl TryFrom<RawTargetSource> for TargetSource {
    type Error = CoreError;

    fn try_from(raw: RawTargetSource) -> Result<Self, Self::Error> {
        match raw.kind {
            TargetKind::Http => {
                if raw.file_path.is_some() {
                    return Err(CoreError::contract(
                        "http targets do not accept target.file_path",
                    ));
                }

                Ok(Self::Http {
                    source_url: raw
                        .source_url
                        .ok_or_else(|| CoreError::contract("target.source_url is required"))?,
                })
            }
            TargetKind::File => {
                if raw.source_url.is_some() {
                    return Err(CoreError::contract(
                        "file targets do not accept target.source_url",
                    ));
                }

                Ok(Self::File {
                    file_path: raw
                        .file_path
                        .ok_or_else(|| CoreError::contract("target.file_path is required"))?,
                })
            }
        }
    }
}

impl From<&TargetSource> for RawTargetSource {
    fn from(source: &TargetSource) -> Self {
        match source {
            TargetSource::Http { source_url } => Self {
                kind: TargetKind::Http,
                source_url: Some(source_url.clone()),
                file_path: None,
            },
            TargetSource::File { file_path } => Self {
                kind: TargetKind::File,
                source_url: None,
                file_path: Some(file_path.clone()),
            },
        }
    }
}

impl Serialize for TargetSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawTargetSource::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TargetSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawTargetSource::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawFetchConfig {
    engine: FetchEngine,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<HttpMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    follow_redirects: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    accept: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    headers: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
}

impl TryFrom<RawFetchConfig> for FetchConfig {
    type Error = CoreError;

    fn try_from(raw: RawFetchConfig) -> Result<Self, Self::Error> {
        let RawFetchConfig {
            engine,
            method,
            timeout_ms,
            max_bytes,
            user_agent,
            follow_redirects,
            accept,
            headers,
            extensions,
        } = raw;

        match engine {
            FetchEngine::Http => Ok(Self::Http(NetworkFetchConfig {
                method: method.unwrap_or_else(default_fetch_method),
                timeout_ms: timeout_ms.unwrap_or_else(default_fetch_timeout_ms),
                max_bytes: max_bytes.unwrap_or_else(default_fetch_max_bytes),
                user_agent: user_agent.unwrap_or_default(),
                follow_redirects: follow_redirects.unwrap_or_else(default_follow_redirects),
                accept: accept.unwrap_or_default(),
                headers,
                extensions,
            })),
            FetchEngine::File => {
                if method.is_some() {
                    return Err(CoreError::contract(
                        "file fetch does not accept fetch.method",
                    ));
                }
                if timeout_ms.is_some() {
                    return Err(CoreError::contract(
                        "file fetch does not accept fetch.timeout_ms",
                    ));
                }
                if user_agent.is_some() {
                    return Err(CoreError::contract(
                        "file fetch does not accept fetch.user_agent",
                    ));
                }
                if follow_redirects.is_some() {
                    return Err(CoreError::contract(
                        "file fetch does not accept fetch.follow_redirects",
                    ));
                }
                if accept.is_some() {
                    return Err(CoreError::contract(
                        "file fetch does not accept fetch.accept",
                    ));
                }
                if !headers.is_empty() {
                    return Err(CoreError::contract(
                        "file fetch does not accept fetch.headers",
                    ));
                }

                Ok(Self::File(FileFetchConfig {
                    max_bytes: max_bytes.unwrap_or_else(default_fetch_max_bytes),
                    extensions,
                }))
            }
        }
    }
}

impl From<&FetchConfig> for RawFetchConfig {
    fn from(fetch: &FetchConfig) -> Self {
        match fetch {
            FetchConfig::Http(config) => Self {
                engine: FetchEngine::Http,
                method: Some(config.method),
                timeout_ms: Some(config.timeout_ms),
                max_bytes: Some(config.max_bytes),
                user_agent: Some(config.user_agent.clone()),
                follow_redirects: Some(config.follow_redirects),
                accept: Some(config.accept.clone()),
                headers: config.headers.clone(),
                extensions: config.extensions.clone(),
            },
            FetchConfig::File(config) => Self {
                engine: FetchEngine::File,
                method: None,
                timeout_ms: None,
                max_bytes: Some(config.max_bytes),
                user_agent: None,
                follow_redirects: None,
                accept: None,
                headers: BTreeMap::new(),
                extensions: config.extensions.clone(),
            },
        }
    }
}

impl Serialize for FetchConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawFetchConfig::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FetchConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawFetchConfig::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawSelectionConfig {
    kind: SelectionKind,
    #[serde(rename = "match")]
    selection_match: SelectionMatch,
    #[serde(skip_serializing_if = "Option::is_none")]
    index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<DelimiterMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_start: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_end: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    flags: Vec<RegexFlag>,
}

impl TryFrom<RawSelectionConfig> for SelectionConfig {
    type Error = CoreError;

    fn try_from(raw: RawSelectionConfig) -> Result<Self, Self::Error> {
        let selection_mode = SelectionModeConfig::from_raw(raw.selection_match, raw.index)?;

        match raw.kind {
            SelectionKind::CssSelector => {
                if raw.start.is_some()
                    || raw.end.is_some()
                    || raw.mode.is_some()
                    || raw.include_start.is_some()
                    || raw.include_end.is_some()
                    || !raw.flags.is_empty()
                {
                    return Err(CoreError::contract(
                        "css_selector targets do not accept delimiter-pair selection fields",
                    ));
                }

                Ok(Self::CssSelector {
                    selection_mode,
                    selector: raw
                        .selector
                        .ok_or_else(|| CoreError::contract("selection.selector is required"))?,
                })
            }
            SelectionKind::DelimiterPair => {
                if raw.selector.is_some() {
                    return Err(CoreError::contract(
                        "delimiter_pair targets do not accept selection.selector",
                    ));
                }

                Ok(Self::DelimiterPair {
                    selection_mode,
                    start: raw
                        .start
                        .ok_or_else(|| CoreError::contract("selection.start is required"))?,
                    end: raw
                        .end
                        .ok_or_else(|| CoreError::contract("selection.end is required"))?,
                    mode: raw
                        .mode
                        .ok_or_else(|| CoreError::contract("selection.mode is required"))?,
                    include_start: raw.include_start.ok_or_else(|| {
                        CoreError::contract("selection.include_start is required")
                    })?,
                    include_end: raw
                        .include_end
                        .ok_or_else(|| CoreError::contract("selection.include_end is required"))?,
                    flags: raw.flags,
                })
            }
        }
    }
}

impl From<&SelectionConfig> for RawSelectionConfig {
    fn from(selection: &SelectionConfig) -> Self {
        match selection {
            SelectionConfig::CssSelector {
                selection_mode,
                selector,
            } => {
                let (selection_match, index) = selection_mode.raw_parts();
                Self {
                    kind: SelectionKind::CssSelector,
                    selection_match,
                    index,
                    selector: Some(selector.clone()),
                    start: None,
                    end: None,
                    mode: None,
                    include_start: None,
                    include_end: None,
                    flags: Vec::new(),
                }
            }
            SelectionConfig::DelimiterPair {
                selection_mode,
                start,
                end,
                mode,
                include_start,
                include_end,
                flags,
            } => {
                let (selection_match, index) = selection_mode.raw_parts();
                Self {
                    kind: SelectionKind::DelimiterPair,
                    selection_match,
                    index,
                    selector: None,
                    start: Some(start.clone()),
                    end: Some(end.clone()),
                    mode: Some(*mode),
                    include_start: Some(*include_start),
                    include_end: Some(*include_end),
                    flags: flags.clone(),
                }
            }
        }
    }
}

impl Serialize for SelectionConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawSelectionConfig::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SelectionConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawSelectionConfig::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawCanonicalizerSpec {
    kind: CanonicalizerKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    flags: Vec<RegexFlag>,
}

impl From<RawCanonicalizerSpec> for CanonicalizerSpec {
    fn from(raw: RawCanonicalizerSpec) -> Self {
        Self {
            kind: raw.kind,
            pattern: raw.pattern,
            flags: raw.flags,
        }
    }
}

impl From<&CanonicalizerSpec> for RawCanonicalizerSpec {
    fn from(spec: &CanonicalizerSpec) -> Self {
        Self {
            kind: spec.kind,
            pattern: spec.pattern.clone(),
            flags: spec.flags.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawCompareConfig {
    basis: CompareBasis,
    #[serde(skip_serializing_if = "Option::is_none")]
    whitespace: Option<WhitespaceMode>,
    #[serde(default)]
    rewrite_urls: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    canonicalization: Vec<RawCanonicalizerSpec>,
}

impl From<RawCompareConfig> for CompareConfig {
    fn from(raw: RawCompareConfig) -> Self {
        Self {
            basis: raw.basis,
            whitespace: raw.whitespace,
            rewrite_urls: raw.rewrite_urls,
            canonicalization: raw
                .canonicalization
                .into_iter()
                .map(CanonicalizerSpec::from)
                .collect(),
        }
    }
}

impl From<&CompareConfig> for RawCompareConfig {
    fn from(compare: &CompareConfig) -> Self {
        Self {
            basis: compare.basis,
            whitespace: compare.whitespace,
            rewrite_urls: compare.rewrite_urls,
            canonicalization: compare
                .canonicalization
                .iter()
                .map(RawCanonicalizerSpec::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawStorageConfig {
    #[serde(default = "default_history_limit")]
    history_limit: usize,
}

impl Default for RawStorageConfig {
    fn default() -> Self {
        Self {
            history_limit: default_history_limit(),
        }
    }
}

impl From<RawStorageConfig> for StorageConfig {
    fn from(raw: RawStorageConfig) -> Self {
        Self {
            history_limit: raw.history_limit,
        }
    }
}

impl From<&StorageConfig> for RawStorageConfig {
    fn from(storage: &StorageConfig) -> Self {
        Self {
            history_limit: storage.history_limit,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawNotificationRoute {
    name: String,
    on: Vec<RunOutcome>,
    endpoint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RawNotificationAdapterKind {
    ProcessStdin,
}

impl From<&NotificationAdapter> for RawNotificationAdapterKind {
    fn from(adapter: &NotificationAdapter) -> Self {
        match adapter {
            NotificationAdapter::ProcessStdin { .. } => Self::ProcessStdin,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawNotificationEndpoint {
    name: String,
    kind: RawNotificationAdapterKind,
    program: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(default = "default_notification_timeout_ms")]
    timeout_ms: u64,
}

impl From<RawNotificationRoute> for NotificationRoute {
    fn from(raw: RawNotificationRoute) -> Self {
        Self {
            name: raw.name,
            on: raw.on,
            endpoint: raw.endpoint,
        }
    }
}

impl From<&NotificationRoute> for RawNotificationRoute {
    fn from(route: &NotificationRoute) -> Self {
        Self {
            name: route.name.clone(),
            on: route.on.clone(),
            endpoint: route.endpoint.clone(),
        }
    }
}

impl From<RawNotificationEndpoint> for NotificationEndpoint {
    fn from(raw: RawNotificationEndpoint) -> Self {
        Self {
            name: raw.name,
            adapter: match raw.kind {
                RawNotificationAdapterKind::ProcessStdin => NotificationAdapter::ProcessStdin {
                    program: raw.program,
                    args: raw.args,
                    timeout_ms: raw.timeout_ms,
                },
            },
        }
    }
}

impl From<&NotificationEndpoint> for RawNotificationEndpoint {
    fn from(endpoint: &NotificationEndpoint) -> Self {
        match &endpoint.adapter {
            NotificationAdapter::ProcessStdin {
                program,
                args,
                timeout_ms,
            } => Self {
                name: endpoint.name.clone(),
                kind: RawNotificationAdapterKind::from(&endpoint.adapter),
                program: program.clone(),
                args: args.clone(),
                timeout_ms: *timeout_ms,
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTargetDocument {
    schema_name: String,
    schema_version: u32,
    target_id: String,
    display_name: String,
    enabled: bool,
    target: TargetSource,
    fetch: FetchConfig,
    selection: SelectionConfig,
    compare: RawCompareConfig,
    #[serde(default)]
    storage: RawStorageConfig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notification_endpoints: Vec<RawNotificationEndpoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notification_routes: Vec<RawNotificationRoute>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
}

impl TryFrom<RawTargetDocument> for TargetDocument {
    type Error = CoreError;

    fn try_from(raw: RawTargetDocument) -> Result<Self, Self::Error> {
        let document = Self {
            schema_name: raw.schema_name,
            schema_version: raw.schema_version,
            target_id: raw.target_id.try_into()?,
            display_name: raw.display_name,
            enabled: raw.enabled,
            target: raw.target,
            fetch: raw.fetch,
            selection: raw.selection,
            compare: raw.compare.into(),
            storage: raw.storage.into(),
            notification_endpoints: raw
                .notification_endpoints
                .into_iter()
                .map(NotificationEndpoint::from)
                .collect(),
            notification_routes: raw
                .notification_routes
                .into_iter()
                .map(NotificationRoute::from)
                .collect(),
            extensions: raw.extensions,
        };
        document.validate()?;
        Ok(document)
    }
}

impl From<&TargetDocument> for RawTargetDocument {
    fn from(document: &TargetDocument) -> Self {
        Self {
            schema_name: document.schema_name.clone(),
            schema_version: document.schema_version,
            target_id: document.target_id.as_str().to_owned(),
            display_name: document.display_name.clone(),
            enabled: document.enabled,
            target: document.target.clone(),
            fetch: document.fetch.clone(),
            selection: document.selection.clone(),
            compare: RawCompareConfig::from(&document.compare),
            storage: RawStorageConfig::from(&document.storage),
            notification_endpoints: document
                .notification_endpoints
                .iter()
                .map(RawNotificationEndpoint::from)
                .collect(),
            notification_routes: document
                .notification_routes
                .iter()
                .map(RawNotificationRoute::from)
                .collect(),
            extensions: document.extensions.clone(),
        }
    }
}

impl Serialize for TargetDocument {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawTargetDocument::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TargetDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawTargetDocument::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

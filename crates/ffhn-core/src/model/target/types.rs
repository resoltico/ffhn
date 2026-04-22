use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use url::Url;

use super::super::{
    CanonicalizerKind, CompareBasis, DelimiterMode, Extensions, FetchEngine, HttpMethod,
    NotificationEvent, OutputKind, RegexFlag, SelectionKind, SelectionMatch, TargetKind,
    WhitespaceMode,
};
use super::defaults::{
    default_fetch_max_bytes, default_fetch_method, default_fetch_timeout_ms,
    default_follow_redirects, default_history_limit, default_notification_shell,
    default_notification_timeout_ms,
};

/// Target source section.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetSource {
    /// Source kind.
    pub kind: TargetKind,
    /// Absolute source URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<Url>,
    /// Absolute local file path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
}

/// Fetch section.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FetchConfig {
    /// Fetch engine.
    pub engine: FetchEngine,
    /// HTTP method.
    #[serde(default = "default_fetch_method")]
    pub method: HttpMethod,
    /// Timeout in milliseconds.
    #[serde(default = "default_fetch_timeout_ms")]
    pub timeout_ms: u64,
    /// Maximum body size in bytes.
    #[serde(default = "default_fetch_max_bytes")]
    pub max_bytes: usize,
    /// User-Agent header.
    #[serde(default)]
    pub user_agent: String,
    /// Redirect policy.
    #[serde(default = "default_follow_redirects")]
    pub follow_redirects: bool,
    /// Accept header.
    #[serde(default)]
    pub accept: String,
    /// Optional extra headers.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// Reserved extensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Extensions,
}

/// Rolling snapshot retention policy.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    /// Total successful snapshots retained, including `snapshots/current`.
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            history_limit: default_history_limit(),
        }
    }
}

/// Best-effort shell notification hook.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NotificationHook {
    /// Stable hook label used in reports.
    pub name: String,
    /// Event filter for the hook.
    pub on: Vec<NotificationEvent>,
    /// Shell path used to execute the command.
    #[serde(default = "default_notification_shell")]
    pub shell: String,
    /// Shell command executed with the run report on stdin.
    pub command: String,
    /// Maximum runtime in milliseconds.
    #[serde(default = "default_notification_timeout_ms")]
    pub timeout_ms: u64,
}

/// Selection section.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SelectionConfig {
    /// Strategy discriminator.
    pub kind: SelectionKind,
    /// Selection mode.
    #[serde(rename = "match")]
    pub r#match: SelectionMatch,
    /// One-based index for `match = "nth"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    /// Output payload.
    pub output: OutputKind,
    /// Extraction-time whitespace mode.
    pub whitespace: WhitespaceMode,
    /// Extraction-time URL rewriting.
    pub rewrite_urls: bool,
    /// CSS selector when `kind = "css_selector"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// Start delimiter when `kind = "delimiter_pair"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// End delimiter when `kind = "delimiter_pair"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    /// Delimiter mode when `kind = "delimiter_pair"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<DelimiterMode>,
    /// Include start boundary when `kind = "delimiter_pair"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_start: Option<bool>,
    /// Include end boundary when `kind = "delimiter_pair"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_end: Option<bool>,
    /// Regex flags when delimiter mode is regex.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<RegexFlag>,
}

/// Compare section.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CompareConfig {
    /// Compare basis.
    pub basis: CompareBasis,
    /// Ordered canonicalization pipeline.
    pub canonicalization: Vec<CanonicalizerSpec>,
}

/// One canonicalizer specification.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CanonicalizerSpec {
    /// Canonicalizer kind.
    pub kind: CanonicalizerKind,
    /// Regex pattern for `strip_regex`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Regex flags for `strip_regex`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<RegexFlag>,
}

/// Top-level FFHN target document.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetDocument {
    /// Frozen schema identity.
    pub schema_name: String,
    /// Frozen schema version.
    pub schema_version: u32,
    /// Target id.
    pub target_id: String,
    /// Display name.
    pub display_name: String,
    /// Enable flag.
    pub enabled: bool,
    /// Target source section.
    pub target: TargetSource,
    /// Fetch section.
    pub fetch: FetchConfig,
    /// Selection section.
    pub selection: SelectionConfig,
    /// Compare section.
    pub compare: CompareConfig,
    /// Rolling storage policy.
    #[serde(default)]
    pub storage: StorageConfig,
    /// Notification hooks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notifications: Vec<NotificationHook>,
    /// Reserved extensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Extensions,
}

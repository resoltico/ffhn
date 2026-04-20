use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::CoreError;

use super::schema::{TARGET_SCHEMA_NAME, TARGET_SCHEMA_VERSION};
use super::validate::{
    apply_regex_flag, forbid_option, require_non_empty, require_non_empty_option,
    validate_absolute_file_path, validate_absolute_url, validate_identity, validate_target_id,
};
use super::{
    CanonicalizerKind, CompareBasis, DelimiterMode, Extensions, FetchEngine, HttpMethod,
    NotificationEvent, OutputKind, RegexFlag, SelectionKind, SelectionMatch, TargetKind,
    WhitespaceMode,
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

impl TargetDocument {
    /// Validates one target document against the frozen FFHN schema contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_identity(
            &self.schema_name,
            TARGET_SCHEMA_NAME,
            self.schema_version,
            TARGET_SCHEMA_VERSION,
        )?;
        validate_target_id(&self.target_id)?;
        require_non_empty("display_name", &self.display_name)?;
        self.target.validate()?;
        self.fetch.validate_for_source(&self.target)?;
        self.storage.validate()?;
        validate_unique_hook_names(&self.notifications)?;
        for hook in &self.notifications {
            hook.validate()?;
        }

        self.selection.validate()?;
        self.compare.validate()
    }
}

impl TargetSource {
    /// Validates the source discriminator-specific fields.
    pub fn validate(&self) -> Result<(), CoreError> {
        match self.kind {
            TargetKind::Http => {
                validate_absolute_url(self.source_url.as_ref().ok_or_else(|| {
                    CoreError::htmlcut("target.source_url is required for http targets")
                })?)?;
                if self.file_path.is_some() {
                    return Err(CoreError::htmlcut(
                        "target.file_path is only valid for file targets",
                    ));
                }
            }
            TargetKind::File => {
                let file_path = self.file_path.as_deref().ok_or_else(|| {
                    CoreError::htmlcut("target.file_path is required for file targets")
                })?;
                validate_absolute_file_path(file_path)?;
                if self.source_url.is_some() {
                    return Err(CoreError::htmlcut(
                        "target.source_url is only valid for http targets",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl FetchConfig {
    fn validate_for_source(&self, target: &TargetSource) -> Result<(), CoreError> {
        if self.max_bytes < 1_024 || self.max_bytes > 104_857_600 {
            return Err(CoreError::htmlcut(
                "fetch.max_bytes must be in 1024..104857600",
            ));
        }

        match target.kind {
            TargetKind::Http => {
                if self.engine == FetchEngine::File {
                    return Err(CoreError::htmlcut(
                        "fetch.engine = file is only valid for file targets",
                    ));
                }
                if self.timeout_ms < 1_000 || self.timeout_ms > 600_000 {
                    return Err(CoreError::htmlcut(
                        "fetch.timeout_ms must be in 1000..600000",
                    ));
                }
                require_non_empty("fetch.user_agent", &self.user_agent)?;
                require_non_empty("fetch.accept", &self.accept)?;
                for (name, value) in &self.headers {
                    require_non_empty("fetch.headers key", name)?;
                    require_non_empty("fetch.headers value", value)?;
                }
            }
            TargetKind::File => {
                if self.engine != FetchEngine::File {
                    return Err(contract_error("file targets require fetch.engine = file"));
                }
                if self.timeout_ms != default_fetch_timeout_ms() {
                    return Err(contract_error(
                        "file targets require the fixed fetch.timeout_ms default of 15000",
                    ));
                }
                if self.follow_redirects {
                    return Err(contract_error(
                        "file targets must disable fetch.follow_redirects",
                    ));
                }
                if !self.user_agent.is_empty() {
                    return Err(contract_error(
                        "file targets must not define fetch.user_agent",
                    ));
                }
                if !self.accept.is_empty() {
                    return Err(contract_error("file targets must not define fetch.accept"));
                }
                if !self.headers.is_empty() {
                    return Err(contract_error("file targets must not define fetch.headers"));
                }
            }
        }

        Ok(())
    }
}

impl StorageConfig {
    /// Validates one rolling storage policy.
    pub fn validate(&self) -> Result<(), CoreError> {
        if !(1..=256).contains(&self.history_limit) {
            return Err(CoreError::htmlcut(
                "storage.history_limit must be in 1..=256",
            ));
        }
        Ok(())
    }
}

impl NotificationHook {
    /// Validates one notification hook.
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty("notifications.name", &self.name)?;
        require_non_empty("notifications.shell", &self.shell)?;
        require_non_empty("notifications.command", &self.command)?;
        if !Path::new(&self.shell).is_absolute() {
            return Err(CoreError::htmlcut(
                "notifications.shell must be an absolute path",
            ));
        }
        if self.on.is_empty() {
            return Err(contract_error(
                "notifications.on must list at least one event",
            ));
        }
        if self.timeout_ms < 100 || self.timeout_ms > 60_000 {
            return Err(contract_error(
                "notifications.timeout_ms must be in 100..60000",
            ));
        }
        Ok(())
    }
}

impl SelectionConfig {
    /// Validates one target selection section.
    pub fn validate(&self) -> Result<(), CoreError> {
        match self.r#match {
            SelectionMatch::Nth => {
                let index = self.index.ok_or_else(|| {
                    CoreError::htmlcut(
                        "selection.index must be present and positive when match = nth",
                    )
                })?;
                if index == 0 {
                    return Err(CoreError::htmlcut(
                        "selection.index must be present and positive when match = nth",
                    ));
                }
            }
            SelectionMatch::Single | SelectionMatch::First => {
                if self.index.is_some() {
                    return Err(CoreError::htmlcut(
                        "selection.index is only valid when match = nth",
                    ));
                }
            }
        }

        match self.kind {
            SelectionKind::CssSelector => {
                require_non_empty_option("selection.selector", self.selector.as_deref())?;
                forbid_option("selection.start", self.start.as_deref())?;
                forbid_option("selection.end", self.end.as_deref())?;
                forbid_option("selection.mode", self.mode.as_ref())?;
                forbid_option("selection.include_start", self.include_start.as_ref())?;
                forbid_option("selection.include_end", self.include_end.as_ref())?;
                if !self.flags.is_empty() {
                    return Err(CoreError::htmlcut(
                        "selection.flags are only valid for delimiter_pair",
                    ));
                }
            }
            SelectionKind::DelimiterPair => {
                require_non_empty_option("selection.start", self.start.as_deref())?;
                require_non_empty_option("selection.end", self.end.as_deref())?;
                if self.selector.is_some() {
                    return Err(CoreError::htmlcut(
                        "selection.selector is only valid for css_selector",
                    ));
                }
                let mode = self
                    .mode
                    .ok_or_else(|| CoreError::htmlcut("selection.mode is required"))?;
                if self.include_start.is_none() {
                    return Err(CoreError::htmlcut(
                        "selection.include_start and selection.include_end are required",
                    ));
                }
                if self.include_end.is_none() {
                    return Err(CoreError::htmlcut(
                        "selection.include_start and selection.include_end are required",
                    ));
                }
                if mode == DelimiterMode::Literal && !self.flags.is_empty() {
                    return Err(CoreError::htmlcut(
                        "selection.flags are forbidden for literal delimiter mode",
                    ));
                }
            }
        }

        Ok(())
    }
}

impl CompareConfig {
    /// Validates one compare section.
    pub fn validate(&self) -> Result<(), CoreError> {
        for canonicalizer in &self.canonicalization {
            canonicalizer.validate()?;
        }
        Ok(())
    }
}

impl CanonicalizerSpec {
    /// Validates one canonicalizer entry.
    pub fn validate(&self) -> Result<(), CoreError> {
        match self.kind {
            CanonicalizerKind::Trim
            | CanonicalizerKind::CollapseWhitespace
            | CanonicalizerKind::NormalizeNewlines
            | CanonicalizerKind::Lowercase => {
                if self.pattern.is_some() {
                    return Err(CoreError::htmlcut(
                        "canonicalizer pattern/flags are only valid for strip_regex",
                    ));
                }
                if !self.flags.is_empty() {
                    return Err(CoreError::htmlcut(
                        "canonicalizer pattern/flags are only valid for strip_regex",
                    ));
                }
            }
            CanonicalizerKind::StripRegex => {
                let pattern = self.pattern.as_deref().ok_or_else(|| {
                    CoreError::htmlcut("strip_regex canonicalizer requires pattern")
                })?;
                require_non_empty("compare.canonicalization.pattern", pattern)?;
                let mut builder = regex::RegexBuilder::new(pattern);
                builder.unicode(true);
                for flag in &self.flags {
                    apply_regex_flag(flag, &mut builder);
                }
                builder.build().map_err(|error| {
                    CoreError::htmlcut(format!("invalid strip_regex pattern: {error}"))
                })?;
            }
        }
        Ok(())
    }
}

fn validate_unique_hook_names(hooks: &[NotificationHook]) -> Result<(), CoreError> {
    let mut names = BTreeSet::new();
    for hook in hooks {
        if !names.insert(hook.name.as_str()) {
            return Err(CoreError::htmlcut(
                "notifications.name values must be unique",
            ));
        }
    }
    Ok(())
}

fn contract_error(message: &'static str) -> CoreError {
    CoreError::htmlcut(message)
}

const fn default_fetch_method() -> HttpMethod {
    HttpMethod::GET
}

const fn default_fetch_timeout_ms() -> u64 {
    15_000
}

const fn default_fetch_max_bytes() -> usize {
    2_000_000
}

const fn default_follow_redirects() -> bool {
    true
}

const fn default_history_limit() -> usize {
    10
}

fn default_notification_shell() -> String {
    "/bin/sh".to_owned()
}

const fn default_notification_timeout_ms() -> u64 {
    5_000
}

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use url::Url;

use super::super::{
    CanonicalizerKind, CompareBasis, DelimiterMode, Extensions, FetchEngine, HttpMethod,
    OutputKind, RegexFlag, RunOutcome, SelectionKind, SelectionMatch, TargetId, TargetKind,
    WhitespaceMode,
};
use super::defaults::default_history_limit;

mod access;
mod wire;

/// Target source section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TargetSource {
    /// HTTP or HTTPS source.
    Http {
        /// Absolute source URL.
        source_url: Url,
    },
    /// Absolute local file source.
    File {
        /// Absolute local file path.
        file_path: String,
    },
}

/// Fetch section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FetchConfig {
    /// Raw HTTP fetch.
    Http(NetworkFetchConfig),
    /// Local file read.
    File(FileFetchConfig),
}

/// Shared network-fetch settings used by HTTP source acquisition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NetworkFetchConfig {
    /// HTTP method.
    pub(crate) method: HttpMethod,
    /// Timeout in milliseconds.
    pub(crate) timeout_ms: u64,
    /// Maximum body size in bytes.
    pub(crate) max_bytes: usize,
    /// User-Agent header.
    pub(crate) user_agent: String,
    /// Redirect policy.
    pub(crate) follow_redirects: bool,
    /// Accept header.
    pub(crate) accept: String,
    /// Optional extra headers.
    pub(crate) headers: BTreeMap<String, String>,
    /// Reserved extensions.
    pub(crate) extensions: Extensions,
}

/// File-fetch settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileFetchConfig {
    /// Maximum body size in bytes.
    pub(crate) max_bytes: usize,
    /// Reserved extensions.
    pub(crate) extensions: Extensions,
}

/// Rolling snapshot retention policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StorageConfig {
    /// Total successful snapshots retained, including `snapshots/current`.
    pub(crate) history_limit: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            history_limit: default_history_limit(),
        }
    }
}

/// Best-effort process notification hook.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NotificationHook {
    /// Stable hook label used in reports.
    pub(crate) name: String,
    /// Run outcomes that should trigger the hook.
    pub(crate) on: Vec<RunOutcome>,
    /// Executable path used to deliver the notification.
    pub(crate) program: String,
    /// Exact argument vector passed to the executable.
    pub(crate) args: Vec<String>,
    /// Maximum runtime in milliseconds.
    pub(crate) timeout_ms: u64,
}

/// Candidate selection mode inside one extraction strategy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SelectionModeConfig {
    /// Exactly one candidate must match.
    Single,
    /// Choose the first candidate.
    First,
    /// Choose one explicit one-based candidate index.
    Nth {
        /// One-based candidate index.
        index: NonZeroUsize,
    },
}

/// Selection section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SelectionConfig {
    /// CSS selector extraction.
    CssSelector {
        /// Candidate selection mode.
        selection_mode: SelectionModeConfig,
        /// Output payload.
        output: OutputKind,
        /// Extraction-time whitespace mode.
        whitespace: WhitespaceMode,
        /// Extraction-time URL rewriting.
        rewrite_urls: bool,
        /// CSS selector query.
        selector: String,
    },
    /// Delimiter-pair extraction.
    DelimiterPair {
        /// Candidate selection mode.
        selection_mode: SelectionModeConfig,
        /// Output payload.
        output: OutputKind,
        /// Extraction-time whitespace mode.
        whitespace: WhitespaceMode,
        /// Extraction-time URL rewriting.
        rewrite_urls: bool,
        /// Start delimiter.
        start: String,
        /// End delimiter.
        end: String,
        /// Delimiter matching mode.
        mode: DelimiterMode,
        /// Whether the start delimiter is included in the match.
        include_start: bool,
        /// Whether the end delimiter is included in the match.
        include_end: bool,
        /// Regex flags when delimiter mode is `regex`.
        flags: Vec<RegexFlag>,
    },
}

/// Compare section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompareConfig {
    /// Compare basis.
    pub(crate) basis: CompareBasis,
    /// Ordered canonicalization pipeline.
    pub(crate) canonicalization: Vec<CanonicalizerSpec>,
}

/// One canonicalizer specification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalizerSpec {
    /// Canonicalizer kind.
    pub(crate) kind: CanonicalizerKind,
    /// Regex pattern for `strip_regex`.
    pub(crate) pattern: Option<String>,
    /// Regex flags for `strip_regex`.
    pub(crate) flags: Vec<RegexFlag>,
}

/// Top-level FFHN target document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetDocument {
    /// Frozen schema identity.
    pub(crate) schema_name: String,
    /// Frozen schema version.
    pub(crate) schema_version: u32,
    /// Target id.
    pub(crate) target_id: TargetId,
    /// Display name.
    pub(crate) display_name: String,
    /// Enable flag.
    pub(crate) enabled: bool,
    /// Target source section.
    pub(crate) target: TargetSource,
    /// Fetch section.
    pub(crate) fetch: FetchConfig,
    /// Selection section.
    pub(crate) selection: SelectionConfig,
    /// Compare section.
    pub(crate) compare: CompareConfig,
    /// Rolling storage policy.
    pub(crate) storage: StorageConfig,
    /// Notification hooks.
    pub(crate) notifications: Vec<NotificationHook>,
    /// Reserved extensions.
    pub(crate) extensions: Extensions,
}

/// Public read-only projection of one notification hook.
#[derive(Clone, Copy, Debug)]
pub struct NotificationHookView<'a>(pub(crate) &'a NotificationHook);

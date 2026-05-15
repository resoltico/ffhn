use serde::{Deserialize, Serialize};

/// Supported target source family in FFHN v1.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TargetKind {
    /// HTTP or HTTPS source.
    Http,
    /// Local file source.
    File,
}

/// Supported fetch engines in FFHN v1.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FetchEngine {
    /// Raw HTTP fetch.
    Http,
    /// Local file read.
    File,
}

/// Supported HTTP method vocabulary for FFHN v1.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum HttpMethod {
    /// GET.
    GET,
}

/// Supported selection strategy kinds.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SelectionKind {
    /// CSS selector extraction.
    CssSelector,
    /// Delimiter-pair extraction.
    DelimiterPair,
}

/// Supported candidate selection modes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SelectionMatch {
    /// Exactly one candidate.
    Single,
    /// First candidate.
    First,
    /// Explicit one-based candidate index.
    Nth,
}

/// Supported output payload kinds.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OutputKind {
    /// Plain text.
    Text,
    /// Inner HTML.
    InnerHtml,
    /// Outer HTML.
    OuterHtml,
}

/// Supported whitespace modes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WhitespaceMode {
    /// Preserve whitespace.
    Preserve,
    /// Normalize whitespace.
    Normalize,
}

/// Supported delimiter matching modes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DelimiterMode {
    /// Literal substring boundaries.
    Literal,
    /// Regex boundaries.
    Regex,
}

/// Shared regex flag vocabulary used by FFHN target and canonicalization config.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RegexFlag {
    /// Case-insensitive matching.
    CaseInsensitive,
    /// Multi-line anchors.
    MultiLine,
    /// Dot matches new lines.
    DotMatchesNewLine,
    /// Swap greed mode.
    SwapGreed,
    /// Ignore pattern whitespace.
    IgnoreWhitespace,
}

/// Supported compare basis vocabulary.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CompareBasis {
    /// SHA-256 of compare-time canonical text.
    CanonicalTextSha256,
}

/// Supported canonicalizer kinds.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CanonicalizerKind {
    /// Trim leading and trailing whitespace.
    Trim,
    /// Collapse runs of whitespace to one space.
    CollapseWhitespace,
    /// Normalize line endings.
    NormalizeNewlines,
    /// Strip regex matches.
    StripRegex,
    /// Lowercase the text.
    Lowercase,
}

impl CanonicalizerKind {
    /// Returns the stable schema vocabulary token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trim => "trim",
            Self::CollapseWhitespace => "collapse_whitespace",
            Self::NormalizeNewlines => "normalize_newlines",
            Self::StripRegex => "strip_regex",
            Self::Lowercase => "lowercase",
        }
    }
}

impl FetchEngine {
    /// Returns the stable schema vocabulary token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::File => "file",
        }
    }
}

impl SelectionKind {
    /// Returns the stable schema vocabulary token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CssSelector => "css_selector",
            Self::DelimiterPair => "delimiter_pair",
        }
    }
}

impl SelectionMatch {
    /// Returns the stable schema vocabulary token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::First => "first",
            Self::Nth => "nth",
        }
    }
}

impl OutputKind {
    /// Returns the stable schema vocabulary token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::InnerHtml => "inner_html",
            Self::OuterHtml => "outer_html",
        }
    }
}

impl CompareBasis {
    /// Returns the stable schema vocabulary token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CanonicalTextSha256 => "canonical_text_sha256",
        }
    }
}

impl RunMode {
    /// Returns the stable schema vocabulary token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::DryRun => "dry_run",
        }
    }
}

impl FailureClass {
    /// Returns the stable schema vocabulary token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Transient => "transient",
            Self::Permanent => "permanent",
        }
    }
}

impl RunOutcome {
    /// Returns the stable schema vocabulary token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initialized => "initialized",
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
            Self::FailedTransient => "failed_transient",
            Self::FailedPermanent => "failed_permanent",
            Self::SkippedDisabled => "skipped_disabled",
        }
    }
}

impl RunFailureCause {
    /// Returns the stable schema vocabulary token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigInvalid => "config_invalid",
            Self::TargetUnavailable => "target_unavailable",
            Self::StateInvalid => "state_invalid",
            Self::LockUnavailable => "lock_unavailable",
            Self::FetchHttpClientError => "fetch_http_client_error",
            Self::FetchHttpServerError => "fetch_http_server_error",
            Self::FetchSourceError => "fetch_source_error",
            Self::FetchNetworkError => "fetch_network_error",
            Self::FetchTimeout => "fetch_timeout",
            Self::FetchTooLarge => "fetch_too_large",
            Self::FetchUnsupportedContentType => "fetch_unsupported_content_type",
            Self::FetchDecodeError => "fetch_decode_error",
            Self::SelectionContractInvalid => "selection_contract_invalid",
            Self::SelectionNoMatch => "selection_no_match",
            Self::SelectionAmbiguousMatch => "selection_ambiguous_match",
            Self::SelectionInternalError => "selection_internal_error",
            Self::CanonicalizationError => "canonicalization_error",
            Self::CompareError => "compare_error",
            Self::PersistError => "persist_error",
            Self::IntegrityMismatch => "integrity_mismatch",
        }
    }

    pub(crate) const fn failure_class(self) -> FailureClass {
        match self {
            Self::LockUnavailable
            | Self::FetchHttpServerError
            | Self::FetchNetworkError
            | Self::FetchTimeout
            | Self::PersistError => FailureClass::Transient,
            Self::ConfigInvalid
            | Self::TargetUnavailable
            | Self::StateInvalid
            | Self::FetchHttpClientError
            | Self::FetchSourceError
            | Self::FetchTooLarge
            | Self::FetchUnsupportedContentType
            | Self::FetchDecodeError
            | Self::SelectionContractInvalid
            | Self::SelectionNoMatch
            | Self::SelectionAmbiguousMatch
            | Self::SelectionInternalError
            | Self::CanonicalizationError
            | Self::CompareError
            | Self::IntegrityMismatch => FailureClass::Permanent,
        }
    }

    pub(crate) const fn run_outcome(self) -> RunOutcome {
        match self.failure_class() {
            FailureClass::Transient => RunOutcome::FailedTransient,
            FailureClass::Permanent => RunOutcome::FailedPermanent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizer_kind_as_str_matches_schema_vocabulary() {
        assert_eq!(CanonicalizerKind::Trim.as_str(), "trim");
        assert_eq!(
            CanonicalizerKind::CollapseWhitespace.as_str(),
            "collapse_whitespace"
        );
        assert_eq!(
            CanonicalizerKind::NormalizeNewlines.as_str(),
            "normalize_newlines"
        );
        assert_eq!(CanonicalizerKind::StripRegex.as_str(), "strip_regex");
        assert_eq!(CanonicalizerKind::Lowercase.as_str(), "lowercase");
        assert_eq!(FetchEngine::Http.as_str(), "http");
        assert_eq!(FetchEngine::File.as_str(), "file");
        assert_eq!(SelectionKind::CssSelector.as_str(), "css_selector");
        assert_eq!(SelectionKind::DelimiterPair.as_str(), "delimiter_pair");
        assert_eq!(SelectionMatch::Single.as_str(), "single");
        assert_eq!(SelectionMatch::First.as_str(), "first");
        assert_eq!(SelectionMatch::Nth.as_str(), "nth");
        assert_eq!(OutputKind::Text.as_str(), "text");
        assert_eq!(OutputKind::InnerHtml.as_str(), "inner_html");
        assert_eq!(OutputKind::OuterHtml.as_str(), "outer_html");
        assert_eq!(
            CompareBasis::CanonicalTextSha256.as_str(),
            "canonical_text_sha256"
        );
        assert_eq!(RunMode::Live.as_str(), "live");
        assert_eq!(RunMode::DryRun.as_str(), "dry_run");
        assert_eq!(FailureClass::Transient.as_str(), "transient");
        assert_eq!(FailureClass::Permanent.as_str(), "permanent");
        assert_eq!(RunOutcome::Initialized.as_str(), "initialized");
        assert_eq!(RunOutcome::Changed.as_str(), "changed");
        assert_eq!(RunOutcome::Unchanged.as_str(), "unchanged");
        assert_eq!(RunOutcome::FailedTransient.as_str(), "failed_transient");
        assert_eq!(RunOutcome::FailedPermanent.as_str(), "failed_permanent");
        assert_eq!(RunOutcome::SkippedDisabled.as_str(), "skipped_disabled");
        assert_eq!(RunFailureCause::ConfigInvalid.as_str(), "config_invalid");
        assert_eq!(
            RunFailureCause::TargetUnavailable.as_str(),
            "target_unavailable"
        );
        assert_eq!(RunFailureCause::StateInvalid.as_str(), "state_invalid");
        assert_eq!(
            RunFailureCause::LockUnavailable.as_str(),
            "lock_unavailable"
        );
        assert_eq!(
            RunFailureCause::FetchHttpClientError.as_str(),
            "fetch_http_client_error"
        );
        assert_eq!(
            RunFailureCause::FetchHttpServerError.as_str(),
            "fetch_http_server_error"
        );
        assert_eq!(
            RunFailureCause::FetchSourceError.as_str(),
            "fetch_source_error"
        );
        assert_eq!(
            RunFailureCause::FetchNetworkError.as_str(),
            "fetch_network_error"
        );
        assert_eq!(RunFailureCause::FetchTimeout.as_str(), "fetch_timeout");
        assert_eq!(RunFailureCause::FetchTooLarge.as_str(), "fetch_too_large");
        assert_eq!(
            RunFailureCause::FetchUnsupportedContentType.as_str(),
            "fetch_unsupported_content_type"
        );
        assert_eq!(
            RunFailureCause::FetchDecodeError.as_str(),
            "fetch_decode_error"
        );
        assert_eq!(
            RunFailureCause::SelectionContractInvalid.as_str(),
            "selection_contract_invalid"
        );
        assert_eq!(
            RunFailureCause::SelectionNoMatch.as_str(),
            "selection_no_match"
        );
        assert_eq!(
            RunFailureCause::SelectionAmbiguousMatch.as_str(),
            "selection_ambiguous_match"
        );
        assert_eq!(
            RunFailureCause::SelectionInternalError.as_str(),
            "selection_internal_error"
        );
        assert_eq!(
            RunFailureCause::CanonicalizationError.as_str(),
            "canonicalization_error"
        );
        assert_eq!(RunFailureCause::CompareError.as_str(), "compare_error");
        assert_eq!(RunFailureCause::PersistError.as_str(), "persist_error");
        assert_eq!(
            RunFailureCause::IntegrityMismatch.as_str(),
            "integrity_mismatch"
        );
        assert_eq!(ChangeKind::Initialized.as_str(), "initialized");
        assert_eq!(ChangeKind::Changed.as_str(), "changed");
        assert_eq!(ChangeKind::Unchanged.as_str(), "unchanged");
        assert_eq!(BaselinePhase::NeverSucceeded.as_str(), "never_succeeded");
        assert_eq!(BaselinePhase::HasBaseline.as_str(), "has_baseline");
    }

    #[test]
    fn run_failure_cause_failure_class_maps_transient_and_permanent_outcomes() {
        assert_eq!(
            RunFailureCause::FetchTimeout.failure_class(),
            FailureClass::Transient
        );
        assert_eq!(
            RunFailureCause::FetchSourceError.failure_class(),
            FailureClass::Permanent
        );
        assert_eq!(
            RunFailureCause::ConfigInvalid.failure_class(),
            FailureClass::Permanent
        );
        assert_eq!(
            RunFailureCause::PersistError.run_outcome(),
            RunOutcome::FailedTransient
        );
        assert_eq!(
            RunFailureCause::CompareError.run_outcome(),
            RunOutcome::FailedPermanent
        );
    }
}

/// Supported execution mode vocabulary.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RunMode {
    /// Normal run with persistence and notifications enabled.
    Live,
    /// Validation and extraction smoke test without persistence.
    DryRun,
}

/// Failure classification used by run reports and notifications.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FailureClass {
    /// Retry-later failure.
    Transient,
    /// Investigate-now failure.
    Permanent,
}

/// Supported run-outcome vocabulary.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RunOutcome {
    /// First successful baseline capture.
    Initialized,
    /// Successful change.
    Changed,
    /// Successful no-change run.
    Unchanged,
    /// Structured retry-later failure.
    FailedTransient,
    /// Structured investigate-now failure.
    FailedPermanent,
    /// Target disabled.
    SkippedDisabled,
}

/// Supported change-summary vocabulary.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChangeKind {
    /// First successful baseline capture.
    Initialized,
    /// Content changed relative to the baseline.
    Changed,
    /// Content did not change.
    Unchanged,
}

impl ChangeKind {
    /// Returns the stable schema vocabulary token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initialized => "initialized",
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
        }
    }
}

/// Supported baseline-phase vocabulary.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BaselinePhase {
    /// No successful baseline exists.
    NeverSucceeded,
    /// One current baseline exists.
    HasBaseline,
}

impl BaselinePhase {
    /// Returns the stable schema vocabulary token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeverSucceeded => "never_succeeded",
            Self::HasBaseline => "has_baseline",
        }
    }
}

/// Supported run-failure cause vocabulary.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RunFailureCause {
    /// Target config invalid.
    ConfigInvalid,
    /// Target document unavailable at its resolved path.
    TargetUnavailable,
    /// State invalid.
    StateInvalid,
    /// Exclusive lock unavailable.
    LockUnavailable,
    /// HTTP 3xx/4xx failure.
    FetchHttpClientError,
    /// HTTP 5xx failure.
    FetchHttpServerError,
    /// Local source access failure.
    FetchSourceError,
    /// Network failure.
    FetchNetworkError,
    /// Timeout failure.
    FetchTimeout,
    /// Response exceeded max bytes.
    FetchTooLarge,
    /// Unsupported content type.
    FetchUnsupportedContentType,
    /// Decode failure.
    FetchDecodeError,
    /// FFHN selection contract invalid at the extractor boundary.
    SelectionContractInvalid,
    /// Extraction matched nothing.
    SelectionNoMatch,
    /// Extraction saw multiple exact matches.
    SelectionAmbiguousMatch,
    /// Extraction failed internally at the seam boundary.
    SelectionInternalError,
    /// Compare canonicalization failed.
    CanonicalizationError,
    /// Compare stage failed.
    CompareError,
    /// Persistence failed.
    PersistError,
    /// Snapshot artifacts do not match their recorded digests.
    IntegrityMismatch,
}

/// Supported snapshot slots.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SnapshotSlot {
    /// Current baseline.
    Current,
    /// Historical retained snapshot.
    History,
}

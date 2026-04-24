use serde::{Deserialize, Serialize};

/// Supported target source family in FFHN v1.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    /// HTTP or HTTPS source.
    Http,
    /// Local file source.
    File,
}

/// Supported fetch engines in FFHN v1.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FetchEngine {
    /// Raw HTTP fetch.
    Http,
    /// Browser-compatible fetch. The current rewrite keeps this contract stable while
    /// reusing the HTTP transport backend.
    Browser,
    /// Local file read.
    File,
}

/// Supported HTTP method vocabulary for FFHN v1.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum HttpMethod {
    /// GET.
    GET,
}

/// Supported selection strategy kinds.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelectionKind {
    /// CSS selector extraction.
    CssSelector,
    /// Delimiter-pair extraction.
    DelimiterPair,
}

/// Supported candidate selection modes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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
pub enum WhitespaceMode {
    /// Preserve whitespace.
    Preserve,
    /// Normalize whitespace.
    Normalize,
}

/// Supported delimiter matching modes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DelimiterMode {
    /// Literal substring boundaries.
    Literal,
    /// Regex boundaries.
    Regex,
}

/// Shared regex flag vocabulary used by FFHN target and canonicalization config.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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
pub enum CompareBasis {
    /// SHA-256 of compare-time canonical text.
    CanonicalTextSha256,
}

/// Supported canonicalizer kinds.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Trim => "trim",
            Self::CollapseWhitespace => "collapse_whitespace",
            Self::NormalizeNewlines => "normalize_newlines",
            Self::StripRegex => "strip_regex",
            Self::Lowercase => "lowercase",
        }
    }
}

impl RunMode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::DryRun => "dry_run",
        }
    }
}

impl FailureClass {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Transient => "transient",
            Self::Permanent => "permanent",
        }
    }
}

impl RunOutcome {
    pub(crate) const fn as_str(self) -> &'static str {
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
        assert_eq!(ReasonCode::Ok.as_str(), "ok");
        assert_eq!(ReasonCode::Disabled.as_str(), "disabled");
        assert_eq!(ReasonCode::ConfigInvalid.as_str(), "config_invalid");
        assert_eq!(ReasonCode::StateInvalid.as_str(), "state_invalid");
        assert_eq!(ReasonCode::LockUnavailable.as_str(), "lock_unavailable");
        assert_eq!(
            ReasonCode::FetchHttpClientError.as_str(),
            "fetch_http_client_error"
        );
        assert_eq!(
            ReasonCode::FetchHttpServerError.as_str(),
            "fetch_http_server_error"
        );
        assert_eq!(ReasonCode::FetchSourceError.as_str(), "fetch_source_error");
        assert_eq!(
            ReasonCode::FetchNetworkError.as_str(),
            "fetch_network_error"
        );
        assert_eq!(ReasonCode::FetchTimeout.as_str(), "fetch_timeout");
        assert_eq!(ReasonCode::FetchTooLarge.as_str(), "fetch_too_large");
        assert_eq!(
            ReasonCode::FetchUnsupportedContentType.as_str(),
            "fetch_unsupported_content_type"
        );
        assert_eq!(ReasonCode::FetchDecodeError.as_str(), "fetch_decode_error");
        assert_eq!(
            ReasonCode::ExtractionPlanInvalid.as_str(),
            "extraction_plan_invalid"
        );
        assert_eq!(
            ReasonCode::ExtractionNoMatch.as_str(),
            "extraction_no_match"
        );
        assert_eq!(
            ReasonCode::ExtractionAmbiguousMatch.as_str(),
            "extraction_ambiguous_match"
        );
        assert_eq!(
            ReasonCode::ExtractionInternalError.as_str(),
            "extraction_internal_error"
        );
        assert_eq!(
            ReasonCode::CanonicalizationError.as_str(),
            "canonicalization_error"
        );
        assert_eq!(ReasonCode::CompareError.as_str(), "compare_error");
        assert_eq!(ReasonCode::PersistError.as_str(), "persist_error");
        assert_eq!(ReasonCode::IntegrityMismatch.as_str(), "integrity_mismatch");
        assert_eq!(NotificationEvent::Initialized.as_str(), "initialized");
        assert_eq!(NotificationEvent::Changed.as_str(), "changed");
        assert_eq!(NotificationEvent::Unchanged.as_str(), "unchanged");
        assert_eq!(
            NotificationEvent::FailedTransient.as_str(),
            "failed_transient"
        );
        assert_eq!(
            NotificationEvent::FailedPermanent.as_str(),
            "failed_permanent"
        );
        assert_eq!(
            NotificationEvent::SkippedDisabled.as_str(),
            "skipped_disabled"
        );
    }

    #[test]
    fn reason_code_failure_class_maps_success_and_failures() {
        assert_eq!(ReasonCode::Ok.failure_class(), None);
        assert_eq!(ReasonCode::Disabled.failure_class(), None);
        assert_eq!(
            ReasonCode::FetchTimeout.failure_class(),
            Some(FailureClass::Transient)
        );
        assert_eq!(
            ReasonCode::FetchSourceError.failure_class(),
            Some(FailureClass::Permanent)
        );
        assert_eq!(
            ReasonCode::ConfigInvalid.failure_class(),
            Some(FailureClass::Permanent)
        );
    }
}

/// Supported target-status vocabulary.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetStatus {
    /// Valid target without a baseline.
    Pending,
    /// Valid target with a baseline.
    Ready,
    /// Invalid config or state.
    Invalid,
}

/// Supported execution mode vocabulary.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// Normal run with persistence and notifications enabled.
    Live,
    /// Validation and extraction smoke test without persistence.
    DryRun,
}

/// Failure classification used by run reports and notifications.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// Retry-later failure.
    Transient,
    /// Investigate-now failure.
    Permanent,
}

/// Supported run-outcome vocabulary.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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
pub enum ChangeKind {
    /// First successful baseline capture.
    Initialized,
    /// Content changed relative to the baseline.
    Changed,
    /// Content did not change.
    Unchanged,
}

/// Supported state-phase vocabulary.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StatePhase {
    /// No successful baseline exists.
    NeverSucceeded,
    /// One current baseline exists.
    HasBaseline,
}

/// Supported reason-code vocabulary.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    /// Successful or valid state.
    Ok,
    /// Target disabled.
    Disabled,
    /// Target config invalid.
    ConfigInvalid,
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
    /// HTMLCut plan invalid.
    ExtractionPlanInvalid,
    /// HTMLCut matched nothing.
    ExtractionNoMatch,
    /// HTMLCut saw multiple exact matches.
    ExtractionAmbiguousMatch,
    /// HTMLCut internal failure.
    ExtractionInternalError,
    /// Compare canonicalization failed.
    CanonicalizationError,
    /// Compare stage failed.
    CompareError,
    /// Persistence failed.
    PersistError,
    /// Snapshot artifacts do not match their recorded digests.
    IntegrityMismatch,
}

impl ReasonCode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Disabled => "disabled",
            Self::ConfigInvalid => "config_invalid",
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
            Self::ExtractionPlanInvalid => "extraction_plan_invalid",
            Self::ExtractionNoMatch => "extraction_no_match",
            Self::ExtractionAmbiguousMatch => "extraction_ambiguous_match",
            Self::ExtractionInternalError => "extraction_internal_error",
            Self::CanonicalizationError => "canonicalization_error",
            Self::CompareError => "compare_error",
            Self::PersistError => "persist_error",
            Self::IntegrityMismatch => "integrity_mismatch",
        }
    }

    pub(crate) const fn failure_class(self) -> Option<FailureClass> {
        match self {
            Self::Ok | Self::Disabled => None,
            Self::LockUnavailable
            | Self::FetchHttpServerError
            | Self::FetchNetworkError
            | Self::FetchTimeout
            | Self::PersistError => Some(FailureClass::Transient),
            Self::ConfigInvalid
            | Self::StateInvalid
            | Self::FetchHttpClientError
            | Self::FetchSourceError
            | Self::FetchTooLarge
            | Self::FetchUnsupportedContentType
            | Self::FetchDecodeError
            | Self::ExtractionPlanInvalid
            | Self::ExtractionNoMatch
            | Self::ExtractionAmbiguousMatch
            | Self::ExtractionInternalError
            | Self::CanonicalizationError
            | Self::CompareError
            | Self::IntegrityMismatch => Some(FailureClass::Permanent),
        }
    }
}

/// Notification event vocabulary used in target configuration.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationEvent {
    /// First successful baseline capture.
    Initialized,
    /// Successful change.
    Changed,
    /// Successful no-change run.
    Unchanged,
    /// Retry-later failure.
    FailedTransient,
    /// Investigate-now failure.
    FailedPermanent,
    /// Target disabled.
    SkippedDisabled,
}

impl NotificationEvent {
    pub(crate) const fn as_str(self) -> &'static str {
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

/// Supported snapshot slots.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotSlot {
    /// Current baseline.
    Current,
    /// Historical retained snapshot.
    History,
}

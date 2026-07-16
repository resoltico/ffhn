use serde::{Deserialize, Serialize};

/// The closed M3 vocabulary for source-suspect health episodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceSuspectReason {
    /// File, network, HTTP-status, bounded-read, or UTF-8 acquisition failed.
    FetchFailed,
    /// The acquired source could not be decoded as JSON.
    JsonMalformed,
    /// The configured JSON Pointer did not select a value.
    JsonMissingPointerTarget,
    /// The configured JSON Pointer selected an array or object rather than a scalar.
    JsonNonScalarPointerTarget,
    /// A selected JSON scalar could not satisfy the target's declared type contract.
    ValueUnparseable,
    /// HTMLCut found no candidate for the configured HTML selection.
    HtmlcutNoMatch,
    /// HTMLCut found multiple candidates for an exact-one HTML selection.
    HtmlcutAmbiguousMatch,
    /// HTMLCut could not find the configured attribute on the selected CSS match.
    HtmlcutMissingAttribute,
    /// HTMLCut could not select the configured candidate index.
    HtmlcutMatchIndexOutOfRange,
    /// HTMLCut failed without a classifiable target-contract diagnostic.
    HtmlcutInternalFailure,
}

impl SourceSuspectReason {
    /// Returns the stable persisted and event-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FetchFailed => "fetch_failed",
            Self::JsonMalformed => "json_malformed",
            Self::JsonMissingPointerTarget => "json_missing_pointer_target",
            Self::JsonNonScalarPointerTarget => "json_non_scalar_pointer_target",
            Self::ValueUnparseable => "value_unparseable",
            Self::HtmlcutNoMatch => "htmlcut_no_match",
            Self::HtmlcutAmbiguousMatch => "htmlcut_ambiguous_match",
            Self::HtmlcutMissingAttribute => "htmlcut_missing_attribute",
            Self::HtmlcutMatchIndexOutOfRange => "htmlcut_match_index_out_of_range",
            Self::HtmlcutInternalFailure => "htmlcut_internal_failure",
        }
    }
}

/// The complete M3 vocabulary for permanent measurement-contract errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermanentErrorCode {
    /// The configured projection is not a valid RFC 6901 JSON Pointer.
    InvalidJsonPointer,
    /// An HTMLCut selection plan violates its public v1 contract.
    HtmlcutPlanInvalid,
    /// The configured source cannot be represented as an HTMLCut input and base URL.
    HtmlcutInputInvalid,
    /// The configured HTMLCut CSS selector is invalid.
    HtmlcutInvalidSelector,
    /// The configured HTMLCut delimiter pattern is invalid.
    HtmlcutInvalidSlicePattern,
    /// The configured HTMLCut output type is unsupported by its strategy.
    HtmlcutUnsupportedValueType,
    /// An HTML attribute projection requires CSS selector match metadata.
    HtmlAttributeRequiresCssSelector,
    /// One FFHN target cannot acquire every HTMLCut candidate.
    HtmlSelectionMustSelectOne,
}

impl PermanentErrorCode {
    /// Returns the stable persisted and event-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidJsonPointer => "invalid_json_pointer",
            Self::HtmlcutPlanInvalid => "htmlcut_plan_invalid",
            Self::HtmlcutInputInvalid => "htmlcut_input_invalid",
            Self::HtmlcutInvalidSelector => "htmlcut_invalid_selector",
            Self::HtmlcutInvalidSlicePattern => "htmlcut_invalid_slice_pattern",
            Self::HtmlcutUnsupportedValueType => "htmlcut_unsupported_value_type",
            Self::HtmlAttributeRequiresCssSelector => "html_attribute_requires_css_selector",
            Self::HtmlSelectionMustSelectOne => "html_selection_must_select_one",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_failure_vocabularies_have_their_persisted_spellings() {
        assert_eq!(SourceSuspectReason::FetchFailed.as_str(), "fetch_failed");
        assert_eq!(
            SourceSuspectReason::JsonMalformed.as_str(),
            "json_malformed"
        );
        assert_eq!(
            SourceSuspectReason::JsonMissingPointerTarget.as_str(),
            "json_missing_pointer_target"
        );
        assert_eq!(
            SourceSuspectReason::JsonNonScalarPointerTarget.as_str(),
            "json_non_scalar_pointer_target"
        );
        assert_eq!(
            SourceSuspectReason::ValueUnparseable.as_str(),
            "value_unparseable"
        );
        assert_eq!(
            SourceSuspectReason::HtmlcutNoMatch.as_str(),
            "htmlcut_no_match"
        );
        assert_eq!(
            SourceSuspectReason::HtmlcutAmbiguousMatch.as_str(),
            "htmlcut_ambiguous_match"
        );
        assert_eq!(
            SourceSuspectReason::HtmlcutMissingAttribute.as_str(),
            "htmlcut_missing_attribute"
        );
        assert_eq!(
            SourceSuspectReason::HtmlcutMatchIndexOutOfRange.as_str(),
            "htmlcut_match_index_out_of_range"
        );
        assert_eq!(
            SourceSuspectReason::HtmlcutInternalFailure.as_str(),
            "htmlcut_internal_failure"
        );
        assert_eq!(
            PermanentErrorCode::InvalidJsonPointer.as_str(),
            "invalid_json_pointer"
        );
        assert_eq!(
            PermanentErrorCode::HtmlcutPlanInvalid.as_str(),
            "htmlcut_plan_invalid"
        );
        assert_eq!(
            PermanentErrorCode::HtmlcutInputInvalid.as_str(),
            "htmlcut_input_invalid"
        );
        assert_eq!(
            PermanentErrorCode::HtmlcutInvalidSelector.as_str(),
            "htmlcut_invalid_selector"
        );
        assert_eq!(
            PermanentErrorCode::HtmlcutInvalidSlicePattern.as_str(),
            "htmlcut_invalid_slice_pattern"
        );
        assert_eq!(
            PermanentErrorCode::HtmlcutUnsupportedValueType.as_str(),
            "htmlcut_unsupported_value_type"
        );
        assert_eq!(
            PermanentErrorCode::HtmlAttributeRequiresCssSelector.as_str(),
            "html_attribute_requires_css_selector"
        );
        assert_eq!(
            PermanentErrorCode::HtmlSelectionMustSelectOne.as_str(),
            "html_selection_must_select_one"
        );
    }
}

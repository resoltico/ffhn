//! Closed FFHN projection of the reachable HTMLCut diagnostic evidence contract.

mod interop;
mod validation;

use serde::{Deserialize, Serialize};

/// One HTMLCut diagnostic retained as FFHN measurement evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HtmlcutDiagnostic {
    level: HtmlcutDiagnosticLevel,
    code: HtmlcutDiagnosticCode,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<HtmlcutDiagnosticDetails>,
}

/// Stable HTMLCut diagnostic severity retained by FFHN.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HtmlcutDiagnosticLevel {
    /// HTMLCut could not complete the operation.
    Error,
    /// HTMLCut completed with a declared caveat.
    Warning,
    /// HTMLCut supplied supplemental information.
    Info,
}

impl HtmlcutDiagnosticLevel {
    /// Returns the stable HTMLCut spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

/// Stable HTMLCut diagnostic code retained by FFHN.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlcutDiagnosticCode {
    /// The supplied source could not be loaded.
    #[serde(rename = "SOURCE_LOAD_FAILED")]
    SourceLoadFailed,
    /// The upstream extraction specification is unsupported.
    #[serde(rename = "UNSUPPORTED_SPEC_VERSION")]
    UnsupportedSpecVersion,
    /// A CSS selector could not be parsed.
    #[serde(rename = "INVALID_SELECTOR")]
    InvalidSelector,
    /// A delimiter-pair pattern is invalid.
    #[serde(rename = "INVALID_SLICE_PATTERN")]
    InvalidSlicePattern,
    /// The requested output is not supported by the extraction strategy.
    #[serde(rename = "UNSUPPORTED_VALUE_TYPE")]
    UnsupportedValueType,
    /// No candidate matched the extraction.
    #[serde(rename = "NO_MATCH")]
    NoMatch,
    /// Exact-one selection found more than one candidate.
    #[serde(rename = "AMBIGUOUS_MATCH")]
    AmbiguousMatch,
    /// The requested match index did not exist.
    #[serde(rename = "MATCH_INDEX_OUT_OF_RANGE")]
    MatchIndexOutOfRange,
    /// A selected candidate omitted the requested attribute.
    #[serde(rename = "MISSING_ATTRIBUTE")]
    MissingAttribute,
    /// First-match selection found more than one candidate.
    #[serde(rename = "MULTIPLE_MATCHES")]
    MultipleMatches,
    /// Relative URL rewriting had no effective base URL.
    #[serde(rename = "EFFECTIVE_BASE_URL_UNRESOLVED")]
    EffectiveBaseUrlUnresolved,
    /// A delimiter slice appears to cut through markup.
    #[serde(rename = "SLICE_SPLITS_MARKUP")]
    SliceSplitsMarkup,
}

impl HtmlcutDiagnosticCode {
    /// Returns the stable HTMLCut spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceLoadFailed => "SOURCE_LOAD_FAILED",
            Self::UnsupportedSpecVersion => "UNSUPPORTED_SPEC_VERSION",
            Self::InvalidSelector => "INVALID_SELECTOR",
            Self::InvalidSlicePattern => "INVALID_SLICE_PATTERN",
            Self::UnsupportedValueType => "UNSUPPORTED_VALUE_TYPE",
            Self::NoMatch => "NO_MATCH",
            Self::AmbiguousMatch => "AMBIGUOUS_MATCH",
            Self::MatchIndexOutOfRange => "MATCH_INDEX_OUT_OF_RANGE",
            Self::MissingAttribute => "MISSING_ATTRIBUTE",
            Self::MultipleMatches => "MULTIPLE_MATCHES",
            Self::EffectiveBaseUrlUnresolved => "EFFECTIVE_BASE_URL_UNRESOLVED",
            Self::SliceSplitsMarkup => "SLICE_SPLITS_MARKUP",
        }
    }
}

/// Closed structured detail families emitted by the pinned HTMLCut interop profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HtmlcutDiagnosticDetails {
    /// A CSS selector parse location and normalized parser classification.
    SelectorParse {
        /// One-based source line and UTF-16 column.
        selector_parse: HtmlcutSelectorParse,
    },
    /// Candidate facts emitted by HTMLCut selection.
    CandidateSelection {
        /// Candidate count before selection.
        candidate_count: usize,
        /// One-based configured index, when HTMLCut supplied one.
        #[serde(skip_serializing_if = "Option::is_none")]
        requested_index: Option<usize>,
        /// One-based selected index, when HTMLCut supplied one.
        #[serde(skip_serializing_if = "Option::is_none")]
        selected_index: Option<usize>,
    },
    /// URL-rewrite evidence when an effective base could not be determined.
    EffectiveBaseUrlUnresolved {
        /// The document-declared base URL, when present.
        #[serde(skip_serializing_if = "Option::is_none")]
        document_base_href: Option<String>,
        /// Whether the execution requested URL rewriting.
        rewrite_requested: bool,
    },
    /// Every selected delimiter fragment whose boundaries appear to cross markup.
    SliceSplitsMarkup {
        /// Affected selected matches in returned order.
        affected_matches: Vec<HtmlcutSliceMarkupMatch>,
    },
    /// Delimiter-pattern evidence for missing or invalid boundaries.
    SlicePattern {
        /// Configured start delimiter, when supplied.
        #[serde(skip_serializing_if = "Option::is_none")]
        from: Option<String>,
        /// Configured end delimiter, when supplied.
        #[serde(skip_serializing_if = "Option::is_none")]
        to: Option<String>,
        /// Start-delimiter byte offset when the end delimiter was missing.
        #[serde(skip_serializing_if = "Option::is_none")]
        offset: Option<usize>,
        /// Invalid regex pattern, when supplied.
        #[serde(skip_serializing_if = "Option::is_none")]
        pattern: Option<String>,
        /// Regex flags, when supplied.
        #[serde(skip_serializing_if = "Option::is_none")]
        flags: Option<String>,
    },
    /// A strategy/output combination rejected by HTMLCut.
    UnsupportedValueType {
        /// HTMLCut strategy identifier.
        strategy: String,
        /// Requested output value identifier.
        value: String,
        /// Affected path, when HTMLCut supplied one.
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    /// Attribute evidence supplied by selector or delimiter extraction.
    MissingAttribute {
        /// Requested attribute name.
        attribute: String,
        /// CSS path, when selector extraction supplied one.
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        /// Delimiter-slice range, when delimiter extraction supplied one.
        #[serde(skip_serializing_if = "Option::is_none")]
        selected_range: Option<HtmlcutByteRange>,
        /// Boundary-retention guidance, when HTMLCut supplied one.
        #[serde(skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
    },
}

impl HtmlcutDiagnosticDetails {
    /// Returns selector-parse evidence when this closed detail family carries it.
    pub(crate) const fn selector_parse(&self) -> Option<&HtmlcutSelectorParse> {
        match self {
            Self::SelectorParse { selector_parse } => Some(selector_parse),
            Self::CandidateSelection { .. }
            | Self::EffectiveBaseUrlUnresolved { .. }
            | Self::SliceSplitsMarkup { .. }
            | Self::SlicePattern { .. }
            | Self::UnsupportedValueType { .. }
            | Self::MissingAttribute { .. } => None,
        }
    }
}

/// One HTMLCut delimiter match affected by a markup-splitting warning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HtmlcutSliceMarkupMatch {
    /// One-based returned match position.
    match_index: usize,
    /// One-based candidate position.
    candidate_index: usize,
    /// Selected half-open byte range.
    selected_range: HtmlcutByteRange,
}

impl HtmlcutSliceMarkupMatch {
    /// Returns the one-based returned match position.
    pub const fn match_index(&self) -> usize {
        self.match_index
    }

    /// Returns the one-based candidate position.
    pub const fn candidate_index(&self) -> usize {
        self.candidate_index
    }

    /// Returns the selected half-open byte range.
    pub const fn selected_range(&self) -> &HtmlcutByteRange {
        &self.selected_range
    }
}

/// One half-open byte range emitted by HTMLCut delimiter extraction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HtmlcutByteRange {
    /// Inclusive byte offset at which the range starts.
    start: usize,
    /// Exclusive byte offset at which the range ends.
    end: usize,
}

impl HtmlcutByteRange {
    /// Returns the inclusive starting byte offset.
    pub const fn start(&self) -> usize {
        self.start
    }

    /// Returns the exclusive ending byte offset.
    pub const fn end(&self) -> usize {
        self.end
    }
}

/// One normalized HTMLCut selector parse location and class.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HtmlcutSelectorParse {
    /// One-based source line.
    line: u64,
    /// One-based UTF-16 source column.
    column_utf16: u64,
    /// Closed parser classification.
    parse_error_class: HtmlcutSelectorParseErrorClass,
}

impl HtmlcutSelectorParse {
    /// Returns the one-based source line.
    pub const fn line(&self) -> u64 {
        self.line
    }

    /// Returns the one-based UTF-16 source column.
    pub const fn column_utf16(&self) -> u64 {
        self.column_utf16
    }

    /// Returns the closed parser classification.
    pub const fn parse_error_class(&self) -> HtmlcutSelectorParseErrorClass {
        self.parse_error_class
    }
}

/// Closed selector-parser failure classes published by HTMLCut v12.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HtmlcutSelectorParseErrorClass {
    /// The parser encountered an unexpected token.
    UnexpectedToken,
    /// The selector ended before the parser could finish.
    EndOfInput,
    /// An at-rule was invalid.
    InvalidAtRule,
    /// An at-rule body was invalid.
    InvalidAtRuleBody,
    /// A qualified rule was invalid.
    InvalidQualifiedRule,
    /// A pseudo-element was missing its colon.
    PseudoElementExpectedColon,
    /// A pseudo-element was missing its identifier.
    PseudoElementExpectedIdent,
    /// An attribute selector was invalid.
    InvalidAttributeSelector,
    /// The selector was empty.
    EmptySelector,
    /// The selector ended with a combinator.
    DanglingCombinator,
    /// A non-compound selector was invalid in context.
    NonCompoundSelector,
    /// A non-pseudo-element followed `::slotted`.
    NonPseudoElementAfterSlotted,
    /// A pseudo-element after `::slotted` was invalid.
    InvalidPseudoElementAfterSlotted,
    /// A pseudo-element inside `:where` was invalid.
    InvalidPseudoElementInsideWhere,
    /// The parser entered an invalid state.
    InvalidState,
    /// An attribute selector contained an unexpected token.
    UnexpectedTokenInAttributeSelector,
    /// A pseudo selector lacked an identifier.
    NoIdentForPseudo,
    /// A pseudo class or element is unsupported.
    UnsupportedPseudoClassOrElement,
    /// The parser encountered an unexpected identifier.
    UnexpectedIdent,
    /// A namespace was expected.
    ExpectedNamespace,
    /// An attribute selector was missing its namespace bar.
    ExpectedBarInAttributeSelector,
    /// An attribute selector value was invalid.
    InvalidAttributeValue,
    /// An attribute selector qualified name was invalid.
    InvalidQualifiedNameInAttributeSelector,
    /// A namespace used an unexpected token.
    ExplicitNamespaceUnexpectedToken,
    /// A class selector lacked an identifier.
    ClassNeedsIdent,
}

impl HtmlcutSelectorParseErrorClass {
    /// Returns the stable HTMLCut report-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnexpectedToken => "unexpected_token",
            Self::EndOfInput => "end_of_input",
            Self::InvalidAtRule => "invalid_at_rule",
            Self::InvalidAtRuleBody => "invalid_at_rule_body",
            Self::InvalidQualifiedRule => "invalid_qualified_rule",
            Self::PseudoElementExpectedColon => "pseudo_element_expected_colon",
            Self::PseudoElementExpectedIdent => "pseudo_element_expected_ident",
            Self::InvalidAttributeSelector => "invalid_attribute_selector",
            Self::EmptySelector => "empty_selector",
            Self::DanglingCombinator => "dangling_combinator",
            Self::NonCompoundSelector => "non_compound_selector",
            Self::NonPseudoElementAfterSlotted => "non_pseudo_element_after_slotted",
            Self::InvalidPseudoElementAfterSlotted => "invalid_pseudo_element_after_slotted",
            Self::InvalidPseudoElementInsideWhere => "invalid_pseudo_element_inside_where",
            Self::InvalidState => "invalid_state",
            Self::UnexpectedTokenInAttributeSelector => "unexpected_token_in_attribute_selector",
            Self::NoIdentForPseudo => "no_ident_for_pseudo",
            Self::UnsupportedPseudoClassOrElement => "unsupported_pseudo_class_or_element",
            Self::UnexpectedIdent => "unexpected_ident",
            Self::ExpectedNamespace => "expected_namespace",
            Self::ExpectedBarInAttributeSelector => "expected_bar_in_attribute_selector",
            Self::InvalidAttributeValue => "invalid_attribute_value",
            Self::InvalidQualifiedNameInAttributeSelector => {
                "invalid_qualified_name_in_attribute_selector"
            }
            Self::ExplicitNamespaceUnexpectedToken => "explicit_namespace_unexpected_token",
            Self::ClassNeedsIdent => "class_needs_ident",
        }
    }
}

impl HtmlcutDiagnostic {
    /// Returns HTMLCut's stable diagnostic severity.
    pub const fn level(&self) -> &'static str {
        self.level.as_str()
    }

    /// Returns HTMLCut's stable diagnostic code.
    pub const fn code(&self) -> &'static str {
        self.code.as_str()
    }

    /// Returns the HTMLCut diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns HTMLCut's closed structured detail when supplied.
    pub const fn details(&self) -> Option<&HtmlcutDiagnosticDetails> {
        self.details.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::HtmlcutSelectorParseErrorClass;

    #[test]
    fn selector_parse_class_display_spelling_matches_its_serialized_contract() {
        for class in [
            HtmlcutSelectorParseErrorClass::UnexpectedToken,
            HtmlcutSelectorParseErrorClass::EndOfInput,
            HtmlcutSelectorParseErrorClass::InvalidAtRule,
            HtmlcutSelectorParseErrorClass::InvalidAtRuleBody,
            HtmlcutSelectorParseErrorClass::InvalidQualifiedRule,
            HtmlcutSelectorParseErrorClass::PseudoElementExpectedColon,
            HtmlcutSelectorParseErrorClass::PseudoElementExpectedIdent,
            HtmlcutSelectorParseErrorClass::InvalidAttributeSelector,
            HtmlcutSelectorParseErrorClass::EmptySelector,
            HtmlcutSelectorParseErrorClass::DanglingCombinator,
            HtmlcutSelectorParseErrorClass::NonCompoundSelector,
            HtmlcutSelectorParseErrorClass::NonPseudoElementAfterSlotted,
            HtmlcutSelectorParseErrorClass::InvalidPseudoElementAfterSlotted,
            HtmlcutSelectorParseErrorClass::InvalidPseudoElementInsideWhere,
            HtmlcutSelectorParseErrorClass::InvalidState,
            HtmlcutSelectorParseErrorClass::UnexpectedTokenInAttributeSelector,
            HtmlcutSelectorParseErrorClass::NoIdentForPseudo,
            HtmlcutSelectorParseErrorClass::UnsupportedPseudoClassOrElement,
            HtmlcutSelectorParseErrorClass::UnexpectedIdent,
            HtmlcutSelectorParseErrorClass::ExpectedNamespace,
            HtmlcutSelectorParseErrorClass::ExpectedBarInAttributeSelector,
            HtmlcutSelectorParseErrorClass::InvalidAttributeValue,
            HtmlcutSelectorParseErrorClass::InvalidQualifiedNameInAttributeSelector,
            HtmlcutSelectorParseErrorClass::ExplicitNamespaceUnexpectedToken,
            HtmlcutSelectorParseErrorClass::ClassNeedsIdent,
        ] {
            assert_eq!(
                serde_json::to_string(&class).expect("serialized selector class"),
                format!("\"{}\"", class.as_str()),
            );
        }
    }
}

//! Translation from HTMLCut's public interop diagnostics into FFHN's closed evidence model.

use htmlcut_core::interop::v1::{InteropDiagnostic, InteropDiagnosticCode, InteropDiagnosticLevel};
use serde_json::{Map, Value};

use super::validation::unsupported_shape;
use super::{
    HtmlcutByteRange, HtmlcutDiagnostic, HtmlcutDiagnosticCode, HtmlcutDiagnosticDetails,
    HtmlcutDiagnosticLevel, HtmlcutSelectorParse, HtmlcutSelectorParseErrorClass,
    HtmlcutSliceMarkupMatch,
};
use crate::CoreError;

impl HtmlcutDiagnostic {
    /// Projects one published HTMLCut diagnostic into FFHN's complete closed evidence model.
    pub(crate) fn from_interop(diagnostic: InteropDiagnostic) -> Result<Self, CoreError> {
        let level = map_level(diagnostic.level);
        let code = map_code(diagnostic.code);
        let details = diagnostic
            .details
            .as_ref()
            .map(|value| HtmlcutDiagnosticDetails::from_interop(code, value))
            .transpose()?;
        let diagnostic = Self {
            level,
            code,
            message: diagnostic.message,
            details,
        };
        diagnostic.validate()?;
        Ok(diagnostic)
    }
}

impl HtmlcutDiagnosticDetails {
    fn from_interop(code: HtmlcutDiagnosticCode, value: &Value) -> Result<Self, CoreError> {
        let details = object(value)?;
        let result = match code {
            HtmlcutDiagnosticCode::InvalidSelector => {
                exact_keys(details, &["selector_parse"])?;
                Self::SelectorParse {
                    selector_parse: selector_parse(required(details, "selector_parse")?)?,
                }
            }
            HtmlcutDiagnosticCode::AmbiguousMatch => {
                exact_keys(details, &["candidateCount"])?;
                Self::CandidateSelection {
                    candidate_count: usize_required(details, "candidateCount")?,
                    requested_index: None,
                    selected_index: None,
                }
            }
            HtmlcutDiagnosticCode::MatchIndexOutOfRange => {
                exact_keys(details, &["requestedIndex", "candidateCount"])?;
                Self::CandidateSelection {
                    candidate_count: usize_required(details, "candidateCount")?,
                    requested_index: Some(usize_required(details, "requestedIndex")?),
                    selected_index: None,
                }
            }
            HtmlcutDiagnosticCode::MultipleMatches => {
                exact_keys(details, &["candidateCount", "selectedIndex"])?;
                Self::CandidateSelection {
                    candidate_count: usize_required(details, "candidateCount")?,
                    requested_index: None,
                    selected_index: Some(usize_required(details, "selectedIndex")?),
                }
            }
            HtmlcutDiagnosticCode::EffectiveBaseUrlUnresolved => {
                exact_keys(details, &["documentBaseHref", "rewriteRequested"])?;
                Self::EffectiveBaseUrlUnresolved {
                    document_base_href: string_optional_or_null(details, "documentBaseHref")?,
                    rewrite_requested: bool_required(details, "rewriteRequested")?,
                }
            }
            HtmlcutDiagnosticCode::SliceSplitsMarkup => {
                exact_keys(details, &["affectedMatches"])?;
                Self::SliceSplitsMarkup {
                    affected_matches: array_required(details, "affectedMatches")?
                        .iter()
                        .map(slice_markup_match)
                        .collect::<Result<_, _>>()?,
                }
            }
            HtmlcutDiagnosticCode::NoMatch => {
                exact_key_set(details, &[&["from", "to"], &["from", "to", "offset"]])?;
                Self::SlicePattern {
                    from: Some(string_required(details, "from")?),
                    to: Some(string_required(details, "to")?),
                    offset: usize_optional(details, "offset")?,
                    pattern: None,
                    flags: None,
                }
            }
            HtmlcutDiagnosticCode::InvalidSlicePattern => {
                exact_key_set(details, &[&["flags"], &["pattern", "flags"]])?;
                Self::SlicePattern {
                    from: None,
                    to: None,
                    offset: None,
                    pattern: string_optional(details, "pattern")?,
                    flags: string_optional(details, "flags")?,
                }
            }
            HtmlcutDiagnosticCode::UnsupportedValueType => {
                exact_key_set(
                    details,
                    &[&["strategy", "value"], &["strategy", "value", "path"]],
                )?;
                Self::UnsupportedValueType {
                    strategy: string_required(details, "strategy")?,
                    value: string_required(details, "value")?,
                    path: string_optional(details, "path")?,
                }
            }
            HtmlcutDiagnosticCode::MissingAttribute => {
                exact_key_set(
                    details,
                    &[
                        &["attribute", "path"],
                        &["attribute", "selectedRange", "hint"],
                    ],
                )?;
                Self::MissingAttribute {
                    attribute: string_required(details, "attribute")?,
                    path: string_optional(details, "path")?,
                    selected_range: optional(details, "selectedRange")
                        .map(byte_range)
                        .transpose()?,
                    hint: string_optional_or_null(details, "hint")?,
                }
            }
            HtmlcutDiagnosticCode::SourceLoadFailed
            | HtmlcutDiagnosticCode::UnsupportedSpecVersion => return Err(unsupported_shape()),
        };
        result.validate_for(code)?;
        Ok(result)
    }
}

fn map_level(level: InteropDiagnosticLevel) -> HtmlcutDiagnosticLevel {
    match level {
        InteropDiagnosticLevel::Error => HtmlcutDiagnosticLevel::Error,
        InteropDiagnosticLevel::Warning => HtmlcutDiagnosticLevel::Warning,
        InteropDiagnosticLevel::Info => HtmlcutDiagnosticLevel::Info,
    }
}

fn map_code(code: InteropDiagnosticCode) -> HtmlcutDiagnosticCode {
    match code {
        InteropDiagnosticCode::SourceLoadFailed => HtmlcutDiagnosticCode::SourceLoadFailed,
        InteropDiagnosticCode::UnsupportedSpecVersion => {
            HtmlcutDiagnosticCode::UnsupportedSpecVersion
        }
        InteropDiagnosticCode::InvalidSelector => HtmlcutDiagnosticCode::InvalidSelector,
        InteropDiagnosticCode::InvalidSlicePattern => HtmlcutDiagnosticCode::InvalidSlicePattern,
        InteropDiagnosticCode::UnsupportedValueType => HtmlcutDiagnosticCode::UnsupportedValueType,
        InteropDiagnosticCode::NoMatch => HtmlcutDiagnosticCode::NoMatch,
        InteropDiagnosticCode::AmbiguousMatch => HtmlcutDiagnosticCode::AmbiguousMatch,
        InteropDiagnosticCode::MatchIndexOutOfRange => HtmlcutDiagnosticCode::MatchIndexOutOfRange,
        InteropDiagnosticCode::MissingAttribute => HtmlcutDiagnosticCode::MissingAttribute,
        InteropDiagnosticCode::MultipleMatches => HtmlcutDiagnosticCode::MultipleMatches,
        InteropDiagnosticCode::EffectiveBaseUrlUnresolved => {
            HtmlcutDiagnosticCode::EffectiveBaseUrlUnresolved
        }
        InteropDiagnosticCode::SliceSplitsMarkup => HtmlcutDiagnosticCode::SliceSplitsMarkup,
    }
}

fn selector_parse(value: &Value) -> Result<HtmlcutSelectorParse, CoreError> {
    let details = object(value)?;
    exact_keys(details, &["line", "column_utf16", "parse_error_class"])?;
    let parse_error_class = match string_required(details, "parse_error_class")?.as_str() {
        "unexpected_token" => HtmlcutSelectorParseErrorClass::UnexpectedToken,
        "end_of_input" => HtmlcutSelectorParseErrorClass::EndOfInput,
        "invalid_at_rule" => HtmlcutSelectorParseErrorClass::InvalidAtRule,
        "invalid_at_rule_body" => HtmlcutSelectorParseErrorClass::InvalidAtRuleBody,
        "invalid_qualified_rule" => HtmlcutSelectorParseErrorClass::InvalidQualifiedRule,
        "pseudo_element_expected_colon" => {
            HtmlcutSelectorParseErrorClass::PseudoElementExpectedColon
        }
        "pseudo_element_expected_ident" => {
            HtmlcutSelectorParseErrorClass::PseudoElementExpectedIdent
        }
        "invalid_attribute_selector" => HtmlcutSelectorParseErrorClass::InvalidAttributeSelector,
        "empty_selector" => HtmlcutSelectorParseErrorClass::EmptySelector,
        "dangling_combinator" => HtmlcutSelectorParseErrorClass::DanglingCombinator,
        "non_compound_selector" => HtmlcutSelectorParseErrorClass::NonCompoundSelector,
        "non_pseudo_element_after_slotted" => {
            HtmlcutSelectorParseErrorClass::NonPseudoElementAfterSlotted
        }
        "invalid_pseudo_element_after_slotted" => {
            HtmlcutSelectorParseErrorClass::InvalidPseudoElementAfterSlotted
        }
        "invalid_pseudo_element_inside_where" => {
            HtmlcutSelectorParseErrorClass::InvalidPseudoElementInsideWhere
        }
        "invalid_state" => HtmlcutSelectorParseErrorClass::InvalidState,
        "unexpected_token_in_attribute_selector" => {
            HtmlcutSelectorParseErrorClass::UnexpectedTokenInAttributeSelector
        }
        "no_ident_for_pseudo" => HtmlcutSelectorParseErrorClass::NoIdentForPseudo,
        "unsupported_pseudo_class_or_element" => {
            HtmlcutSelectorParseErrorClass::UnsupportedPseudoClassOrElement
        }
        "unexpected_ident" => HtmlcutSelectorParseErrorClass::UnexpectedIdent,
        "expected_namespace" => HtmlcutSelectorParseErrorClass::ExpectedNamespace,
        "expected_bar_in_attribute_selector" => {
            HtmlcutSelectorParseErrorClass::ExpectedBarInAttributeSelector
        }
        "invalid_attribute_value" => HtmlcutSelectorParseErrorClass::InvalidAttributeValue,
        "invalid_qualified_name_in_attribute_selector" => {
            HtmlcutSelectorParseErrorClass::InvalidQualifiedNameInAttributeSelector
        }
        "explicit_namespace_unexpected_token" => {
            HtmlcutSelectorParseErrorClass::ExplicitNamespaceUnexpectedToken
        }
        "class_needs_ident" => HtmlcutSelectorParseErrorClass::ClassNeedsIdent,
        _ => return Err(unsupported_shape()),
    };
    let parsed = HtmlcutSelectorParse {
        line: u64_required(details, "line")?,
        column_utf16: u64_required(details, "column_utf16")?,
        parse_error_class,
    };
    parsed.validate()?;
    Ok(parsed)
}

fn slice_markup_match(value: &Value) -> Result<HtmlcutSliceMarkupMatch, CoreError> {
    let details = object(value)?;
    exact_keys(details, &["matchIndex", "candidateIndex", "selectedRange"])?;
    let parsed = HtmlcutSliceMarkupMatch {
        match_index: usize_required(details, "matchIndex")?,
        candidate_index: usize_required(details, "candidateIndex")?,
        selected_range: byte_range(required(details, "selectedRange")?)?,
    };
    parsed.validate()?;
    Ok(parsed)
}

fn byte_range(value: &Value) -> Result<HtmlcutByteRange, CoreError> {
    let details = object(value)?;
    exact_keys(details, &["start", "end"])?;
    let parsed = HtmlcutByteRange {
        start: usize_required(details, "start")?,
        end: usize_required(details, "end")?,
    };
    parsed.validate()?;
    Ok(parsed)
}

fn object(value: &Value) -> Result<&Map<String, Value>, CoreError> {
    value.as_object().ok_or_else(unsupported_shape)
}

fn required<'a>(
    details: &'a Map<String, Value>,
    name: &'static str,
) -> Result<&'a Value, CoreError> {
    details.get(name).ok_or_else(unsupported_shape)
}

fn optional<'a>(details: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
    details.get(name)
}

fn string_required(details: &Map<String, Value>, name: &'static str) -> Result<String, CoreError> {
    required(details, name)?
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(unsupported_shape)
}

fn string_optional(details: &Map<String, Value>, name: &str) -> Result<Option<String>, CoreError> {
    optional(details, name)
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(unsupported_shape)
        })
        .transpose()
}

fn string_optional_or_null(
    details: &Map<String, Value>,
    name: &str,
) -> Result<Option<String>, CoreError> {
    optional(details, name)
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(unsupported_shape)
        })
        .transpose()
}

fn bool_required(details: &Map<String, Value>, name: &'static str) -> Result<bool, CoreError> {
    required(details, name)?
        .as_bool()
        .ok_or_else(unsupported_shape)
}

fn u64_required(details: &Map<String, Value>, name: &'static str) -> Result<u64, CoreError> {
    required(details, name)?
        .as_u64()
        .ok_or_else(unsupported_shape)
}

fn usize_required(details: &Map<String, Value>, name: &'static str) -> Result<usize, CoreError> {
    usize::try_from(u64_required(details, name)?).map_err(|_| unsupported_shape())
}

fn usize_optional(details: &Map<String, Value>, name: &str) -> Result<Option<usize>, CoreError> {
    optional(details, name)
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(unsupported_shape)
        })
        .transpose()
}

fn array_required<'a>(
    details: &'a Map<String, Value>,
    name: &'static str,
) -> Result<&'a Vec<Value>, CoreError> {
    required(details, name)?
        .as_array()
        .ok_or_else(unsupported_shape)
}

fn exact_keys(details: &Map<String, Value>, expected: &[&str]) -> Result<(), CoreError> {
    exact_key_set(details, &[expected])
}

fn exact_key_set(details: &Map<String, Value>, accepted: &[&[&str]]) -> Result<(), CoreError> {
    accepted
        .iter()
        .any(|expected| {
            details.len() == expected.len() && expected.iter().all(|key| details.contains_key(*key))
        })
        .then_some(())
        .ok_or_else(unsupported_shape)
}

#[cfg(test)]
mod mutation_tests {
    use super::*;

    #[test]
    fn required_boolean_preserves_both_values_and_rejects_other_shapes() {
        for expected in [false, true] {
            let details = serde_json::json!({"value": expected})
                .as_object()
                .expect("object")
                .clone();
            assert_eq!(
                bool_required(&details, "value").expect("boolean value"),
                expected
            );
        }
        let missing = Map::new();
        assert!(bool_required(&missing, "value").is_err());
        let wrong = serde_json::json!({"value": "true"})
            .as_object()
            .expect("object")
            .clone();
        assert!(bool_required(&wrong, "value").is_err());
    }

    #[test]
    fn exact_key_guards_distinguish_exact_missing_extra_and_alternate_shapes() {
        let one = serde_json::json!({"a": 1})
            .as_object()
            .expect("object")
            .clone();
        assert!(exact_keys(&one, &["a"]).is_ok());
        assert!(exact_keys(&one, &["b"]).is_err());
        assert!(exact_keys(&one, &[]).is_err());

        let extra = serde_json::json!({"a": 1, "b": 2})
            .as_object()
            .expect("object")
            .clone();
        assert!(exact_keys(&extra, &["a"]).is_err());
        assert!(exact_key_set(&extra, &[&["a"], &["a", "b"]]).is_ok());
        assert!(exact_key_set(&extra, &[&["a"], &["b", "c"]]).is_err());
    }

    #[test]
    fn optional_or_null_string_preserves_present_absent_null_and_wrong_type() {
        let present = serde_json::json!({"value": "text"})
            .as_object()
            .expect("object")
            .clone();
        assert_eq!(
            string_optional_or_null(&present, "value").expect("present string"),
            Some("text".to_owned())
        );
        let null = serde_json::json!({"value": null})
            .as_object()
            .expect("object")
            .clone();
        assert_eq!(
            string_optional_or_null(&null, "value").expect("null string"),
            None
        );
        assert_eq!(
            string_optional_or_null(&Map::new(), "value").expect("missing string"),
            None
        );
        let wrong = serde_json::json!({"value": 7})
            .as_object()
            .expect("object")
            .clone();
        assert!(string_optional_or_null(&wrong, "value").is_err());
    }
}

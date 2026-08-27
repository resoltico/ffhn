//! Validation for FFHN's closed persisted HTMLCut evidence.

use super::{
    HtmlcutByteRange, HtmlcutDiagnostic, HtmlcutDiagnosticCode, HtmlcutDiagnosticDetails,
    HtmlcutSelectorParse, HtmlcutSliceMarkupMatch,
};
use crate::CoreError;

impl HtmlcutDiagnostic {
    /// Validates the complete released HTMLCut diagnostic vocabulary retained in FFHN state.
    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        if self.message.len() > 1_024 {
            return Err(CoreError::contract(
                "HTMLCut diagnostic message exceeds the upstream 1024-byte contract",
            ));
        }
        if let Some(details) = &self.details {
            details.validate_for(self.code)?;
        }
        Ok(())
    }
}

impl HtmlcutDiagnosticDetails {
    pub(super) fn validate_for(&self, code: HtmlcutDiagnosticCode) -> Result<(), CoreError> {
        match (code, self) {
            (HtmlcutDiagnosticCode::InvalidSelector, Self::SelectorParse { selector_parse }) => {
                selector_parse.validate()
            }
            (
                HtmlcutDiagnosticCode::AmbiguousMatch
                | HtmlcutDiagnosticCode::MatchIndexOutOfRange
                | HtmlcutDiagnosticCode::MultipleMatches,
                Self::CandidateSelection {
                    candidate_count,
                    requested_index,
                    selected_index,
                },
            ) => candidate_selection(code, *candidate_count, *requested_index, *selected_index),
            (
                HtmlcutDiagnosticCode::EffectiveBaseUrlUnresolved,
                Self::EffectiveBaseUrlUnresolved {
                    document_base_href, ..
                },
            ) => text_optional(document_base_href.as_deref()),
            (
                HtmlcutDiagnosticCode::SliceSplitsMarkup,
                Self::SliceSplitsMarkup { affected_matches },
            ) => {
                if affected_matches.is_empty() {
                    return Err(CoreError::contract(
                        "HTMLCut slice-splits-markup details require an affected match",
                    ));
                }
                affected_matches
                    .iter()
                    .try_for_each(HtmlcutSliceMarkupMatch::validate)
            }
            (
                HtmlcutDiagnosticCode::NoMatch,
                Self::SlicePattern {
                    from: Some(from),
                    to: Some(to),
                    offset,
                    pattern: None,
                    flags: None,
                },
            ) => {
                text_required(from)?;
                text_required(to)?;
                if offset.is_some_and(|offset| offset == 0) {
                    return Err(CoreError::contract(
                        "HTMLCut slice-pattern offset must be positive when present",
                    ));
                }
                Ok(())
            }
            (
                HtmlcutDiagnosticCode::InvalidSlicePattern,
                Self::SlicePattern {
                    from: None,
                    to: None,
                    offset: None,
                    pattern,
                    flags,
                },
            ) if pattern.is_some() || flags.is_some() => {
                text_optional(pattern.as_deref())?;
                text_optional(flags.as_deref())
            }
            (
                HtmlcutDiagnosticCode::UnsupportedValueType,
                Self::UnsupportedValueType {
                    strategy,
                    value,
                    path,
                },
            ) => {
                text_required(strategy)?;
                text_required(value)?;
                text_optional(path.as_deref())
            }
            (
                HtmlcutDiagnosticCode::MissingAttribute,
                Self::MissingAttribute {
                    attribute,
                    path,
                    selected_range,
                    hint,
                },
            ) => {
                text_required(attribute)?;
                match (path, selected_range, hint) {
                    (Some(path), None, None) => text_required(path),
                    (None, Some(range), hint) => {
                        range.validate()?;
                        text_optional(hint.as_deref())
                    }
                    _ => Err(CoreError::contract(
                        "HTMLCut missing-attribute details must describe either CSS or delimiter extraction",
                    )),
                }
            }
            _ => Err(CoreError::contract(
                "HTMLCut diagnostic details do not match the diagnostic code",
            )),
        }
    }
}

impl HtmlcutSliceMarkupMatch {
    pub(super) fn validate(&self) -> Result<(), CoreError> {
        if self.match_index == 0 || self.candidate_index == 0 {
            return Err(CoreError::contract(
                "HTMLCut markup-split match indexes must be positive",
            ));
        }
        self.selected_range.validate()
    }
}

impl HtmlcutByteRange {
    pub(super) fn validate(&self) -> Result<(), CoreError> {
        if self.start > self.end {
            return Err(CoreError::contract(
                "HTMLCut byte range start must not exceed end",
            ));
        }
        Ok(())
    }
}

impl HtmlcutSelectorParse {
    pub(super) fn validate(&self) -> Result<(), CoreError> {
        if self.line == 0 || self.column_utf16 == 0 {
            return Err(CoreError::contract(
                "HTMLCut selector-parse positions must be positive",
            ));
        }
        Ok(())
    }
}

fn candidate_selection(
    code: HtmlcutDiagnosticCode,
    candidate_count: usize,
    requested_index: Option<usize>,
    selected_index: Option<usize>,
) -> Result<(), CoreError> {
    match code {
        HtmlcutDiagnosticCode::AmbiguousMatch
            if candidate_count > 1 && requested_index.is_none() && selected_index.is_none() =>
        {
            Ok(())
        }
        HtmlcutDiagnosticCode::MatchIndexOutOfRange
            if requested_index.is_some_and(|index| index > candidate_count)
                && selected_index.is_none() =>
        {
            Ok(())
        }
        HtmlcutDiagnosticCode::MultipleMatches
            if candidate_count > 1 && requested_index.is_none() && selected_index == Some(1) =>
        {
            Ok(())
        }
        _ => Err(CoreError::contract(
            "HTMLCut candidate-selection details do not match the diagnostic code",
        )),
    }
}

fn text_required(value: &str) -> Result<(), CoreError> {
    if value.is_empty() || value.len() > 1_024 {
        Err(CoreError::contract(
            "HTMLCut diagnostic detail text must be non-empty and at most 1024 bytes",
        ))
    } else {
        Ok(())
    }
}

fn text_optional(value: Option<&str>) -> Result<(), CoreError> {
    value.map(text_required).transpose().map(|_| ())
}

pub(super) fn unsupported_shape() -> CoreError {
    CoreError::contract("HTMLCut emitted a diagnostic-details shape outside FFHN's pinned contract")
}

#[cfg(test)]
mod mutation_tests {
    use super::*;

    #[test]
    fn diagnostic_text_helpers_enforce_absence_and_both_length_boundaries() {
        assert!(text_required("x").is_ok());
        assert!(text_required(&"x".repeat(1_024)).is_ok());
        assert!(text_required("").is_err());
        assert!(text_required(&"x".repeat(1_025)).is_err());
        assert!(text_optional(None).is_ok());
        assert!(text_optional(Some("value")).is_ok());
        assert!(text_optional(Some("")).is_err());
        assert!(text_optional(Some(&"x".repeat(1_025))).is_err());
        assert!(
            unsupported_shape()
                .to_string()
                .contains("outside FFHN's pinned contract")
        );
    }
}

use regex::RegexBuilder;
use scraper::Selector;

use crate::CoreError;
use crate::model::validate::apply_regex_flag;

use super::{DelimiterMode, SelectionConfig};

pub(super) fn validate_htmlcut_selection_contract(
    selection: &SelectionConfig,
) -> Result<(), CoreError> {
    match selection {
        SelectionConfig::CssSelector { selector, .. } => {
            Selector::parse(selector)
                .map_err(|error| CoreError::contract(format!("invalid CSS selector: {error}")))?;
        }
        SelectionConfig::DelimiterPair {
            start,
            end,
            mode,
            flags,
            ..
        } => {
            if *mode == DelimiterMode::Regex {
                validate_regex_boundary("selection.start", start, flags)?;
                validate_regex_boundary("selection.end", end, flags)?;
            }
        }
    }

    Ok(())
}

fn validate_regex_boundary(
    field_name: &str,
    pattern: &str,
    flags: &[crate::RegexFlag],
) -> Result<(), CoreError> {
    let mut builder = RegexBuilder::new(pattern);
    builder.unicode(true);
    for flag in flags {
        apply_regex_flag(flag, &mut builder);
    }
    builder
        .build()
        .map(|_| ())
        .map_err(|error| CoreError::contract(format!("invalid {field_name} regex: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DelimiterMode, SelectionModeConfig};

    fn css_selection() -> SelectionConfig {
        SelectionConfig::CssSelector {
            selection_mode: SelectionModeConfig::Single,
            output: crate::OutputKind::OuterHtml,
            whitespace: crate::WhitespaceMode::Normalize,
            rewrite_urls: false,
            selector: "main".to_owned(),
        }
    }

    #[test]
    fn selection_contract_accepts_supported_selection_variants() {
        validate_htmlcut_selection_contract(&css_selection()).expect("css selection");

        let first_text = SelectionConfig::DelimiterPair {
            selection_mode: SelectionModeConfig::First,
            output: crate::OutputKind::Text,
            whitespace: crate::WhitespaceMode::Preserve,
            rewrite_urls: false,
            start: "BEGIN".to_owned(),
            end: "END".to_owned(),
            mode: DelimiterMode::Regex,
            include_start: false,
            include_end: true,
            flags: vec![
                crate::RegexFlag::CaseInsensitive,
                crate::RegexFlag::MultiLine,
                crate::RegexFlag::DotMatchesNewLine,
                crate::RegexFlag::SwapGreed,
                crate::RegexFlag::IgnoreWhitespace,
            ],
        };
        validate_htmlcut_selection_contract(&first_text).expect("first text selection");

        let nth_inner_html = SelectionConfig::DelimiterPair {
            selection_mode: SelectionModeConfig::Nth {
                index: std::num::NonZeroUsize::new(1).expect("non-zero index"),
            },
            output: crate::OutputKind::InnerHtml,
            whitespace: crate::WhitespaceMode::Normalize,
            rewrite_urls: false,
            start: "BEGIN".to_owned(),
            end: "END".to_owned(),
            mode: DelimiterMode::Regex,
            include_start: false,
            include_end: true,
            flags: vec![crate::RegexFlag::CaseInsensitive],
        };
        validate_htmlcut_selection_contract(&nth_inner_html).expect("nth inner-html selection");
    }

    #[test]
    fn selection_contract_reports_invalid_css_selectors() {
        let invalid = SelectionConfig::CssSelector {
            selection_mode: SelectionModeConfig::Single,
            output: crate::OutputKind::OuterHtml,
            whitespace: crate::WhitespaceMode::Normalize,
            rewrite_urls: false,
            selector: "main[".to_owned(),
        };

        let error =
            validate_htmlcut_selection_contract(&invalid).expect_err("invalid css selector");
        assert!(matches!(error, CoreError::Contract(_)));
    }

    #[test]
    fn selection_contract_reports_invalid_regex_boundaries() {
        let invalid = SelectionConfig::DelimiterPair {
            selection_mode: SelectionModeConfig::Single,
            output: crate::OutputKind::OuterHtml,
            whitespace: crate::WhitespaceMode::Normalize,
            rewrite_urls: false,
            start: "(".to_owned(),
            end: "END".to_owned(),
            mode: DelimiterMode::Regex,
            include_start: false,
            include_end: true,
            flags: Vec::new(),
        };

        let error =
            validate_htmlcut_selection_contract(&invalid).expect_err("invalid regex boundary");
        assert!(matches!(error, CoreError::Contract(_)));
    }
}

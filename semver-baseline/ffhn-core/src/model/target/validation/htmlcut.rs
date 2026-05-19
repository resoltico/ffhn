use htmlcut_core::interop::v1::{
    DelimiterBoundaryRetention, DelimiterBoundaryText, DelimiterMode as HtmlcutDelimiterMode,
    ErrorCode as HtmlcutErrorCode, HtmlInput, InteropError, Output, Plan, PlanStrategy,
    RegexFlag as HtmlcutRegexFlag, Rendering, Selection, TextWhitespace, execute_plan,
};
use regex::RegexBuilder;

use crate::CoreError;
use crate::model::validate::apply_regex_flag;

use super::{DelimiterMode, SelectionConfig};

pub(super) fn validate_htmlcut_selection_contract(
    selection: &SelectionConfig,
) -> Result<(), CoreError> {
    match selection {
        SelectionConfig::CssSelector { selector, .. } => {
            validate_selection_with_htmlcut(
                selection,
                PlanStrategy::css_selector(
                    htmlcut_core::interop::v1::CssSelectorText::new(selector.clone())
                        .map_err(|error| CoreError::contract(error.to_string()))?,
                ),
            )?;
        }
        SelectionConfig::DelimiterPair {
            start,
            end,
            mode,
            flags,
            include_start,
            include_end,
            ..
        } => {
            if *mode == DelimiterMode::Regex {
                validate_regex_boundary("selection.start", start, flags)?;
                validate_regex_boundary("selection.end", end, flags)?;
            }
            validate_selection_with_htmlcut(
                selection,
                PlanStrategy::delimiter_pair(
                    DelimiterBoundaryText::new(start.clone())
                        .map_err(|error| CoreError::contract(error.to_string()))?,
                    DelimiterBoundaryText::new(end.clone())
                        .map_err(|error| CoreError::contract(error.to_string()))?,
                    match mode {
                        DelimiterMode::Literal => HtmlcutDelimiterMode::Literal,
                        DelimiterMode::Regex => HtmlcutDelimiterMode::Regex,
                    },
                    DelimiterBoundaryRetention::from_flags(*include_start, *include_end),
                    flags.iter().copied().map(map_regex_flag).collect(),
                ),
            )?;
        }
    }

    Ok(())
}

fn validate_selection_with_htmlcut(
    selection: &SelectionConfig,
    strategy: PlanStrategy,
) -> Result<(), CoreError> {
    let input = HtmlInput::new(
        "validation",
        "<html><head><title>Validation</title></head><body><main><article class=\"release\"><a href=\"guide.html\">Guide</a></article><div id=\"payload\">BEGIN PAYLOAD Guide END PAYLOAD</div></main></body></html>",
    )
    .map_err(|error| CoreError::contract(error.to_string()))?;
    let plan = Plan::new(
        strategy,
        map_selection(selection),
        Output::text(),
        Rendering::new(TextWhitespace::Normalize, false),
    );

    match execute_plan(&input, &plan) {
        Ok(_) => Ok(()),
        Err(error) if error.error_code == HtmlcutErrorCode::PlanInvalid => {
            Err(CoreError::contract(htmlcut_error_message(error.as_ref())))
        }
        Err(_) => Ok(()),
    }
}

fn map_selection(selection: &SelectionConfig) -> Selection {
    match selection.selection_mode() {
        crate::SelectionModeConfig::Single => Selection::single(),
        crate::SelectionModeConfig::First => Selection::first(),
        crate::SelectionModeConfig::Nth { index } => Selection::nth(*index),
    }
}

fn map_regex_flag(flag: crate::RegexFlag) -> HtmlcutRegexFlag {
    match flag {
        crate::RegexFlag::CaseInsensitive => HtmlcutRegexFlag::CaseInsensitive,
        crate::RegexFlag::MultiLine => HtmlcutRegexFlag::MultiLine,
        crate::RegexFlag::DotMatchesNewLine => HtmlcutRegexFlag::DotMatchesNewLine,
        crate::RegexFlag::SwapGreed => HtmlcutRegexFlag::SwapGreed,
        crate::RegexFlag::IgnoreWhitespace => HtmlcutRegexFlag::IgnoreWhitespace,
    }
}

fn htmlcut_error_message(error: &InteropError) -> String {
    error
        .diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| error.message.clone())
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
            selector: "main".to_owned(),
        }
    }

    #[test]
    fn selection_contract_accepts_supported_selection_variants() {
        validate_htmlcut_selection_contract(&css_selection()).expect("css selection");

        let first_text = SelectionConfig::DelimiterPair {
            selection_mode: SelectionModeConfig::First,
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

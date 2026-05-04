use htmlcut_core::interop::v1::{ErrorCode, HtmlInput, execute_plan};

use crate::CoreError;
use crate::runtime::interop::build_htmlcut_plan;

use super::SelectionConfig;

pub(super) fn validate_htmlcut_selection_contract(
    selection: &SelectionConfig,
) -> Result<(), CoreError> {
    let plan = build_htmlcut_plan(selection)?;
    let probe_source = HtmlInput::new(
        "ffhn-target-validation",
        "<html><body><main>probe</main><article>probe</article></body></html>",
    )
    .map_err(|error| CoreError::internal(format!("validation probe HTML input failed: {error}")))?;
    if let Err(error) = execute_plan(&probe_source, &plan)
        && error.error_code == ErrorCode::PlanInvalid
    {
        let detail = error
            .details
            .get("contract_error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("selection is not valid for HTMLCut");
        return Err(CoreError::contract(detail.to_owned()));
    }

    Ok(())
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
    fn htmlcut_selection_contract_accepts_supported_selection_variants() {
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
    fn htmlcut_selection_contract_reports_invalid_css_selectors() {
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
}

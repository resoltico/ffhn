use htmlcut_core::interop::v1::{
    ByteRange as HtmlcutByteRange, CssSelectorText, DelimiterBoundaryRetention,
    DelimiterBoundaryText, DelimiterMode as HtmlcutDelimiterMode, HtmlInput, HttpUrl,
    InteropResult, Output, OutputKind as HtmlcutOutputKind, Plan, PlanStrategy,
    RegexFlag as HtmlcutRegexFlag, Rendering, SelectedMatchMetadata, Selection, SelectionMode,
    StrategyKind, TextWhitespace,
};
use url::Url;

use crate::{
    CoreError, DelimiterMode, OutputKind, RegexFlag, SelectionConfig, SelectionEvidence,
    SelectionKind, SelectionMatch, SelectionModeConfig, SelectionRange, TargetId, WhitespaceMode,
};

pub(crate) fn build_htmlcut_plan(selection: &SelectionConfig) -> Result<Plan, CoreError> {
    let strategy = match selection {
        SelectionConfig::CssSelector { selector, .. } => PlanStrategy::css_selector(
            CssSelectorText::new(selector.clone())
                .map_err(|error| CoreError::htmlcut_interop(error.to_string()))?,
        ),
        SelectionConfig::DelimiterPair {
            start,
            end,
            mode,
            include_start,
            include_end,
            flags,
            ..
        } => PlanStrategy::delimiter_pair(
            DelimiterBoundaryText::new(start.clone())
                .map_err(|error| CoreError::htmlcut_interop(error.to_string()))?,
            DelimiterBoundaryText::new(end.clone())
                .map_err(|error| CoreError::htmlcut_interop(error.to_string()))?,
            match mode {
                DelimiterMode::Literal => HtmlcutDelimiterMode::Literal,
                DelimiterMode::Regex => HtmlcutDelimiterMode::Regex,
            },
            DelimiterBoundaryRetention::from_flags(*include_start, *include_end),
            flags.iter().copied().map(map_regex_flag).collect(),
        ),
    };

    let selection_mode = match selection.selection_mode() {
        SelectionModeConfig::Single => Selection::single(),
        SelectionModeConfig::First => Selection::first(),
        SelectionModeConfig::Nth { index } => Selection::nth(*index),
    };

    let output = match selection.output_kind() {
        OutputKind::Text => Output::text(),
        OutputKind::InnerHtml => Output::inner_html(),
        OutputKind::OuterHtml => Output::outer_html(),
    };

    let rendering = Rendering::new(
        match selection.whitespace_mode() {
            WhitespaceMode::Preserve => TextWhitespace::Rendered,
            WhitespaceMode::Normalize => TextWhitespace::Normalize,
        },
        selection.rewrite_urls(),
    );

    Ok(Plan::new(strategy, selection_mode, output, rendering))
}

pub(crate) fn build_htmlcut_input(
    target_id: &TargetId,
    html: String,
    final_url: &Url,
) -> Result<HtmlInput, CoreError> {
    let input = HtmlInput::new(target_id.to_string(), html)
        .map_err(|error| CoreError::htmlcut_interop(error.to_string()))?;

    match final_url.scheme() {
        "http" | "https" => Ok(input.with_input_base_url(
            HttpUrl::try_from(final_url.clone())
                .map_err(|error| CoreError::htmlcut_interop(error.to_string()))?,
        )),
        _ => Ok(input),
    }
}

pub(crate) fn map_regex_flag(flag: RegexFlag) -> HtmlcutRegexFlag {
    match flag {
        RegexFlag::CaseInsensitive => HtmlcutRegexFlag::CaseInsensitive,
        RegexFlag::MultiLine => HtmlcutRegexFlag::MultiLine,
        RegexFlag::DotMatchesNewLine => HtmlcutRegexFlag::DotMatchesNewLine,
        RegexFlag::SwapGreed => HtmlcutRegexFlag::SwapGreed,
        RegexFlag::IgnoreWhitespace => HtmlcutRegexFlag::IgnoreWhitespace,
    }
}

pub(crate) fn map_strategy_kind(result: &InteropResult) -> Result<SelectionKind, CoreError> {
    match result.strategy_kind {
        StrategyKind::CssSelector => Ok(SelectionKind::CssSelector),
        StrategyKind::DelimiterPair => Ok(SelectionKind::DelimiterPair),
    }
}

pub(crate) fn map_selection_mode(result: &InteropResult) -> Result<SelectionMatch, CoreError> {
    match result.selection_mode {
        SelectionMode::Single => Ok(SelectionMatch::Single),
        SelectionMode::First => Ok(SelectionMatch::First),
        SelectionMode::Nth => Ok(SelectionMatch::Nth),
        SelectionMode::All => Err(CoreError::htmlcut_interop(
            "FFHN does not support HTMLCut selection_mode = all",
        )),
    }
}

pub(crate) fn map_output_kind(kind: HtmlcutOutputKind) -> Result<OutputKind, CoreError> {
    match kind {
        HtmlcutOutputKind::Text => Ok(OutputKind::Text),
        HtmlcutOutputKind::InnerHtml => Ok(OutputKind::InnerHtml),
        HtmlcutOutputKind::OuterHtml => Ok(OutputKind::OuterHtml),
        HtmlcutOutputKind::SelectedHtml
        | HtmlcutOutputKind::Attribute
        | HtmlcutOutputKind::Structured => Err(CoreError::htmlcut_interop(format!(
            "FFHN does not support HTMLCut output kind {kind}"
        ))),
    }
}

pub(crate) fn build_selection_evidence(metadata: &SelectedMatchMetadata) -> SelectionEvidence {
    match metadata {
        SelectedMatchMetadata::CssSelector { path, tag_name, .. } => {
            SelectionEvidence::CssSelector {
                path: path.clone(),
                tag_name: tag_name.clone(),
            }
        }
        SelectedMatchMetadata::DelimiterPair {
            selected_range,
            inner_range,
            outer_range,
            include_start,
            include_end,
            ..
        } => SelectionEvidence::DelimiterPair {
            selected_range: map_range(selected_range),
            inner_range: map_range(inner_range),
            outer_range: map_range(outer_range),
            include_start: *include_start,
            include_end: *include_end,
        },
    }
}

fn map_range(range: &HtmlcutByteRange) -> SelectionRange {
    SelectionRange {
        start_byte: range.start,
        end_byte: range.end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FetchConfig, NetworkFetchConfig, TargetDocument, TargetSource};
    use htmlcut_core::interop::v1::{InteropDiagnosticCode, execute_plan, prepare_plan};
    use url::Url;

    fn target_with_css() -> TargetDocument {
        TargetDocument {
            schema_name: crate::TARGET_SCHEMA_NAME.to_owned(),
            schema_version: crate::TARGET_SCHEMA_VERSION,
            target_id: crate::TargetId::new("demo").expect("target id"),
            display_name: "Demo".to_owned(),
            enabled: true,
            target: TargetSource::Http {
                source_url: Url::parse("https://example.com/page").expect("url"),
            },
            fetch: FetchConfig::Http(NetworkFetchConfig {
                method: crate::HttpMethod::GET,
                timeout_ms: 15_000,
                max_bytes: 2_000_000,
                user_agent: "ffhn/example".to_owned(),
                follow_redirects: true,
                accept: "text/html".to_owned(),
                headers: Default::default(),
                extensions: None,
            }),
            selection: crate::SelectionConfig::CssSelector {
                selection_mode: crate::SelectionModeConfig::Single,
                output: crate::OutputKind::OuterHtml,
                whitespace: crate::WhitespaceMode::Normalize,
                rewrite_urls: false,
                selector: "main".to_owned(),
            },
            compare: crate::CompareConfig {
                basis: crate::CompareBasis::CanonicalTextSha256,
                canonicalization: Vec::new(),
            },
            storage: Default::default(),
            notification_endpoints: Vec::new(),
            notification_routes: Vec::new(),
            extensions: None,
        }
    }

    #[test]
    fn build_htmlcut_plan_supports_css_selector_targets() {
        let target = target_with_css();
        let plan = build_htmlcut_plan(target.selection_config()).expect("css plan");
        prepare_plan(&plan).expect("valid css plan");
    }

    #[test]
    fn build_htmlcut_plan_supports_delimiter_targets_and_regex_flags() {
        let mut target = target_with_css();
        target.selection = crate::SelectionConfig::DelimiterPair {
            selection_mode: crate::SelectionModeConfig::Nth {
                index: std::num::NonZeroUsize::new(1).expect("non-zero index"),
            },
            output: crate::OutputKind::OuterHtml,
            whitespace: crate::WhitespaceMode::Normalize,
            rewrite_urls: false,
            start: "BEGIN".to_owned(),
            end: "END".to_owned(),
            mode: crate::DelimiterMode::Regex,
            include_start: false,
            include_end: true,
            flags: vec![crate::RegexFlag::CaseInsensitive],
        };

        let plan = build_htmlcut_plan(target.selection_config()).expect("delimiter plan");
        prepare_plan(&plan).expect("valid delimiter plan");
    }

    #[test]
    fn build_htmlcut_plan_supports_literal_delimiter_targets_without_regex_flags() {
        let mut target = target_with_css();
        target.selection = crate::SelectionConfig::DelimiterPair {
            selection_mode: crate::SelectionModeConfig::First,
            output: crate::OutputKind::OuterHtml,
            whitespace: crate::WhitespaceMode::Normalize,
            rewrite_urls: false,
            start: "BEGIN".to_owned(),
            end: "END".to_owned(),
            mode: crate::DelimiterMode::Literal,
            include_start: true,
            include_end: false,
            flags: Vec::new(),
        };

        let plan = build_htmlcut_plan(target.selection_config()).expect("literal delimiter plan");
        prepare_plan(&plan).expect("valid literal delimiter plan");
    }

    #[test]
    fn build_htmlcut_plan_covers_first_match_and_output_variants() {
        let mut target = target_with_css();
        target.selection = crate::SelectionConfig::CssSelector {
            selection_mode: crate::SelectionModeConfig::First,
            output: crate::OutputKind::InnerHtml,
            whitespace: crate::WhitespaceMode::Preserve,
            rewrite_urls: false,
            selector: "main".to_owned(),
        };

        let plan = build_htmlcut_plan(target.selection_config()).expect("first-match plan");
        prepare_plan(&plan).expect("valid first-match plan");

        target.selection = crate::SelectionConfig::CssSelector {
            selection_mode: crate::SelectionModeConfig::First,
            output: crate::OutputKind::Text,
            whitespace: crate::WhitespaceMode::Preserve,
            rewrite_urls: false,
            selector: "main".to_owned(),
        };
        let plan = build_htmlcut_plan(target.selection_config()).expect("text plan");
        prepare_plan(&plan).expect("valid text plan");
    }

    #[test]
    fn mapping_helpers_reflect_htmlcut_result_vocabulary() {
        let target = target_with_css();
        let plan = build_htmlcut_plan(target.selection_config()).expect("plan");
        let source = HtmlInput::new("demo".to_owned(), "<main>Hello</main>".to_owned())
            .expect("source")
            .with_input_base_url(HttpUrl::parse("https://example.com/page").expect("base url"));
        let result = execute_plan(&source, &plan).expect("execute plan");

        assert_eq!(
            map_strategy_kind(&result).expect("strategy"),
            crate::SelectionKind::CssSelector
        );
        assert_eq!(
            map_selection_mode(&result).expect("selection"),
            crate::SelectionMatch::Single
        );
        assert_eq!(
            map_regex_flag(crate::RegexFlag::IgnoreWhitespace),
            HtmlcutRegexFlag::IgnoreWhitespace
        );
    }

    #[test]
    fn mapping_helpers_cover_delimiter_and_remaining_flag_variants() {
        let mut target = target_with_css();
        target.selection = crate::SelectionConfig::DelimiterPair {
            selection_mode: crate::SelectionModeConfig::Nth {
                index: std::num::NonZeroUsize::new(1).expect("non-zero index"),
            },
            output: crate::OutputKind::OuterHtml,
            whitespace: crate::WhitespaceMode::Normalize,
            rewrite_urls: false,
            start: "BEGIN".to_owned(),
            end: "END".to_owned(),
            mode: crate::DelimiterMode::Regex,
            include_start: true,
            include_end: false,
            flags: vec![
                crate::RegexFlag::MultiLine,
                crate::RegexFlag::DotMatchesNewLine,
                crate::RegexFlag::SwapGreed,
            ],
        };

        let plan = build_htmlcut_plan(target.selection_config()).expect("delimiter plan");
        let source =
            HtmlInput::new("demo".to_owned(), "BEGIN\nValue\nEND".to_owned()).expect("source");
        let result = execute_plan(&source, &plan).expect("delimiter result");
        assert_eq!(
            map_strategy_kind(&result).expect("strategy"),
            crate::SelectionKind::DelimiterPair
        );
        assert_eq!(
            map_selection_mode(&result).expect("selection"),
            crate::SelectionMatch::Nth
        );

        let mut target = target_with_css();
        target.selection = crate::SelectionConfig::CssSelector {
            selection_mode: crate::SelectionModeConfig::First,
            output: crate::OutputKind::OuterHtml,
            whitespace: crate::WhitespaceMode::Normalize,
            rewrite_urls: false,
            selector: "main".to_owned(),
        };
        let plan = build_htmlcut_plan(target.selection_config()).expect("first plan");
        let source = HtmlInput::new(
            "demo".to_owned(),
            "<main>One</main><main>Two</main>".to_owned(),
        )
        .expect("source");
        let result = execute_plan(&source, &plan).expect("first result");
        assert_eq!(
            map_selection_mode(&result).expect("first selection"),
            crate::SelectionMatch::First
        );
        assert_eq!(
            map_regex_flag(crate::RegexFlag::MultiLine),
            HtmlcutRegexFlag::MultiLine
        );
        assert_eq!(
            map_regex_flag(crate::RegexFlag::DotMatchesNewLine),
            HtmlcutRegexFlag::DotMatchesNewLine
        );
        assert_eq!(
            map_regex_flag(crate::RegexFlag::SwapGreed),
            HtmlcutRegexFlag::SwapGreed
        );
    }

    #[test]
    fn map_output_kind_reflects_htmlcut_output_vocabulary() {
        assert_eq!(
            map_output_kind(HtmlcutOutputKind::Text).expect("text output kind"),
            OutputKind::Text
        );
        assert_eq!(
            map_output_kind(HtmlcutOutputKind::InnerHtml).expect("inner-html output kind"),
            OutputKind::InnerHtml
        );
        assert_eq!(
            map_output_kind(HtmlcutOutputKind::OuterHtml).expect("outer-html output kind"),
            OutputKind::OuterHtml
        );
        assert!(map_output_kind(HtmlcutOutputKind::SelectedHtml).is_err());
        assert!(map_output_kind(HtmlcutOutputKind::Attribute).is_err());
        assert!(map_output_kind(HtmlcutOutputKind::Structured).is_err());
    }

    #[test]
    fn map_selection_mode_rejects_htmlcut_all_selection_for_ffhn() {
        let plan = Plan::new(
            PlanStrategy::css_selector(CssSelectorText::new("main").expect("selector")),
            Selection::all(),
            Output::outer_html(),
            Rendering::new(TextWhitespace::Normalize, false),
        );
        let source = HtmlInput::new(
            "demo".to_owned(),
            "<main>One</main><main>Two</main>".to_owned(),
        )
        .expect("source");
        let result = execute_plan(&source, &plan).expect("all-selection result");

        let error = map_selection_mode(&result).expect_err("all selection should be rejected");
        assert!(
            error
                .to_string()
                .contains("does not support HTMLCut selection_mode = all")
        );
    }

    #[test]
    fn build_selection_evidence_covers_delimiter_range_translation() {
        let mut target = target_with_css();
        target.selection = crate::SelectionConfig::DelimiterPair {
            selection_mode: crate::SelectionModeConfig::Single,
            output: crate::OutputKind::OuterHtml,
            whitespace: crate::WhitespaceMode::Normalize,
            rewrite_urls: false,
            start: "BEGIN".to_owned(),
            end: "END".to_owned(),
            mode: crate::DelimiterMode::Literal,
            include_start: true,
            include_end: false,
            flags: Vec::new(),
        };

        let plan = build_htmlcut_plan(target.selection_config()).expect("delimiter plan");
        let source =
            HtmlInput::new("demo".to_owned(), "BEGIN Value END".to_owned()).expect("source");
        let result = execute_plan(&source, &plan).expect("delimiter result");
        let evidence = build_selection_evidence(&result.selected_matches[0].metadata);
        let evidence_json = serde_json::to_value(&evidence).expect("serialize evidence");
        assert_eq!(
            evidence_json["selected_range"]["start_byte"].as_u64(),
            Some(0)
        );
        assert_eq!(evidence_json["outer_range"]["start_byte"].as_u64(), Some(0));
        assert_eq!(evidence_json["include_start"].as_bool(), Some(true));
        assert_eq!(evidence_json["include_end"].as_bool(), Some(false));
    }

    #[test]
    fn build_htmlcut_input_keeps_http_base_urls_for_network_targets() {
        let final_url = Url::parse("https://example.com/docs/start.html").expect("url");
        let input = build_htmlcut_input(
            &crate::TargetId::new("demo").expect("target id"),
            "<main>Hello</main>".to_owned(),
            &final_url,
        )
        .expect("htmlcut input");
        assert_eq!(
            input.input_base_url.as_ref().map(|url| url.as_fetch_str()),
            Some(final_url.as_str())
        );
    }

    #[test]
    fn build_htmlcut_input_omits_file_urls_and_preserves_htmlcut_warning_behavior() {
        let target = TargetDocument {
            selection: crate::SelectionConfig::CssSelector {
                selection_mode: crate::SelectionModeConfig::Single,
                output: crate::OutputKind::OuterHtml,
                whitespace: crate::WhitespaceMode::Normalize,
                rewrite_urls: true,
                selector: "a".to_owned(),
            },
            ..target_with_css()
        };
        let plan = build_htmlcut_plan(target.selection_config()).expect("plan");
        let input = build_htmlcut_input(
            &target.target_id,
            "<a href=\"guide.html\">Guide</a>".to_owned(),
            &Url::parse("file:///tmp/source.html").expect("file url"),
        )
        .expect("htmlcut input");
        assert!(input.input_base_url.is_none());

        let result = execute_plan(&input, &plan).expect("execute plan");
        assert_eq!(result.source.input_base_url, None);
        assert_eq!(result.source.effective_base_url, None);
        assert_eq!(
            result.selected_matches[0].outer_html_output,
            "<a href=\"guide.html\">Guide</a>"
        );
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            vec![InteropDiagnosticCode::EffectiveBaseUrlUnresolved]
        );
    }

    #[test]
    fn build_htmlcut_input_leaves_file_targets_free_to_use_document_base_href() {
        let target = TargetDocument {
            selection: crate::SelectionConfig::CssSelector {
                selection_mode: crate::SelectionModeConfig::Single,
                output: crate::OutputKind::OuterHtml,
                whitespace: crate::WhitespaceMode::Normalize,
                rewrite_urls: true,
                selector: "a".to_owned(),
            },
            ..target_with_css()
        };
        let plan = build_htmlcut_plan(target.selection_config()).expect("plan");
        let input = build_htmlcut_input(
            &target.target_id,
            "<base href=\"https://example.com/docs/\"><a href=\"guide.html\">Guide</a>".to_owned(),
            &Url::parse("file:///tmp/source.html").expect("file url"),
        )
        .expect("htmlcut input");

        let result = execute_plan(&input, &plan).expect("execute plan");
        assert_eq!(result.source.input_base_url, None);
        assert_eq!(
            result
                .source
                .effective_base_url
                .as_ref()
                .map(|url| url.as_str()),
            Some("https://example.com/docs/")
        );
        assert!(result.diagnostics.is_empty());
        assert_eq!(
            result.selected_matches[0].outer_html_output,
            "<a href=\"https://example.com/docs/guide.html\">Guide</a>"
        );
    }
}

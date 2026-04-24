use std::num::NonZeroUsize;

use htmlcut_core::interop::v1::{
    DelimiterMode as HtmlcutDelimiterMode, InteropResult, Normalization, Output,
    OutputKind as HtmlcutOutputKind, Plan, PlanStrategy, RegexFlag as HtmlcutRegexFlag, Selection,
    SelectionMode, StrategyKind, TextWhitespace,
};
use htmlcut_core::{SelectorQuery, SliceBoundary};

use crate::{
    CoreError, DelimiterMode, OutputKind, RegexFlag, SelectionKind, SelectionMatch, TargetDocument,
    WhitespaceMode,
};

pub(crate) fn build_htmlcut_plan(target: &TargetDocument) -> Result<Plan, CoreError> {
    let strategy =
        match target.selection.kind {
            SelectionKind::CssSelector => PlanStrategy::css_selector(
                SelectorQuery::new(target.selection.selector.clone().ok_or_else(|| {
                    CoreError::internal("validated target is missing selection.selector")
                })?)
                .map_err(|error| CoreError::htmlcut_interop(error.to_string()))?,
            ),
            SelectionKind::DelimiterPair => PlanStrategy::delimiter_pair(
                SliceBoundary::new(target.selection.start.clone().ok_or_else(|| {
                    CoreError::internal("validated target is missing selection.start")
                })?)
                .map_err(|error| CoreError::htmlcut_interop(error.to_string()))?,
                SliceBoundary::new(target.selection.end.clone().ok_or_else(|| {
                    CoreError::internal("validated target is missing selection.end")
                })?)
                .map_err(|error| CoreError::htmlcut_interop(error.to_string()))?,
                match target.selection.mode.ok_or_else(|| {
                    CoreError::internal("validated target is missing selection.mode")
                })? {
                    DelimiterMode::Literal => HtmlcutDelimiterMode::Literal,
                    DelimiterMode::Regex => HtmlcutDelimiterMode::Regex,
                },
                target.selection.include_start.unwrap_or(false),
                target.selection.include_end.unwrap_or(false),
                target
                    .selection
                    .flags
                    .iter()
                    .copied()
                    .map(map_regex_flag)
                    .collect(),
            ),
        };

    let selection = match target.selection.r#match {
        SelectionMatch::Single => Selection::single(),
        SelectionMatch::First => Selection::first(),
        SelectionMatch::Nth => Selection::nth(
            NonZeroUsize::new(target.selection.index.ok_or_else(|| {
                CoreError::internal("validated target is missing selection.index")
            })?)
            .ok_or_else(|| {
                CoreError::internal("validated target has a non-positive selection.index")
            })?,
        ),
    };

    let output = Output::new(match target.selection.output {
        OutputKind::Text => HtmlcutOutputKind::Text,
        OutputKind::InnerHtml => HtmlcutOutputKind::InnerHtml,
        OutputKind::OuterHtml => HtmlcutOutputKind::OuterHtml,
    });

    let normalization = Normalization::new(
        match target.selection.whitespace {
            WhitespaceMode::Preserve => TextWhitespace::Preserve,
            WhitespaceMode::Normalize => TextWhitespace::Normalize,
        },
        target.selection.rewrite_urls,
    );

    Ok(Plan::new(strategy, selection, output, normalization))
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
    }
}

pub(crate) const fn map_output_kind(kind: HtmlcutOutputKind) -> OutputKind {
    match kind {
        HtmlcutOutputKind::Text => OutputKind::Text,
        HtmlcutOutputKind::InnerHtml => OutputKind::InnerHtml,
        HtmlcutOutputKind::OuterHtml => OutputKind::OuterHtml,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use htmlcut_core::interop::v1::{HtmlInput, execute_plan, validate_plan};
    use url::Url;

    fn target_with_css() -> TargetDocument {
        TargetDocument {
            schema_name: crate::TARGET_SCHEMA_NAME.to_owned(),
            schema_version: crate::TARGET_SCHEMA_VERSION,
            target_id: crate::TargetId::new("demo").expect("target id"),
            display_name: "Demo".to_owned(),
            enabled: true,
            target: crate::TargetSource {
                kind: crate::model::TargetKind::Http,
                source_url: Some(Url::parse("https://example.com/page").expect("url")),
                file_path: None,
            },
            fetch: crate::FetchConfig {
                engine: crate::FetchEngine::Http,
                method: crate::HttpMethod::GET,
                timeout_ms: 15_000,
                max_bytes: 2_000_000,
                user_agent: "ffhn/2.0.0".to_owned(),
                follow_redirects: true,
                accept: "text/html".to_owned(),
                headers: Default::default(),
                extensions: None,
            },
            selection: crate::SelectionConfig {
                kind: crate::SelectionKind::CssSelector,
                r#match: crate::SelectionMatch::Single,
                index: None,
                output: crate::OutputKind::OuterHtml,
                whitespace: crate::WhitespaceMode::Normalize,
                rewrite_urls: false,
                selector: Some("main".to_owned()),
                start: None,
                end: None,
                mode: None,
                include_start: None,
                include_end: None,
                flags: Vec::new(),
            },
            compare: crate::CompareConfig {
                basis: crate::CompareBasis::CanonicalTextSha256,
                canonicalization: Vec::new(),
            },
            storage: Default::default(),
            notifications: Vec::new(),
            extensions: None,
        }
    }

    #[test]
    fn build_htmlcut_plan_supports_css_selector_targets() {
        let plan = build_htmlcut_plan(&target_with_css()).expect("css plan");
        validate_plan(&plan).expect("valid css plan");
    }

    #[test]
    fn build_htmlcut_plan_supports_delimiter_targets_and_regex_flags() {
        let mut target = target_with_css();
        target.selection.kind = crate::SelectionKind::DelimiterPair;
        target.selection.r#match = crate::SelectionMatch::Nth;
        target.selection.index = Some(1);
        target.selection.selector = None;
        target.selection.start = Some("BEGIN".to_owned());
        target.selection.end = Some("END".to_owned());
        target.selection.mode = Some(crate::DelimiterMode::Regex);
        target.selection.include_start = Some(false);
        target.selection.include_end = Some(true);
        target.selection.flags = vec![crate::RegexFlag::CaseInsensitive];

        let plan = build_htmlcut_plan(&target).expect("delimiter plan");
        validate_plan(&plan).expect("valid delimiter plan");
    }

    #[test]
    fn build_htmlcut_plan_covers_first_match_and_output_variants() {
        let mut target = target_with_css();
        target.selection.r#match = crate::SelectionMatch::First;
        target.selection.output = crate::OutputKind::InnerHtml;
        target.selection.whitespace = crate::WhitespaceMode::Preserve;

        let plan = build_htmlcut_plan(&target).expect("first-match plan");
        validate_plan(&plan).expect("valid first-match plan");

        target.selection.output = crate::OutputKind::Text;
        let plan = build_htmlcut_plan(&target).expect("text plan");
        validate_plan(&plan).expect("valid text plan");
    }

    #[test]
    fn build_htmlcut_plan_reports_missing_required_fields() {
        let mut target = target_with_css();
        target.selection.selector = None;
        assert!(build_htmlcut_plan(&target).is_err());

        let mut target = target_with_css();
        target.selection.kind = crate::SelectionKind::DelimiterPair;
        target.selection.selector = None;
        target.selection.start = Some("BEGIN".to_owned());
        target.selection.end = Some("END".to_owned());
        target.selection.mode = Some(crate::DelimiterMode::Literal);
        target.selection.include_start = Some(false);
        target.selection.include_end = Some(false);
        target.selection.r#match = crate::SelectionMatch::Nth;
        target.selection.index = Some(0);
        assert!(build_htmlcut_plan(&target).is_err());

        let mut target = target_with_css();
        target.selection.kind = crate::SelectionKind::DelimiterPair;
        target.selection.selector = None;
        target.selection.end = Some("END".to_owned());
        target.selection.mode = Some(crate::DelimiterMode::Literal);
        target.selection.include_start = Some(false);
        target.selection.include_end = Some(false);
        assert!(build_htmlcut_plan(&target).is_err());

        let mut target = target_with_css();
        target.selection.kind = crate::SelectionKind::DelimiterPair;
        target.selection.selector = None;
        target.selection.start = Some("BEGIN".to_owned());
        target.selection.mode = Some(crate::DelimiterMode::Literal);
        target.selection.include_start = Some(false);
        target.selection.include_end = Some(false);
        assert!(build_htmlcut_plan(&target).is_err());

        let mut target = target_with_css();
        target.selection.kind = crate::SelectionKind::DelimiterPair;
        target.selection.selector = None;
        target.selection.start = Some("BEGIN".to_owned());
        target.selection.end = Some("END".to_owned());
        target.selection.include_start = Some(false);
        target.selection.include_end = Some(false);
        assert!(build_htmlcut_plan(&target).is_err());

        let mut target = target_with_css();
        target.selection.kind = crate::SelectionKind::DelimiterPair;
        target.selection.r#match = crate::SelectionMatch::Nth;
        target.selection.selector = None;
        target.selection.start = Some("BEGIN".to_owned());
        target.selection.end = Some("END".to_owned());
        target.selection.mode = Some(crate::DelimiterMode::Literal);
        target.selection.include_start = Some(false);
        target.selection.include_end = Some(false);
        target.selection.index = None;
        assert!(build_htmlcut_plan(&target).is_err());
    }

    #[test]
    fn mapping_helpers_reflect_htmlcut_result_vocabulary() {
        let plan = build_htmlcut_plan(&target_with_css()).expect("plan");
        let source = HtmlInput::new("demo".to_owned(), "<main>Hello</main>".to_owned())
            .expect("source")
            .with_input_base_url(Url::parse("https://example.com/page").expect("base url"));
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
        target.selection.kind = crate::SelectionKind::DelimiterPair;
        target.selection.r#match = crate::SelectionMatch::Nth;
        target.selection.index = Some(1);
        target.selection.selector = None;
        target.selection.start = Some("BEGIN".to_owned());
        target.selection.end = Some("END".to_owned());
        target.selection.mode = Some(crate::DelimiterMode::Regex);
        target.selection.include_start = Some(true);
        target.selection.include_end = Some(false);
        target.selection.flags = vec![
            crate::RegexFlag::MultiLine,
            crate::RegexFlag::DotMatchesNewLine,
            crate::RegexFlag::SwapGreed,
        ];

        let plan = build_htmlcut_plan(&target).expect("delimiter plan");
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
        target.selection.r#match = crate::SelectionMatch::First;
        let plan = build_htmlcut_plan(&target).expect("first plan");
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
        assert_eq!(map_output_kind(HtmlcutOutputKind::Text), OutputKind::Text);
        assert_eq!(
            map_output_kind(HtmlcutOutputKind::InnerHtml),
            OutputKind::InnerHtml
        );
        assert_eq!(
            map_output_kind(HtmlcutOutputKind::OuterHtml),
            OutputKind::OuterHtml
        );
    }
}

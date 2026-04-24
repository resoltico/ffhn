use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::path::Path;

use htmlcut_core::interop::v1::{
    DelimiterMode as HtmlcutDelimiterMode, ErrorCode, HtmlInput, Normalization, Output,
    OutputKind as HtmlcutOutputKind, Plan, PlanStrategy, Selection, TextWhitespace, execute_plan,
};
use htmlcut_core::{SelectorQuery, SliceBoundary};

use crate::CoreError;

use super::super::schema::{TARGET_SCHEMA_NAME, TARGET_SCHEMA_VERSION};
use super::super::validate::{
    apply_regex_flag, forbid_option, require_non_empty, validate_absolute_file_path,
    validate_absolute_url, validate_identity,
};
use super::super::{
    CanonicalizerKind, DelimiterMode, FetchEngine, SelectionKind, SelectionMatch, TargetKind,
};
use super::defaults::default_fetch_timeout_ms;
use super::types::{
    CanonicalizerSpec, CompareConfig, FetchConfig, NotificationHook, SelectionConfig,
    StorageConfig, TargetDocument, TargetSource,
};

impl TargetDocument {
    /// Validates one target document against the frozen FFHN schema contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_identity(
            &self.schema_name,
            TARGET_SCHEMA_NAME,
            self.schema_version,
            TARGET_SCHEMA_VERSION,
        )?;
        require_non_empty("display_name", &self.display_name)?;
        self.target.validate()?;
        self.fetch.validate_for_source(&self.target)?;
        self.storage.validate()?;
        validate_unique_hook_names(&self.notifications)?;
        for hook in &self.notifications {
            hook.validate()?;
        }

        self.selection.validate()?;
        self.compare.validate()
    }
}

impl TargetSource {
    /// Validates the source discriminator-specific fields.
    pub fn validate(&self) -> Result<(), CoreError> {
        match self.kind {
            TargetKind::Http => {
                validate_absolute_url(self.source_url.as_ref().ok_or_else(|| {
                    CoreError::contract("target.source_url is required for http targets")
                })?)?;
                if self.file_path.is_some() {
                    return Err(CoreError::contract(
                        "target.file_path is only valid for file targets",
                    ));
                }
            }
            TargetKind::File => {
                let file_path = self.file_path.as_deref().ok_or_else(|| {
                    CoreError::contract("target.file_path is required for file targets")
                })?;
                validate_absolute_file_path(file_path)?;
                if self.source_url.is_some() {
                    return Err(CoreError::contract(
                        "target.source_url is only valid for http targets",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl FetchConfig {
    fn validate_for_source(&self, target: &TargetSource) -> Result<(), CoreError> {
        if self.max_bytes < 1_024 || self.max_bytes > 104_857_600 {
            return Err(CoreError::contract(
                "fetch.max_bytes must be in 1024..104857600",
            ));
        }

        match target.kind {
            TargetKind::Http => {
                if self.engine == FetchEngine::File {
                    return Err(CoreError::contract(
                        "fetch.engine = file is only valid for file targets",
                    ));
                }
                if self.timeout_ms < 1_000 || self.timeout_ms > 600_000 {
                    return Err(CoreError::contract(
                        "fetch.timeout_ms must be in 1000..600000",
                    ));
                }
                require_non_empty("fetch.user_agent", &self.user_agent)?;
                require_non_empty("fetch.accept", &self.accept)?;
                for (name, value) in &self.headers {
                    require_non_empty("fetch.headers key", name)?;
                    require_non_empty("fetch.headers value", value)?;
                }
            }
            TargetKind::File => {
                if self.engine != FetchEngine::File {
                    return Err(contract_error("file targets require fetch.engine = file"));
                }
                if self.timeout_ms != default_fetch_timeout_ms() {
                    return Err(contract_error(
                        "file targets require the fixed fetch.timeout_ms default of 15000",
                    ));
                }
                if self.follow_redirects {
                    return Err(contract_error(
                        "file targets must disable fetch.follow_redirects",
                    ));
                }
                if !self.user_agent.is_empty() {
                    return Err(contract_error(
                        "file targets must not define fetch.user_agent",
                    ));
                }
                if !self.accept.is_empty() {
                    return Err(contract_error("file targets must not define fetch.accept"));
                }
                if !self.headers.is_empty() {
                    return Err(contract_error("file targets must not define fetch.headers"));
                }
            }
        }

        Ok(())
    }
}

impl StorageConfig {
    /// Validates one rolling storage policy.
    pub fn validate(&self) -> Result<(), CoreError> {
        if !(1..=256).contains(&self.history_limit) {
            return Err(CoreError::contract(
                "storage.history_limit must be in 1..=256",
            ));
        }
        Ok(())
    }
}

impl NotificationHook {
    /// Validates one notification hook.
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty("notifications.name", &self.name)?;
        require_non_empty("notifications.shell", &self.shell)?;
        require_non_empty("notifications.command", &self.command)?;
        if !Path::new(&self.shell).is_absolute() {
            return Err(CoreError::contract(
                "notifications.shell must be an absolute path",
            ));
        }
        if self.on.is_empty() {
            return Err(contract_error(
                "notifications.on must list at least one event",
            ));
        }
        if self.timeout_ms < 100 || self.timeout_ms > 60_000 {
            return Err(contract_error(
                "notifications.timeout_ms must be in 100..60000",
            ));
        }
        Ok(())
    }
}

impl SelectionConfig {
    /// Validates one target selection section.
    pub fn validate(&self) -> Result<(), CoreError> {
        match self.r#match {
            SelectionMatch::Nth => {
                let index = self.index.ok_or_else(|| {
                    CoreError::contract(
                        "selection.index must be present and positive when match = nth",
                    )
                })?;
                if index == 0 {
                    return Err(CoreError::contract(
                        "selection.index must be present and positive when match = nth",
                    ));
                }
            }
            SelectionMatch::Single | SelectionMatch::First => {
                if self.index.is_some() {
                    return Err(CoreError::contract(
                        "selection.index is only valid when match = nth",
                    ));
                }
            }
        }

        match self.kind {
            SelectionKind::CssSelector => {
                let selector = self
                    .selector
                    .as_deref()
                    .ok_or_else(|| CoreError::contract("selection.selector is required"))?;
                require_non_empty("selection.selector", selector)?;
                forbid_option("selection.start", self.start.as_deref())?;
                forbid_option("selection.end", self.end.as_deref())?;
                forbid_option("selection.mode", self.mode.as_ref())?;
                forbid_option("selection.include_start", self.include_start.as_ref())?;
                forbid_option("selection.include_end", self.include_end.as_ref())?;
                if !self.flags.is_empty() {
                    return Err(CoreError::contract(
                        "selection.flags are only valid for delimiter_pair",
                    ));
                }
            }
            SelectionKind::DelimiterPair => {
                let start = self
                    .start
                    .as_deref()
                    .ok_or_else(|| CoreError::contract("selection.start is required"))?;
                require_non_empty("selection.start", start)?;
                let end = self
                    .end
                    .as_deref()
                    .ok_or_else(|| CoreError::contract("selection.end is required"))?;
                require_non_empty("selection.end", end)?;
                if self.selector.is_some() {
                    return Err(CoreError::contract(
                        "selection.selector is only valid for css_selector",
                    ));
                }
                let mode = self
                    .mode
                    .ok_or_else(|| CoreError::contract("selection.mode is required"))?;
                if self.include_start.is_none() {
                    return Err(CoreError::contract(
                        "selection.include_start and selection.include_end are required",
                    ));
                }
                if self.include_end.is_none() {
                    return Err(CoreError::contract(
                        "selection.include_start and selection.include_end are required",
                    ));
                }
                if mode == DelimiterMode::Literal && !self.flags.is_empty() {
                    return Err(CoreError::contract(
                        "selection.flags are forbidden for literal delimiter mode",
                    ));
                }
            }
        }

        validate_htmlcut_selection_contract(self)
    }
}

impl CompareConfig {
    /// Validates one compare section.
    pub fn validate(&self) -> Result<(), CoreError> {
        for canonicalizer in &self.canonicalization {
            canonicalizer.validate()?;
        }
        Ok(())
    }
}

impl CanonicalizerSpec {
    /// Validates one canonicalizer entry.
    pub fn validate(&self) -> Result<(), CoreError> {
        match self.kind {
            CanonicalizerKind::Trim
            | CanonicalizerKind::CollapseWhitespace
            | CanonicalizerKind::NormalizeNewlines
            | CanonicalizerKind::Lowercase => {
                if self.pattern.is_some() {
                    return Err(CoreError::contract(
                        "canonicalizer pattern/flags are only valid for strip_regex",
                    ));
                }
                if !self.flags.is_empty() {
                    return Err(CoreError::contract(
                        "canonicalizer pattern/flags are only valid for strip_regex",
                    ));
                }
            }
            CanonicalizerKind::StripRegex => {
                let pattern = self.pattern.as_deref().ok_or_else(|| {
                    CoreError::contract("strip_regex canonicalizer requires pattern")
                })?;
                require_non_empty("compare.canonicalization.pattern", pattern)?;
                let mut builder = regex::RegexBuilder::new(pattern);
                builder.unicode(true);
                for flag in &self.flags {
                    apply_regex_flag(flag, &mut builder);
                }
                builder.build().map_err(|error| {
                    CoreError::contract(format!("invalid strip_regex pattern: {error}"))
                })?;
            }
        }
        Ok(())
    }
}

fn validate_unique_hook_names(hooks: &[NotificationHook]) -> Result<(), CoreError> {
    let mut names = BTreeSet::new();
    for hook in hooks {
        if !names.insert(hook.name.as_str()) {
            return Err(CoreError::contract(
                "notifications.name values must be unique",
            ));
        }
    }
    Ok(())
}

fn contract_error(message: &'static str) -> CoreError {
    CoreError::contract(message)
}

fn validate_htmlcut_selection_contract(selection: &SelectionConfig) -> Result<(), CoreError> {
    let strategy = match selection.kind {
        SelectionKind::CssSelector => PlanStrategy::css_selector(
            SelectorQuery::new(
                selection
                    .selector
                    .clone()
                    .expect("SelectionConfig::validate ensures selection.selector exists"),
            )
            .expect("SelectionConfig::validate ensures selection.selector is non-empty"),
        ),
        SelectionKind::DelimiterPair => PlanStrategy::delimiter_pair(
            SliceBoundary::new(
                selection
                    .start
                    .clone()
                    .expect("SelectionConfig::validate ensures selection.start exists"),
            )
            .expect("SelectionConfig::validate ensures selection.start is non-empty"),
            SliceBoundary::new(
                selection
                    .end
                    .clone()
                    .expect("SelectionConfig::validate ensures selection.end exists"),
            )
            .expect("SelectionConfig::validate ensures selection.end is non-empty"),
            match selection
                .mode
                .expect("SelectionConfig::validate ensures selection.mode exists")
            {
                DelimiterMode::Literal => HtmlcutDelimiterMode::Literal,
                DelimiterMode::Regex => HtmlcutDelimiterMode::Regex,
            },
            selection
                .include_start
                .expect("SelectionConfig::validate ensures selection.include_start exists"),
            selection
                .include_end
                .expect("SelectionConfig::validate ensures selection.include_end exists"),
            selection
                .flags
                .iter()
                .copied()
                .map(map_regex_flag)
                .collect(),
        ),
    };

    let selection_mode = match selection.r#match {
        SelectionMatch::Single => Selection::single(),
        SelectionMatch::First => Selection::first(),
        SelectionMatch::Nth => Selection::nth(
            NonZeroUsize::new(
                selection
                    .index
                    .expect("SelectionConfig::validate ensures selection.index exists"),
            )
            .expect("SelectionConfig::validate ensures selection.index is positive"),
        ),
    };

    let output = Output::new(match selection.output {
        crate::OutputKind::Text => HtmlcutOutputKind::Text,
        crate::OutputKind::InnerHtml => HtmlcutOutputKind::InnerHtml,
        crate::OutputKind::OuterHtml => HtmlcutOutputKind::OuterHtml,
    });
    let normalization = Normalization::new(
        match selection.whitespace {
            crate::WhitespaceMode::Preserve => TextWhitespace::Preserve,
            crate::WhitespaceMode::Normalize => TextWhitespace::Normalize,
        },
        selection.rewrite_urls,
    );
    let plan = Plan::new(strategy, selection_mode, output, normalization);
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

fn map_regex_flag(flag: crate::RegexFlag) -> htmlcut_core::interop::v1::RegexFlag {
    match flag {
        crate::RegexFlag::CaseInsensitive => htmlcut_core::interop::v1::RegexFlag::CaseInsensitive,
        crate::RegexFlag::MultiLine => htmlcut_core::interop::v1::RegexFlag::MultiLine,
        crate::RegexFlag::DotMatchesNewLine => {
            htmlcut_core::interop::v1::RegexFlag::DotMatchesNewLine
        }
        crate::RegexFlag::SwapGreed => htmlcut_core::interop::v1::RegexFlag::SwapGreed,
        crate::RegexFlag::IgnoreWhitespace => {
            htmlcut_core::interop::v1::RegexFlag::IgnoreWhitespace
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn css_selection() -> SelectionConfig {
        SelectionConfig {
            kind: SelectionKind::CssSelector,
            r#match: SelectionMatch::Single,
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
        }
    }

    fn delimiter_selection() -> SelectionConfig {
        SelectionConfig {
            kind: SelectionKind::DelimiterPair,
            r#match: SelectionMatch::Nth,
            index: Some(1),
            output: crate::OutputKind::OuterHtml,
            whitespace: crate::WhitespaceMode::Normalize,
            rewrite_urls: false,
            selector: None,
            start: Some("BEGIN".to_owned()),
            end: Some("END".to_owned()),
            mode: Some(DelimiterMode::Regex),
            include_start: Some(false),
            include_end: Some(true),
            flags: vec![crate::RegexFlag::CaseInsensitive],
        }
    }

    #[test]
    fn htmlcut_selection_contract_accepts_supported_selection_variants() {
        validate_htmlcut_selection_contract(&css_selection()).expect("css selection");

        let mut first_text = delimiter_selection();
        first_text.r#match = SelectionMatch::First;
        first_text.index = None;
        first_text.output = crate::OutputKind::Text;
        first_text.whitespace = crate::WhitespaceMode::Preserve;
        first_text.flags = vec![
            crate::RegexFlag::CaseInsensitive,
            crate::RegexFlag::MultiLine,
            crate::RegexFlag::DotMatchesNewLine,
            crate::RegexFlag::SwapGreed,
            crate::RegexFlag::IgnoreWhitespace,
        ];
        validate_htmlcut_selection_contract(&first_text).expect("first text selection");

        let mut nth_inner_html = delimiter_selection();
        nth_inner_html.output = crate::OutputKind::InnerHtml;
        validate_htmlcut_selection_contract(&nth_inner_html).expect("nth inner-html selection");
    }

    #[test]
    fn htmlcut_selection_contract_reports_invalid_css_selectors() {
        let mut invalid = css_selection();
        invalid.selector = Some("main[".to_owned());

        let error =
            validate_htmlcut_selection_contract(&invalid).expect_err("invalid css selector");
        assert!(matches!(error, CoreError::Contract(_)));
    }

    #[test]
    fn htmlcut_selection_regex_flag_mapping_covers_every_variant() {
        assert_eq!(
            map_regex_flag(crate::RegexFlag::CaseInsensitive),
            htmlcut_core::interop::v1::RegexFlag::CaseInsensitive
        );
        assert_eq!(
            map_regex_flag(crate::RegexFlag::MultiLine),
            htmlcut_core::interop::v1::RegexFlag::MultiLine
        );
        assert_eq!(
            map_regex_flag(crate::RegexFlag::DotMatchesNewLine),
            htmlcut_core::interop::v1::RegexFlag::DotMatchesNewLine
        );
        assert_eq!(
            map_regex_flag(crate::RegexFlag::SwapGreed),
            htmlcut_core::interop::v1::RegexFlag::SwapGreed
        );
        assert_eq!(
            map_regex_flag(crate::RegexFlag::IgnoreWhitespace),
            htmlcut_core::interop::v1::RegexFlag::IgnoreWhitespace
        );
    }
}

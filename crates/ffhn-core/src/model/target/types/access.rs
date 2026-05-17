use super::*;
use crate::CoreError;

#[derive(serde::Serialize)]
struct MonitoringContract<'a> {
    target: &'a TargetSource,
    fetch: &'a FetchConfig,
    selection: &'a SelectionConfig,
    compare: &'a CompareConfig,
}

impl TargetSource {
    /// Returns the target-kind discriminator.
    pub const fn kind(&self) -> TargetKind {
        match self {
            Self::Http { .. } => TargetKind::Http,
            Self::File { .. } => TargetKind::File,
        }
    }

    /// Returns the source URL for HTTP targets.
    pub const fn source_url(&self) -> Option<&Url> {
        match self {
            Self::Http { source_url } => Some(source_url),
            Self::File { .. } => None,
        }
    }

    /// Returns the absolute file path for file targets.
    pub fn file_path(&self) -> Option<&str> {
        match self {
            Self::Http { .. } => None,
            Self::File { file_path } => Some(file_path),
        }
    }
}

impl FetchConfig {
    /// Returns the fetch-engine discriminator.
    pub const fn engine(&self) -> FetchEngine {
        match self {
            Self::Http(_) => FetchEngine::Http,
            Self::File(_) => FetchEngine::File,
        }
    }

    /// Returns the maximum accepted byte count.
    pub const fn max_bytes(&self) -> usize {
        match self {
            Self::Http(config) => config.max_bytes,
            Self::File(config) => config.max_bytes,
        }
    }

    /// Returns the HTTP-fetch settings.
    pub const fn http(&self) -> Option<&NetworkFetchConfig> {
        match self {
            Self::Http(config) => Some(config),
            Self::File(_) => None,
        }
    }

    /// Returns the file-fetch settings for file engines.
    pub const fn file(&self) -> Option<&FileFetchConfig> {
        match self {
            Self::Http(_) => None,
            Self::File(config) => Some(config),
        }
    }
}

impl NetworkFetchConfig {
    /// Returns the configured HTTP method.
    pub const fn method(&self) -> HttpMethod {
        self.method
    }

    /// Returns the timeout in milliseconds.
    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Returns the configured User-Agent header.
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Returns whether redirects are followed.
    pub const fn follow_redirects(&self) -> bool {
        self.follow_redirects
    }

    /// Returns the configured Accept header.
    pub fn accept(&self) -> &str {
        &self.accept
    }

    /// Returns any extra request headers.
    pub const fn headers(&self) -> &BTreeMap<String, String> {
        &self.headers
    }

    /// Returns any reserved extensions.
    pub fn extensions(&self) -> Option<&BTreeMap<String, serde_json::Value>> {
        self.extensions.as_ref()
    }
}

impl FileFetchConfig {
    /// Returns any reserved extensions.
    pub fn extensions(&self) -> Option<&BTreeMap<String, serde_json::Value>> {
        self.extensions.as_ref()
    }
}

impl NotificationRoute {
    /// Returns the stable route label.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the run outcomes that trigger this route.
    pub fn on(&self) -> &[RunOutcome] {
        &self.on
    }

    /// Returns the named delivery endpoint for the route.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

impl NotificationEndpoint {
    /// Returns the stable endpoint label.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the configured delivery adapter.
    pub const fn adapter(&self) -> &NotificationAdapter {
        &self.adapter
    }
}

impl NotificationAdapter {
    /// Returns the delivery adapter kind.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::ProcessStdin { .. } => "process_stdin",
        }
    }

    /// Returns the executable path used to deliver the notification.
    pub fn program(&self) -> &str {
        match self {
            Self::ProcessStdin { program, .. } => program,
        }
    }

    /// Returns the exact argument vector passed to the executable.
    pub fn args(&self) -> &[String] {
        match self {
            Self::ProcessStdin { args, .. } => args,
        }
    }

    /// Returns the maximum runtime in milliseconds.
    pub const fn timeout_ms(&self) -> u64 {
        match self {
            Self::ProcessStdin { timeout_ms, .. } => *timeout_ms,
        }
    }
}

impl SelectionModeConfig {
    pub(super) fn from_raw(
        selection_match: SelectionMatch,
        index: Option<usize>,
    ) -> Result<Self, crate::CoreError> {
        match selection_match {
            SelectionMatch::Single => {
                if index.is_some() {
                    return Err(crate::CoreError::contract(
                        "selection.index is only valid when match = nth",
                    ));
                }
                Ok(Self::Single)
            }
            SelectionMatch::First => {
                if index.is_some() {
                    return Err(crate::CoreError::contract(
                        "selection.index is only valid when match = nth",
                    ));
                }
                Ok(Self::First)
            }
            SelectionMatch::Nth => {
                let index = index.ok_or_else(|| {
                    crate::CoreError::contract(
                        "selection.index must be present and positive when match = nth",
                    )
                })?;
                let index = NonZeroUsize::new(index).ok_or_else(|| {
                    crate::CoreError::contract(
                        "selection.index must be present and positive when match = nth",
                    )
                })?;
                Ok(Self::Nth { index })
            }
        }
    }

    pub(super) const fn raw_parts(&self) -> (SelectionMatch, Option<usize>) {
        match self {
            Self::Single => (SelectionMatch::Single, None),
            Self::First => (SelectionMatch::First, None),
            Self::Nth { index } => (SelectionMatch::Nth, Some(index.get())),
        }
    }

    pub(crate) const fn view(&self) -> SelectionModeView {
        match self {
            Self::Single => SelectionModeView::Single,
            Self::First => SelectionModeView::First,
            Self::Nth { index } => SelectionModeView::Nth { index: index.get() },
        }
    }
}

impl SelectionConfig {
    pub(crate) const fn selection_mode(&self) -> &SelectionModeConfig {
        match self {
            Self::CssSelector { selection_mode, .. }
            | Self::DelimiterPair { selection_mode, .. } => selection_mode,
        }
    }
}

impl CanonicalizerSpec {
    /// Returns the canonicalizer kind.
    pub const fn kind(&self) -> CanonicalizerKind {
        self.kind
    }

    /// Returns the regex pattern for `strip_regex`, when one exists.
    pub fn pattern(&self) -> Option<&str> {
        self.pattern.as_deref()
    }

    /// Returns the regex flags for `strip_regex`.
    pub fn flags(&self) -> &[RegexFlag] {
        &self.flags
    }
}

impl TargetDocument {
    /// Returns the frozen schema name.
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    /// Returns the frozen schema version.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the target id declared by the document.
    pub fn target_id(&self) -> &str {
        self.target_id.as_str()
    }

    /// Returns the human-readable target label.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns the coherent typed source view.
    pub fn source(&self) -> TargetSourceView<'_> {
        match &self.target {
            TargetSource::Http { source_url } => {
                TargetSourceView::Http(HttpTargetSourceView(source_url))
            }
            TargetSource::File { file_path } => {
                TargetSourceView::File(FileTargetSourceView(file_path))
            }
        }
    }

    /// Returns whether the target is enabled for live runs.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the target-kind discriminator.
    pub const fn target_kind(&self) -> TargetKind {
        self.target.kind()
    }

    /// Returns the source URL for HTTP targets.
    pub const fn source_url(&self) -> Option<&Url> {
        self.target.source_url()
    }

    /// Returns the absolute file path for file targets.
    pub fn file_path(&self) -> Option<&str> {
        self.target.file_path()
    }

    /// Returns the coherent typed fetch-configuration view.
    pub fn fetch_config(&self) -> FetchConfigView<'_> {
        match &self.fetch {
            FetchConfig::Http(config) => FetchConfigView::Http(HttpFetchConfigView(config)),
            FetchConfig::File(config) => FetchConfigView::File(FileFetchConfigView(config)),
        }
    }

    /// Returns the fetch-engine discriminator.
    pub const fn fetch_engine(&self) -> FetchEngine {
        self.fetch.engine()
    }

    /// Returns the maximum accepted byte count.
    pub const fn fetch_max_bytes(&self) -> usize {
        self.fetch.max_bytes()
    }

    /// Returns the configured HTTP method for network fetches.
    pub fn fetch_http_method(&self) -> Option<HttpMethod> {
        self.fetch.http().map(NetworkFetchConfig::method)
    }

    /// Returns the timeout in milliseconds for network fetches.
    pub fn fetch_timeout_ms(&self) -> Option<u64> {
        self.fetch.http().map(NetworkFetchConfig::timeout_ms)
    }

    /// Returns the configured User-Agent header for network fetches.
    pub fn fetch_user_agent(&self) -> Option<&str> {
        self.fetch.http().map(NetworkFetchConfig::user_agent)
    }

    /// Returns whether redirects are followed for network fetches.
    pub fn fetch_follow_redirects(&self) -> Option<bool> {
        self.fetch.http().map(NetworkFetchConfig::follow_redirects)
    }

    /// Returns the configured Accept header for network fetches.
    pub fn fetch_accept(&self) -> Option<&str> {
        self.fetch.http().map(NetworkFetchConfig::accept)
    }

    /// Returns any extra request headers for network fetches.
    pub fn fetch_headers(&self) -> Option<&BTreeMap<String, String>> {
        self.fetch.http().map(NetworkFetchConfig::headers)
    }

    /// Returns any fetch extensions.
    pub fn fetch_extensions(&self) -> Option<&BTreeMap<String, serde_json::Value>> {
        match &self.fetch {
            FetchConfig::Http(config) => config.extensions(),
            FetchConfig::File(config) => config.extensions(),
        }
    }

    /// Returns the coherent typed selection view.
    pub fn selection(&self) -> SelectionConfigView<'_> {
        match &self.selection {
            SelectionConfig::CssSelector {
                selection_mode,
                selector,
            } => SelectionConfigView::CssSelector(CssSelectorSelectionView {
                selection_mode,
                selector,
            }),
            SelectionConfig::DelimiterPair {
                selection_mode,
                start,
                end,
                mode,
                include_start,
                include_end,
                flags,
            } => SelectionConfigView::DelimiterPair(DelimiterPairSelectionView {
                selection_mode,
                start,
                end,
                mode: *mode,
                include_start: *include_start,
                include_end: *include_end,
                flags,
            }),
        }
    }

    /// Returns the selection-kind discriminator.
    pub const fn selection_kind(&self) -> SelectionKind {
        match self.selection {
            SelectionConfig::CssSelector { .. } => SelectionKind::CssSelector,
            SelectionConfig::DelimiterPair { .. } => SelectionKind::DelimiterPair,
        }
    }

    /// Returns the candidate-selection mode discriminator.
    pub const fn selection_match(&self) -> SelectionMatch {
        self.selection.selection_mode().raw_parts().0
    }

    /// Returns the one-based candidate index when the selection mode is `nth`.
    pub const fn selection_index(&self) -> Option<usize> {
        self.selection.selection_mode().raw_parts().1
    }

    /// Returns the CSS selector when the selection kind is `css_selector`.
    pub fn selection_selector(&self) -> Option<&str> {
        match &self.selection {
            SelectionConfig::CssSelector { selector, .. } => Some(selector),
            SelectionConfig::DelimiterPair { .. } => None,
        }
    }

    /// Returns the start delimiter when the selection kind is `delimiter_pair`.
    pub fn selection_start(&self) -> Option<&str> {
        match &self.selection {
            SelectionConfig::CssSelector { .. } => None,
            SelectionConfig::DelimiterPair { start, .. } => Some(start),
        }
    }

    /// Returns the end delimiter when the selection kind is `delimiter_pair`.
    pub fn selection_end(&self) -> Option<&str> {
        match &self.selection {
            SelectionConfig::CssSelector { .. } => None,
            SelectionConfig::DelimiterPair { end, .. } => Some(end),
        }
    }

    /// Returns the delimiter matching mode when the selection kind is `delimiter_pair`.
    pub const fn selection_delimiter_mode(&self) -> Option<DelimiterMode> {
        match &self.selection {
            SelectionConfig::CssSelector { .. } => None,
            SelectionConfig::DelimiterPair { mode, .. } => Some(*mode),
        }
    }

    /// Returns whether the start delimiter is included when the selection kind is `delimiter_pair`.
    pub const fn selection_include_start(&self) -> Option<bool> {
        match &self.selection {
            SelectionConfig::CssSelector { .. } => None,
            SelectionConfig::DelimiterPair { include_start, .. } => Some(*include_start),
        }
    }

    /// Returns whether the end delimiter is included when the selection kind is `delimiter_pair`.
    pub const fn selection_include_end(&self) -> Option<bool> {
        match &self.selection {
            SelectionConfig::CssSelector { .. } => None,
            SelectionConfig::DelimiterPair { include_end, .. } => Some(*include_end),
        }
    }

    /// Returns the regex flags when the selection kind is `delimiter_pair`.
    pub fn selection_regex_flags(&self) -> &[RegexFlag] {
        match &self.selection {
            SelectionConfig::CssSelector { .. } => &[],
            SelectionConfig::DelimiterPair { flags, .. } => flags,
        }
    }

    /// Returns the compare basis used by this target.
    pub const fn compare_basis(&self) -> CompareBasis {
        self.compare.basis
    }

    /// Returns the text whitespace mode when the compare basis is `text`.
    pub const fn compare_whitespace(&self) -> Option<WhitespaceMode> {
        self.compare.whitespace
    }

    /// Returns whether FFHN rewrites discovered URLs before comparison.
    pub const fn compare_rewrite_urls(&self) -> bool {
        self.compare.rewrite_urls
    }

    /// Returns the ordered canonicalization pipeline.
    pub fn compare_canonicalization(&self) -> &[CanonicalizerSpec] {
        &self.compare.canonicalization
    }

    /// Returns the coherent compare-configuration view.
    pub const fn compare_config(&self) -> CompareConfigView<'_> {
        CompareConfigView(&self.compare)
    }

    /// Returns the total retained successful snapshots, including `snapshots/current`.
    pub const fn storage_history_limit(&self) -> usize {
        self.storage.history_limit
    }

    /// Returns the configured notification routes.
    pub fn notifications(&self) -> impl ExactSizeIterator<Item = NotificationRouteView<'_>> + '_ {
        self.notification_routes
            .iter()
            .map(|route| NotificationRouteView {
                route,
                endpoint: self
                    .notification_endpoints
                    .iter()
                    .find(|endpoint| endpoint.name() == route.endpoint())
                    .expect("validated route endpoint link"),
            })
    }

    /// Returns any reserved extensions.
    pub fn extensions(&self) -> Option<&std::collections::BTreeMap<String, serde_json::Value>> {
        self.extensions.as_ref()
    }

    pub(crate) const fn selection_config(&self) -> &SelectionConfig {
        &self.selection
    }

    pub(crate) const fn compare_config_internal(&self) -> &CompareConfig {
        &self.compare
    }

    pub(crate) fn monitoring_contract_digest_sha256(&self) -> Result<String, CoreError> {
        crate::stable_json::stable_digest(&MonitoringContract {
            target: &self.target,
            fetch: &self.fetch,
            selection: &self.selection,
            compare: &self.compare,
        })
    }

    #[cfg(test)]
    pub(crate) const fn fetch(&self) -> &FetchConfig {
        &self.fetch
    }
}

impl<'a> TargetSourceView<'a> {
    /// Returns the stable target-kind discriminator.
    pub const fn kind(self) -> TargetKind {
        match self {
            Self::Http(_) => TargetKind::Http,
            Self::File(_) => TargetKind::File,
        }
    }
}

impl<'a> HttpTargetSourceView<'a> {
    /// Returns the absolute source URL.
    pub fn source_url(self) -> &'a Url {
        self.0
    }
}

impl<'a> FileTargetSourceView<'a> {
    /// Returns the absolute file path.
    pub fn file_path(self) -> &'a str {
        self.0
    }
}

impl<'a> FetchConfigView<'a> {
    /// Returns the stable fetch-engine discriminator.
    pub const fn engine(self) -> FetchEngine {
        match self {
            Self::Http(_) => FetchEngine::Http,
            Self::File(_) => FetchEngine::File,
        }
    }

    /// Returns the maximum accepted byte count.
    pub const fn max_bytes(self) -> usize {
        match self {
            Self::Http(config) => config.max_bytes(),
            Self::File(config) => config.max_bytes(),
        }
    }
}

impl<'a> HttpFetchConfigView<'a> {
    /// Returns the configured HTTP method.
    pub const fn method(self) -> HttpMethod {
        self.0.method()
    }

    /// Returns the timeout in milliseconds.
    pub const fn timeout_ms(self) -> u64 {
        self.0.timeout_ms()
    }

    /// Returns the maximum accepted byte count.
    pub const fn max_bytes(self) -> usize {
        self.0.max_bytes
    }

    /// Returns the configured User-Agent header.
    pub fn user_agent(self) -> &'a str {
        self.0.user_agent()
    }

    /// Returns whether redirects are followed.
    pub const fn follow_redirects(self) -> bool {
        self.0.follow_redirects()
    }

    /// Returns the configured Accept header.
    pub fn accept(self) -> &'a str {
        self.0.accept()
    }

    /// Returns any extra request headers.
    pub const fn headers(self) -> &'a BTreeMap<String, String> {
        self.0.headers()
    }

    /// Returns any reserved extensions.
    pub fn extensions(self) -> Option<&'a BTreeMap<String, serde_json::Value>> {
        self.0.extensions()
    }
}

impl<'a> FileFetchConfigView<'a> {
    /// Returns the maximum accepted byte count.
    pub const fn max_bytes(self) -> usize {
        self.0.max_bytes
    }

    /// Returns any reserved extensions.
    pub fn extensions(self) -> Option<&'a BTreeMap<String, serde_json::Value>> {
        self.0.extensions()
    }
}

impl SelectionModeView {
    /// Returns the stable selection-match discriminator.
    pub const fn selection_match(self) -> SelectionMatch {
        match self {
            Self::Single => SelectionMatch::Single,
            Self::First => SelectionMatch::First,
            Self::Nth { .. } => SelectionMatch::Nth,
        }
    }

    /// Returns the one-based candidate index when one exists.
    pub const fn index(self) -> Option<usize> {
        match self {
            Self::Single | Self::First => None,
            Self::Nth { index } => Some(index),
        }
    }
}

impl<'a> SelectionConfigView<'a> {
    /// Returns the stable selection-kind discriminator.
    pub const fn kind(self) -> SelectionKind {
        match self {
            Self::CssSelector(_) => SelectionKind::CssSelector,
            Self::DelimiterPair(_) => SelectionKind::DelimiterPair,
        }
    }
}

impl<'a> CssSelectorSelectionView<'a> {
    /// Returns the candidate-selection mode.
    pub const fn selection_mode(self) -> SelectionModeView {
        self.selection_mode.view()
    }

    /// Returns the CSS selector query.
    pub fn selector(self) -> &'a str {
        self.selector
    }
}

impl<'a> DelimiterPairSelectionView<'a> {
    /// Returns the candidate-selection mode.
    pub const fn selection_mode(self) -> SelectionModeView {
        self.selection_mode.view()
    }

    /// Returns the start delimiter.
    pub fn start(self) -> &'a str {
        self.start
    }

    /// Returns the end delimiter.
    pub fn end(self) -> &'a str {
        self.end
    }

    /// Returns the delimiter matching mode.
    pub const fn delimiter_mode(self) -> DelimiterMode {
        self.mode
    }

    /// Returns whether the start delimiter is included in the selected payload.
    pub const fn include_start(self) -> bool {
        self.include_start
    }

    /// Returns whether the end delimiter is included in the selected payload.
    pub const fn include_end(self) -> bool {
        self.include_end
    }

    /// Returns the regex flags used when the delimiter mode is `regex`.
    pub fn regex_flags(self) -> &'a [RegexFlag] {
        self.flags
    }
}

impl<'a> CompareConfigView<'a> {
    /// Returns the compare basis.
    pub const fn basis(self) -> CompareBasis {
        self.0.basis
    }

    /// Returns the text whitespace mode when the compare basis is `text`.
    pub const fn whitespace(self) -> Option<WhitespaceMode> {
        self.0.whitespace
    }

    /// Returns whether FFHN rewrites discovered URLs before comparison.
    pub const fn rewrite_urls(self) -> bool {
        self.0.rewrite_urls
    }

    /// Returns the ordered canonicalization pipeline.
    pub fn canonicalization(self) -> &'a [CanonicalizerSpec] {
        &self.0.canonicalization
    }
}

impl<'a> NotificationRouteView<'a> {
    /// Returns the stable route label.
    pub fn name(self) -> &'a str {
        self.route.name()
    }

    /// Returns the run outcomes that trigger this route.
    pub fn on(self) -> &'a [RunOutcome] {
        self.route.on()
    }

    /// Returns the named delivery endpoint.
    pub fn endpoint(self) -> &'a str {
        self.route.endpoint()
    }

    /// Returns the delivery transport kind.
    pub const fn transport_kind(self) -> &'static str {
        self.endpoint.adapter().kind()
    }

    /// Returns the executable path used to deliver this route.
    pub fn program(self) -> &'a str {
        self.endpoint.adapter().program()
    }

    /// Returns the exact argument vector passed to the executable.
    pub fn args(self) -> &'a [String] {
        self.endpoint.adapter().args()
    }

    /// Returns the maximum runtime in milliseconds.
    pub const fn timeout_ms(self) -> u64 {
        self.endpoint.adapter().timeout_ms()
    }
}

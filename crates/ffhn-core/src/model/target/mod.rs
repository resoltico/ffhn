mod defaults;
mod types;
mod validation;

#[cfg(test)]
pub(crate) use types::NotificationAdapter;
#[cfg(test)]
pub(crate) use types::NotificationEndpoint;
pub use types::{
    CanonicalizerSpec, CompareConfigView, CssSelectorSelectionView, DelimiterPairSelectionView,
    FetchConfigView, FileFetchConfigView, FileTargetSourceView, HttpFetchConfigView,
    HttpTargetSourceView, NotificationRouteView, SelectionConfigView, SelectionModeView,
    TargetDocument, TargetSourceView,
};
#[cfg(any(test, doctest))]
pub(crate) use types::{CompareConfig, FetchConfig, StorageConfig, TargetSource};
pub(crate) use types::{
    FileFetchConfig, NetworkFetchConfig, NotificationRoute, SelectionConfig, SelectionModeConfig,
};

#[cfg(test)]
pub(crate) use super::{
    CanonicalizerKind, CompareBasis, DelimiterMode, HttpMethod, OutputKind, RegexFlag, RunOutcome,
    WhitespaceMode,
};

#[cfg(test)]
mod tests;

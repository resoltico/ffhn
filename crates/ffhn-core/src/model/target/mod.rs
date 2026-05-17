mod defaults;
mod types;
mod validation;

pub(crate) use types::CompareConfig;
#[cfg(test)]
pub(crate) use types::NotificationAdapter;
#[cfg(test)]
pub(crate) use types::NotificationEndpoint;
#[cfg(test)]
pub(crate) use types::StorageConfig;
pub use types::{
    CanonicalizerSpec, CompareConfigView, CssSelectorSelectionView, DelimiterPairSelectionView,
    FetchConfigView, FileFetchConfigView, FileTargetSourceView, HttpFetchConfigView,
    HttpTargetSourceView, NotificationRouteView, SelectionConfigView, SelectionModeView,
    TargetDocument, TargetSourceView,
};
#[cfg(any(test, doctest))]
pub(crate) use types::{FetchConfig, TargetSource};
pub(crate) use types::{
    FileFetchConfig, NetworkFetchConfig, NotificationRoute, SelectionConfig, SelectionModeConfig,
};

#[cfg(test)]
pub(crate) use super::{
    CanonicalizerKind, CompareBasis, DelimiterMode, HttpMethod, RegexFlag, RunOutcome,
    WhitespaceMode,
};

#[cfg(test)]
mod tests;

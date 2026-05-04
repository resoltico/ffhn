mod defaults;
mod types;
mod validation;

pub use types::{CanonicalizerSpec, NotificationHookView, TargetDocument};
#[cfg(any(test, doctest))]
pub(crate) use types::{CompareConfig, FetchConfig, StorageConfig, TargetSource};
pub(crate) use types::{
    FileFetchConfig, NetworkFetchConfig, NotificationHook, SelectionConfig, SelectionModeConfig,
};

#[cfg(test)]
pub(crate) use super::{
    CanonicalizerKind, CompareBasis, DelimiterMode, HttpMethod, OutputKind, RegexFlag, RunOutcome,
    WhitespaceMode,
};

#[cfg(test)]
mod tests;

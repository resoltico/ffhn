mod defaults;
mod types;
mod validation;

pub use types::{
    CanonicalizerSpec, CompareConfig, FetchConfig, NotificationHook, SelectionConfig,
    StorageConfig, TargetDocument, TargetSource,
};

#[cfg(test)]
pub(crate) use super::{
    CanonicalizerKind, CompareBasis, DelimiterMode, FetchEngine, HttpMethod, OutputKind, RegexFlag,
    SelectionKind, SelectionMatch, TargetKind, WhitespaceMode,
};

#[cfg(test)]
mod tests;

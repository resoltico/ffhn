mod htmlcut;
pub(crate) mod parse;
mod record;
mod types;

#[cfg(test)]
mod tests;

pub use htmlcut::{
    HtmlcutByteRange, HtmlcutDiagnostic, HtmlcutDiagnosticCode, HtmlcutDiagnosticDetails,
    HtmlcutDiagnosticLevel, HtmlcutSelectorParse, HtmlcutSelectorParseErrorClass,
    HtmlcutSliceMarkupMatch,
};
pub use record::Observation;
pub(crate) use types::HtmlObservationInput;
pub use types::{AcquisitionKind, PARSER_GRAMMAR_VERSION, PARSER_ID};

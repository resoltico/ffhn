//! Core FFHN observation-graph domain and runtime.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
pub mod graph;
mod model;
mod stable_json;

pub use error::CoreError;
pub use model::{
    AcquisitionKind, Condition, ConditionContext, ConditionEvaluation, ConditionId,
    ConditionOutcome, ConditionPredicate, ConditionReference, ConditionReferenceEvidence,
    DeclaredType, HtmlSelection, HtmlcutByteRange, HtmlcutDiagnostic, HtmlcutDiagnosticCode,
    HtmlcutDiagnosticDetails, HtmlcutDiagnosticLevel, HtmlcutSelectorParse,
    HtmlcutSelectorParseErrorClass, HtmlcutSliceMarkupMatch, NumericLocale, Observation,
    PARSER_GRAMMAR_VERSION, PARSER_ID, Projection, ThresholdDirection, TypeParams,
};

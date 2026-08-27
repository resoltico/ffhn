//! Shared typed-measurement vocabulary and exact policy machinery.

mod measurement;
mod observation;
pub(crate) mod policy;
mod validate;

pub use measurement::{DeclaredType, HtmlSelection, NumericLocale, Projection, TypeParams};
pub(crate) use measurement::{validate_json_pointer, validate_type_params};
pub use observation::{
    AcquisitionKind, HtmlcutByteRange, HtmlcutDiagnostic, HtmlcutDiagnosticCode,
    HtmlcutDiagnosticDetails, HtmlcutDiagnosticLevel, HtmlcutSelectorParse,
    HtmlcutSelectorParseErrorClass, HtmlcutSliceMarkupMatch, Observation, PARSER_GRAMMAR_VERSION,
    PARSER_ID,
};
pub(crate) use observation::{
    HtmlObservationInput,
    parse::{
        JsonAcquisitionFailure, parse_html_projection_for_contract,
        parse_json_scalar_token_for_contract, select_json_scalar_token,
    },
};
pub use policy::{
    Condition, ConditionContext, ConditionEvaluation, ConditionId, ConditionOutcome,
    ConditionPredicate, ConditionReference, ConditionReferenceEvidence, ThresholdDirection,
};
pub(crate) use validate::require_canonical_utc_rfc3339;

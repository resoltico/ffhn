//! Target aggregate split into schema vocabulary, aggregate behavior, and validation.

mod document;
mod schema;
mod validation;

pub(crate) use schema::PermanentTargetError;
pub use schema::{
    DeclaredType, FetchConfig, FetchEngine, HtmlSelection, HttpMethod, NumericLocale, Projection,
    TARGET_SCHEMA_NAME, TARGET_SCHEMA_VERSION, TargetDocument, TargetSource, TypeParams,
};
#[cfg(test)]
pub(crate) use validation::permanent_code_for_htmlcut_failure;
pub(super) use validation::validate_type_params;
#[cfg(test)]
pub(super) use validation::{
    default_follow_redirects, default_max_bytes, default_timeout_ms, require_text,
    validate_json_pointer, validate_max_bytes,
};

use serde::{Deserialize, Serialize};

use super::HtmlcutDiagnostic;

/// Fixed identifier of the typed parser.
pub const PARSER_ID: &str = "ffhn.typed-value";
/// Monotonic grammar version for the typed parser.
pub const PARSER_GRAMMAR_VERSION: u32 = 1;

/// The acquisition identifier persisted with every observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionKind {
    /// An RFC 6901 JSON Pointer selected the scalar.
    JsonPointer,
    /// HTMLCut extracted the selected match's plain DOM descendant text.
    HtmlPlainText,
    /// HTMLCut rendered the selected match with structural text decoration.
    HtmlRenderedText,
    /// HTMLCut exposed one original CSS match-metadata attribute.
    HtmlAttribute,
}

/// HTML projection evidence produced by the FFHN-to-HTMLCut boundary.
pub(crate) struct HtmlObservationInput {
    pub(crate) raw_selected: String,
    pub(crate) comparison_projection: String,
    pub(crate) acquisition_kind: AcquisitionKind,
    pub(crate) plan_digest_sha256: String,
    pub(crate) candidate_count: usize,
    pub(crate) diagnostics: Vec<HtmlcutDiagnostic>,
}

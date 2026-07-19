use htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION;
use serde::{Deserialize, Serialize};

use crate::CoreError;

use super::super::target::validate_type_params;
use super::super::{DeclaredType, TypeParams};
use super::HtmlcutDiagnostic;
use super::parse::{json_input_for_declared_type, parse_canonical_value};
use super::types::{AcquisitionKind, HtmlObservationInput, PARSER_GRAMMAR_VERSION, PARSER_ID};

/// Persisted valid typed observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    raw_selected: String,
    comparison_projection: String,
    acquisition_kind: AcquisitionKind,
    parser_id: String,
    parser_grammar_version: u32,
    declared_type: DeclaredType,
    type_params: TypeParams,
    canonical_value: String,
    parse_diagnostics: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    htmlcut_semantics_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan_digest_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    htmlcut_candidate_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    htmlcut_diagnostics: Vec<HtmlcutDiagnostic>,
}

impl Observation {
    pub(super) fn json(
        raw_selected: String,
        declared_type: DeclaredType,
        type_params: TypeParams,
        canonical_value: String,
    ) -> Self {
        Self {
            comparison_projection: raw_selected.clone(),
            raw_selected,
            acquisition_kind: AcquisitionKind::JsonPointer,
            parser_id: PARSER_ID.to_owned(),
            parser_grammar_version: PARSER_GRAMMAR_VERSION,
            declared_type,
            type_params,
            canonical_value,
            parse_diagnostics: Vec::new(),
            htmlcut_semantics_version: None,
            plan_digest_sha256: None,
            htmlcut_candidate_count: None,
            htmlcut_diagnostics: Vec::new(),
        }
    }

    pub(super) fn html(
        input: HtmlObservationInput,
        declared_type: DeclaredType,
        type_params: TypeParams,
        canonical_value: String,
    ) -> Self {
        Self {
            raw_selected: input.raw_selected,
            comparison_projection: input.comparison_projection,
            acquisition_kind: input.acquisition_kind,
            parser_id: PARSER_ID.to_owned(),
            parser_grammar_version: PARSER_GRAMMAR_VERSION,
            declared_type,
            type_params,
            canonical_value,
            parse_diagnostics: Vec::new(),
            htmlcut_semantics_version: Some(HTMLCUT_EXTRACTION_SEMANTICS_VERSION),
            plan_digest_sha256: Some(input.plan_digest_sha256),
            htmlcut_candidate_count: Some(input.candidate_count),
            htmlcut_diagnostics: input.diagnostics,
        }
    }

    /// Validates every fact persisted for one accepted observation.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.parser_id != PARSER_ID || self.parser_grammar_version != PARSER_GRAMMAR_VERSION {
            return Err(CoreError::contract(
                "accepted observation was produced by an incompatible typed parser",
            ));
        }
        if !self.parse_diagnostics.is_empty() {
            return Err(CoreError::contract(
                "accepted observation must not contain parse_diagnostics",
            ));
        }
        validate_type_params(self.declared_type, &self.type_params)?;
        let parser_input = match self.acquisition_kind {
            AcquisitionKind::JsonPointer => {
                if self.raw_selected != self.comparison_projection {
                    return Err(CoreError::contract(
                        "JSON observation comparison_projection must equal raw_selected",
                    ));
                }
                if self.htmlcut_semantics_version.is_some()
                    || self.plan_digest_sha256.is_some()
                    || self.htmlcut_candidate_count.is_some()
                    || !self.htmlcut_diagnostics.is_empty()
                {
                    return Err(CoreError::contract(
                        "JSON observation must not contain HTMLCut evidence",
                    ));
                }
                json_input_for_declared_type(self.declared_type, &self.raw_selected).map_err(
                    |error| {
                        CoreError::contract(format!(
                            "accepted observation is invalid: {}",
                            error.message()
                        ))
                    },
                )?
            }
            AcquisitionKind::HtmlPlainText
            | AcquisitionKind::HtmlRenderedText
            | AcquisitionKind::HtmlAttribute => {
                if self.htmlcut_semantics_version != Some(HTMLCUT_EXTRACTION_SEMANTICS_VERSION) {
                    return Err(CoreError::contract(
                        "HTML observation was produced by an incompatible HTMLCut extraction semantics version",
                    ));
                }
                if !self
                    .plan_digest_sha256
                    .as_deref()
                    .is_some_and(super::super::state::is_sha256)
                {
                    return Err(CoreError::contract(
                        "HTML observation plan_digest_sha256 must be lowercase SHA-256",
                    ));
                }
                if self.htmlcut_candidate_count.is_none_or(|count| count == 0) {
                    return Err(CoreError::contract(
                        "HTML observation must retain a positive HTMLCut candidate count",
                    ));
                }
                for diagnostic in &self.htmlcut_diagnostics {
                    diagnostic.validate()?;
                }
                self.comparison_projection.clone()
            }
        };
        let canonical_value =
            parse_canonical_value(self.declared_type, &self.type_params, &parser_input).map_err(
                |message| {
                    CoreError::contract(format!("accepted observation is invalid: {message}"))
                },
            )?;
        if self.canonical_value != canonical_value {
            return Err(CoreError::contract(
                "accepted observation canonical_value does not match its typed scalar",
            ));
        }
        Ok(())
    }

    /// Returns the original selected evidence.
    pub fn raw_selected(&self) -> &str {
        &self.raw_selected
    }
    /// Returns the comparison projection.
    pub fn comparison_projection(&self) -> &str {
        &self.comparison_projection
    }
    /// Returns the stable acquisition family that produced this observation.
    pub const fn acquisition_kind(&self) -> AcquisitionKind {
        self.acquisition_kind
    }
    /// Returns the declared-type canonical value.
    pub fn canonical_value(&self) -> &str {
        &self.canonical_value
    }
    /// Returns the HTMLCut extraction-semantics counter for HTML observations.
    pub const fn htmlcut_semantics_version(&self) -> Option<u32> {
        self.htmlcut_semantics_version
    }
    /// Returns the exact internally structured HTMLCut plan digest for HTML observations.
    pub fn plan_digest_sha256(&self) -> Option<&str> {
        self.plan_digest_sha256.as_deref()
    }
    /// Returns the pre-selection HTML candidate count for HTML observations.
    pub const fn htmlcut_candidate_count(&self) -> Option<usize> {
        self.htmlcut_candidate_count
    }
    /// Returns warning and informational HTMLCut diagnostics retained with this observation.
    pub fn htmlcut_diagnostics(&self) -> &[HtmlcutDiagnostic] {
        &self.htmlcut_diagnostics
    }

    pub(in crate::model) const fn declared_type_for_policy(&self) -> DeclaredType {
        self.declared_type
    }

    pub(in crate::model) const fn type_params_for_policy(&self) -> &TypeParams {
        &self.type_params
    }
}

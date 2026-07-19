//! HTMLCut evidence carried through FFHN's closed diagnostic boundary.

use htmlcut_core::interop::v1::{ErrorCode, InteropError};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{
    CoreError, HtmlcutDiagnostic, HtmlcutDiagnosticCode, HtmlcutDiagnosticDetails,
    HtmlcutSelectorParse,
};

/// HTMLCut failure evidence retained when an HTML projection cannot yield a measurement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HtmlcutFailureDetails {
    error_class: HtmlcutErrorClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    core_diagnostic_code: Option<HtmlcutDiagnosticCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    candidate_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    boundary_evidence: Option<HtmlcutBoundaryEvidence>,
    plan_digest_sha256: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<HtmlcutDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selector_parse: Option<HtmlcutSelectorParse>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HtmlcutFailureDetailsWire {
    error_class: HtmlcutErrorClass,
    #[serde(default)]
    core_diagnostic_code: Option<HtmlcutDiagnosticCode>,
    #[serde(default)]
    candidate_count: Option<usize>,
    #[serde(default)]
    boundary_evidence: Option<HtmlcutBoundaryEvidence>,
    plan_digest_sha256: String,
    #[serde(default)]
    diagnostics: Vec<HtmlcutDiagnostic>,
    #[serde(default)]
    selector_parse: Option<HtmlcutSelectorParse>,
}

impl<'de> Deserialize<'de> for HtmlcutFailureDetails {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = HtmlcutFailureDetailsWire::deserialize(deserializer)?;
        let failure = Self {
            error_class: wire.error_class,
            core_diagnostic_code: wire.core_diagnostic_code,
            candidate_count: wire.candidate_count,
            boundary_evidence: wire.boundary_evidence,
            plan_digest_sha256: wire.plan_digest_sha256,
            diagnostics: wire.diagnostics,
            selector_parse: wire.selector_parse,
        };
        failure.validate().map_err(serde::de::Error::custom)?;
        Ok(failure)
    }
}

/// Closed top-level class of an HTMLCut failure or an FFHN-observed HTMLCut boundary violation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HtmlcutErrorClass {
    /// HTMLCut rejected a plan before extraction.
    PlanInvalid,
    /// HTMLCut found no candidate for the configured selection.
    NoMatch,
    /// HTMLCut found multiple candidates for an exact-one selection.
    AmbiguousMatch,
    /// HTMLCut could not find a required selected attribute.
    MissingAttribute,
    /// HTMLCut reported an internal integration failure.
    InternalError,
    /// FFHN observed that a successful HTMLCut result violated its adapter postcondition.
    FfhnBoundaryInvariantViolation,
}

impl HtmlcutErrorClass {
    /// Returns the stable report-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlanInvalid => "plan_invalid",
            Self::NoMatch => "no_match",
            Self::AmbiguousMatch => "ambiguous_match",
            Self::MissingAttribute => "missing_attribute",
            Self::InternalError => "internal_error",
            Self::FfhnBoundaryInvariantViolation => "ffhn_boundary_invariant_violation",
        }
    }

    fn from_interop(error_code: ErrorCode) -> Self {
        match error_code {
            ErrorCode::PlanInvalid => Self::PlanInvalid,
            ErrorCode::NoMatch => Self::NoMatch,
            ErrorCode::AmbiguousMatch => Self::AmbiguousMatch,
            ErrorCode::MissingAttribute => Self::MissingAttribute,
            ErrorCode::InternalError => Self::InternalError,
        }
    }
}

/// Typed FFHN evidence for a violated HTMLCut result postcondition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HtmlcutBoundaryEvidence {
    /// HTMLCut returned a selected-match cardinality incompatible with FFHN's exact-one request.
    SelectedMatchCount {
        /// The number of selected matches returned by HTMLCut.
        selected_match_count: usize,
    },
    /// HTMLCut's successful CSS selection omitted an attribute required by the projection.
    RequestedCssAttribute {
        /// The target-configured attribute FFHN required from the selected CSS match.
        attribute: String,
    },
}

impl HtmlcutFailureDetails {
    /// Builds retained HTMLCut evidence from the validated upstream boundary.
    pub(crate) fn new(
        error_class: HtmlcutErrorClass,
        candidate_count: Option<usize>,
        plan_digest_sha256: String,
        diagnostics: Vec<HtmlcutDiagnostic>,
    ) -> Self {
        Self {
            error_class,
            core_diagnostic_code: None,
            candidate_count,
            boundary_evidence: None,
            plan_digest_sha256,
            diagnostics,
            selector_parse: None,
        }
    }

    /// Attaches HTMLCut's closed primary diagnostic code when the public error supplied one.
    #[cfg(test)]
    pub(crate) fn with_core_diagnostic_code(
        mut self,
        core_diagnostic_code: HtmlcutDiagnosticCode,
    ) -> Self {
        self.core_diagnostic_code = Some(core_diagnostic_code);
        self
    }

    /// Attaches one typed fact from an FFHN-observed HTMLCut postcondition violation.
    pub(crate) fn with_boundary_evidence(
        mut self,
        boundary_evidence: HtmlcutBoundaryEvidence,
    ) -> Self {
        self.boundary_evidence = Some(boundary_evidence);
        self
    }

    /// Attaches optional FFHN-observed postcondition evidence without creating a second carrier.
    pub(crate) fn with_optional_boundary_evidence(
        self,
        boundary_evidence: Option<HtmlcutBoundaryEvidence>,
    ) -> Self {
        match boundary_evidence {
            Some(boundary_evidence) => self.with_boundary_evidence(boundary_evidence),
            None => self,
        }
    }

    /// Converts one public HTMLCut failure into stable FFHN report evidence.
    pub(crate) fn from_interop_error(error: &InteropError) -> Result<Self, CoreError> {
        let error_class = HtmlcutErrorClass::from_interop(error.error_code);
        let core_diagnostic_code = core_diagnostic_code_from_error(error)?;
        let core_details = error.details.get("core_details");
        let candidate_count = core_details.and_then(candidate_count_in_value);
        let diagnostics = error
            .diagnostics
            .clone()
            .into_iter()
            .map(HtmlcutDiagnostic::from_interop)
            .collect::<Result<Vec<_>, _>>()?;
        let selector_parse = selector_parse_from_error(&diagnostics, core_details)?;
        let failure = Self {
            error_class,
            core_diagnostic_code,
            candidate_count,
            boundary_evidence: None,
            plan_digest_sha256: error.plan_digest_sha256.clone(),
            diagnostics,
            selector_parse,
        };
        failure.validate()?;
        Ok(failure)
    }

    /// Returns the closed top-level HTMLCut error class.
    pub const fn error_class(&self) -> HtmlcutErrorClass {
        self.error_class
    }

    /// Returns HTMLCut's closed primary diagnostic code when its public error supplied one.
    pub const fn core_diagnostic_code(&self) -> Option<HtmlcutDiagnosticCode> {
        self.core_diagnostic_code
    }

    /// Returns HTMLCut's pre-selection candidate count when supplied.
    pub const fn candidate_count(&self) -> Option<usize> {
        self.candidate_count
    }

    /// Returns FFHN's typed postcondition evidence when HTMLCut returned an impossible success shape.
    pub const fn boundary_evidence(&self) -> Option<&HtmlcutBoundaryEvidence> {
        self.boundary_evidence.as_ref()
    }

    /// Returns the structured HTMLCut plan digest.
    pub fn plan_digest_sha256(&self) -> &str {
        &self.plan_digest_sha256
    }

    /// Returns all HTMLCut diagnostics retained for the failure.
    pub fn diagnostics(&self) -> &[HtmlcutDiagnostic] {
        &self.diagnostics
    }

    /// Returns the closed selector-parse evidence for an invalid-selector failure.
    pub const fn selector_parse(&self) -> Option<&HtmlcutSelectorParse> {
        self.selector_parse.as_ref()
    }

    /// Revalidates bounded upstream evidence before FFHN accepts it from a serialized carrier.
    pub(super) fn validate(&self) -> Result<(), CoreError> {
        if !is_sha256(&self.plan_digest_sha256) {
            return Err(CoreError::contract(
                "HTMLCut failure plan digest must be lowercase SHA-256",
            ));
        }
        for diagnostic in &self.diagnostics {
            diagnostic.validate()?;
        }
        if !core_diagnostic_code_matches_error_class(self.error_class, self.core_diagnostic_code) {
            return Err(CoreError::contract(
                "HTMLCut core diagnostic code does not match its error class",
            ));
        }
        if let Some(boundary_evidence) = &self.boundary_evidence {
            boundary_evidence.validate_for(self.error_class)?;
        }
        let invalid_selector =
            self.core_diagnostic_code == Some(HtmlcutDiagnosticCode::InvalidSelector);
        if invalid_selector != self.selector_parse.is_some() {
            return Err(CoreError::contract(
                "HTMLCut invalid-selector failures require exactly one selector_parse evidence value",
            ));
        }
        if let Some(selector_parse) = &self.selector_parse {
            let matching_diagnostic = self.diagnostics.iter().find_map(|diagnostic| {
                (diagnostic.code() == HtmlcutDiagnosticCode::InvalidSelector.as_str())
                    .then_some(diagnostic.details())
                    .flatten()
                    .and_then(HtmlcutDiagnosticDetails::selector_parse)
            });
            if matching_diagnostic != Some(selector_parse) {
                return Err(CoreError::contract(
                    "HTMLCut failure selector_parse must equal its invalid-selector diagnostic evidence",
                ));
            }
        }
        Ok(())
    }
}

impl HtmlcutBoundaryEvidence {
    fn validate_for(&self, error_class: HtmlcutErrorClass) -> Result<(), CoreError> {
        match self {
            Self::SelectedMatchCount {
                selected_match_count,
            } if error_class == HtmlcutErrorClass::FfhnBoundaryInvariantViolation
                && *selected_match_count != 1 =>
            {
                Ok(())
            }
            Self::RequestedCssAttribute { attribute }
                if error_class == HtmlcutErrorClass::MissingAttribute
                    && !attribute.is_empty()
                    && attribute.len() <= 1_024 =>
            {
                Ok(())
            }
            _ => Err(CoreError::contract(
                "HTMLCut boundary evidence does not match its error class",
            )),
        }
    }
}

fn core_diagnostic_code_from_error(
    error: &InteropError,
) -> Result<Option<HtmlcutDiagnosticCode>, CoreError> {
    let Some(value) = error.details.get("core_diagnostic_code") else {
        return Ok(None);
    };
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|_| {
            CoreError::contract(
                "HTMLCut core_diagnostic_code is not a known pinned diagnostic code",
            )
        })
}

fn core_diagnostic_code_matches_error_class(
    error_class: HtmlcutErrorClass,
    core_diagnostic_code: Option<HtmlcutDiagnosticCode>,
) -> bool {
    let Some(core_diagnostic_code) = core_diagnostic_code else {
        return true;
    };
    matches!(
        (error_class, core_diagnostic_code),
        (
            HtmlcutErrorClass::PlanInvalid,
            HtmlcutDiagnosticCode::UnsupportedSpecVersion
                | HtmlcutDiagnosticCode::InvalidSelector
                | HtmlcutDiagnosticCode::InvalidSlicePattern
        ) | (
            HtmlcutErrorClass::NoMatch,
            HtmlcutDiagnosticCode::NoMatch | HtmlcutDiagnosticCode::MatchIndexOutOfRange
        ) | (
            HtmlcutErrorClass::AmbiguousMatch,
            HtmlcutDiagnosticCode::AmbiguousMatch
        ) | (
            HtmlcutErrorClass::MissingAttribute,
            HtmlcutDiagnosticCode::MissingAttribute
        ) | (
            HtmlcutErrorClass::InternalError,
            HtmlcutDiagnosticCode::SourceLoadFailed
                | HtmlcutDiagnosticCode::UnsupportedValueType
                | HtmlcutDiagnosticCode::MultipleMatches
                | HtmlcutDiagnosticCode::EffectiveBaseUrlUnresolved
                | HtmlcutDiagnosticCode::SliceSplitsMarkup
        )
    )
}

fn candidate_count_in_value(value: &Value) -> Option<usize> {
    value
        .as_object()
        .and_then(|values| values.get("candidateCount"))
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
}

fn selector_parse_from_error(
    diagnostics: &[HtmlcutDiagnostic],
    core_details: Option<&Value>,
) -> Result<Option<HtmlcutSelectorParse>, CoreError> {
    let diagnostic_selector_parse = diagnostics.iter().find_map(|diagnostic| {
        (diagnostic.code() == HtmlcutDiagnosticCode::InvalidSelector.as_str())
            .then_some(diagnostic.details())
            .flatten()
            .and_then(HtmlcutDiagnosticDetails::selector_parse)
            .cloned()
    });
    let core_selector_parse = core_details
        .and_then(Value::as_object)
        .and_then(|details| details.get("selector_parse"))
        .map(selector_parse_from_value)
        .transpose()?;
    if diagnostic_selector_parse.is_some() != core_selector_parse.is_some()
        || diagnostic_selector_parse != core_selector_parse
    {
        return Err(CoreError::contract(
            "HTMLCut invalid-selector carriers must retain one matching selector_parse value",
        ));
    }
    Ok(diagnostic_selector_parse)
}

fn selector_parse_from_value(value: &Value) -> Result<HtmlcutSelectorParse, CoreError> {
    let details = value
        .as_object()
        .filter(|details| {
            details.len() == 3
                && ["line", "column_utf16", "parse_error_class"]
                    .iter()
                    .all(|key| details.contains_key(*key))
        })
        .ok_or_else(|| CoreError::contract("HTMLCut selector_parse evidence is malformed"))?;
    serde_json::from_value(Value::Object(details.clone()))
        .map_err(|_| CoreError::contract("HTMLCut selector_parse evidence is malformed"))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::HtmlcutDiagnosticCode;

    use super::{HtmlcutErrorClass, HtmlcutFailureDetails, candidate_count_in_value};

    #[test]
    fn candidate_count_search_accepts_both_upstream_spellings_and_skips_unrepresentable_values() {
        assert_eq!(
            candidate_count_in_value(&json!({ "candidateCount": 3 })),
            Some(3)
        );
        assert_eq!(
            candidate_count_in_value(&json!({ "outer": [{ "candidate_count": 7 }] })),
            None
        );
        assert_eq!(
            candidate_count_in_value(&json!({ "candidateCount": u64::MAX })),
            usize::try_from(u64::MAX).ok()
        );
        assert_eq!(
            candidate_count_in_value(&json!(["not a count", null])),
            None
        );
    }

    #[test]
    fn htmlcut_error_classes_have_one_stable_report_spelling_each() {
        let classes = [
            (HtmlcutErrorClass::PlanInvalid, "plan_invalid"),
            (HtmlcutErrorClass::NoMatch, "no_match"),
            (HtmlcutErrorClass::AmbiguousMatch, "ambiguous_match"),
            (HtmlcutErrorClass::MissingAttribute, "missing_attribute"),
            (HtmlcutErrorClass::InternalError, "internal_error"),
            (
                HtmlcutErrorClass::FfhnBoundaryInvariantViolation,
                "ffhn_boundary_invariant_violation",
            ),
        ];

        for (class, spelling) in classes {
            assert_eq!(class.as_str(), spelling);
        }
    }

    #[test]
    fn direct_htmlcut_failure_deserialization_rejects_crossed_class_and_primary_code() {
        let failure = HtmlcutFailureDetails::new(
            HtmlcutErrorClass::NoMatch,
            None,
            "a".repeat(64),
            Vec::new(),
        )
        .with_core_diagnostic_code(HtmlcutDiagnosticCode::NoMatch);
        let mut wire = serde_json::to_value(failure).expect("HTMLCut failure JSON");
        wire["core_diagnostic_code"] = json!("AMBIGUOUS_MATCH");

        assert!(serde_json::from_value::<HtmlcutFailureDetails>(wire).is_err());
    }
}

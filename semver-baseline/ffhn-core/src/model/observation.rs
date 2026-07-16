use std::collections::BTreeMap;
use std::str::FromStr;

use htmlcut_core::interop::v1::{
    HTMLCUT_EXTRACTION_SEMANTICS_VERSION, InteropDiagnostic, InteropDiagnosticLevel,
};
use rust_decimal::Decimal;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset, format_description};

use crate::CoreError;

use super::failure::SourceSuspectReason;
use super::target::validate_type_params;
use super::{
    DeclaredType, NumericLocale, ProcessErrorDetail, ProcessErrorKind, TargetDocument, TypeParams,
};

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
    /// HTMLCut rendered the selected match as text.
    HtmlText,
    /// HTMLCut exposed one original CSS match-metadata attribute.
    HtmlAttribute,
}

/// One HTMLCut diagnostic retained as FFHN measurement evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HtmlcutDiagnostic {
    level: String,
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

impl HtmlcutDiagnostic {
    pub(crate) fn from_interop(diagnostic: InteropDiagnostic) -> Self {
        let level = match diagnostic.level {
            InteropDiagnosticLevel::Error => "error",
            InteropDiagnosticLevel::Warning => "warning",
            InteropDiagnosticLevel::Info => "info",
        };
        Self {
            level: level.to_owned(),
            code: diagnostic.code.as_str().to_owned(),
            message: diagnostic.message,
            details: diagnostic.details,
        }
    }

    /// Returns HTMLCut's stable diagnostic severity.
    pub fn level(&self) -> &str {
        &self.level
    }

    /// Returns HTMLCut's stable diagnostic code.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the HTMLCut diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the optional structured HTMLCut diagnostic details.
    pub fn details(&self) -> Option<&serde_json::Value> {
        self.details.as_ref()
    }
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

/// A structured M3 JSON-acquisition failure before it becomes a persisted health fact.
#[derive(Debug)]
pub(crate) enum JsonAcquisitionFailure {
    Malformed(ProcessErrorDetail),
    MissingPointerTarget(ProcessErrorDetail),
    NonScalarPointerTarget(ProcessErrorDetail),
}

impl JsonAcquisitionFailure {
    pub(crate) const fn reason(&self) -> SourceSuspectReason {
        match self {
            Self::Malformed(_) => SourceSuspectReason::JsonMalformed,
            Self::MissingPointerTarget(_) => SourceSuspectReason::JsonMissingPointerTarget,
            Self::NonScalarPointerTarget(_) => SourceSuspectReason::JsonNonScalarPointerTarget,
        }
    }

    pub(crate) fn into_detail(self) -> ProcessErrorDetail {
        match self {
            Self::Malformed(detail)
            | Self::MissingPointerTarget(detail)
            | Self::NonScalarPointerTarget(detail) => detail,
        }
    }

    fn malformed(message: impl Into<String>) -> Self {
        Self::Malformed(ProcessErrorDetail::new(
            ProcessErrorKind::Json,
            message,
            None,
        ))
    }

    fn missing_pointer_target() -> Self {
        Self::MissingPointerTarget(ProcessErrorDetail::new(
            ProcessErrorKind::Json,
            "projection.pointer did not select a JSON value",
            None,
        ))
    }

    fn non_scalar_pointer_target() -> Self {
        Self::NonScalarPointerTarget(ProcessErrorDetail::new(
            ProcessErrorKind::Json,
            "projection.pointer must select a scalar JSON leaf",
            None,
        ))
    }
}

#[derive(Debug)]
pub(super) enum JsonScalarError {
    Invalid(String),
    NonScalar,
}

impl JsonScalarError {
    fn message(&self) -> &str {
        match self {
            Self::Invalid(message) => message,
            Self::NonScalar => "projection.pointer must select a scalar JSON leaf",
        }
    }
}

pub(super) fn parse_json_scalar_token(
    target: &TargetDocument,
    raw_selected: String,
) -> Result<Observation, ProcessErrorDetail> {
    let parser_input = json_scalar_value(&raw_selected).map_err(|error| {
        ProcessErrorDetail::new(ProcessErrorKind::ValueUnparseable, error.message(), None)
    })?;
    let canonical_value =
        parse_canonical_value(target.declared_type(), target.type_params(), &parser_input)
            .map_err(|message| {
                ProcessErrorDetail::new(ProcessErrorKind::ValueUnparseable, message, None)
            })?;
    Ok(Observation {
        comparison_projection: raw_selected.clone(),
        raw_selected,
        acquisition_kind: AcquisitionKind::JsonPointer,
        parser_id: PARSER_ID.to_owned(),
        parser_grammar_version: PARSER_GRAMMAR_VERSION,
        declared_type: target.declared_type(),
        type_params: target.type_params().clone(),
        canonical_value,
        parse_diagnostics: Vec::new(),
        htmlcut_semantics_version: None,
        plan_digest_sha256: None,
        htmlcut_candidate_count: None,
        htmlcut_diagnostics: Vec::new(),
    })
}

pub(super) fn parse_html_projection(
    target: &TargetDocument,
    input: HtmlObservationInput,
) -> Result<Observation, ProcessErrorDetail> {
    let canonical_value = parse_canonical_value(
        target.declared_type(),
        target.type_params(),
        &input.comparison_projection,
    )
    .map_err(|message| {
        ProcessErrorDetail::new(ProcessErrorKind::ValueUnparseable, message, None)
    })?;
    Ok(Observation {
        raw_selected: input.raw_selected,
        comparison_projection: input.comparison_projection,
        acquisition_kind: input.acquisition_kind,
        parser_id: PARSER_ID.to_owned(),
        parser_grammar_version: PARSER_GRAMMAR_VERSION,
        declared_type: target.declared_type(),
        type_params: target.type_params().clone(),
        canonical_value,
        parse_diagnostics: Vec::new(),
        htmlcut_semantics_version: Some(HTMLCUT_EXTRACTION_SEMANTICS_VERSION),
        plan_digest_sha256: Some(input.plan_digest_sha256),
        htmlcut_candidate_count: Some(input.candidate_count),
        htmlcut_diagnostics: input.diagnostics,
    })
}

impl Observation {
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
                json_scalar_value(&self.raw_selected).map_err(|error| {
                    CoreError::contract(format!(
                        "accepted observation is invalid: {}",
                        error.message()
                    ))
                })?
            }
            AcquisitionKind::HtmlText | AcquisitionKind::HtmlAttribute => {
                if self.htmlcut_semantics_version != Some(HTMLCUT_EXTRACTION_SEMANTICS_VERSION) {
                    return Err(CoreError::contract(
                        "HTML observation was produced by an incompatible HTMLCut extraction semantics version",
                    ));
                }
                if !self
                    .plan_digest_sha256
                    .as_deref()
                    .is_some_and(super::state::is_sha256)
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
    /// Returns the normalized typed value.
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

    pub(super) const fn declared_type_for_policy(&self) -> DeclaredType {
        self.declared_type
    }

    pub(super) const fn type_params_for_policy(&self) -> &TypeParams {
        &self.type_params
    }
}

/// Selects one RFC 6901 JSON Pointer scalar while preserving its exact JSON token.
pub(crate) fn select_json_scalar_token(
    body: &str,
    pointer: &str,
) -> Result<String, JsonAcquisitionFailure> {
    let root: Box<RawValue> = serde_json::from_str(body).map_err(|error| {
        JsonAcquisitionFailure::malformed(format!("source is not valid JSON: {error}"))
    })?;
    let mut selected =
        normalized_raw_token(root.get()).map_err(JsonAcquisitionFailure::malformed)?;
    for encoded_token in pointer.split('/').skip(1) {
        let token =
            decode_json_pointer_token(encoded_token).map_err(JsonAcquisitionFailure::malformed)?;
        selected = select_json_pointer_child(&selected, &token)?;
    }
    validated_scalar_selection(selected)
}

fn validated_scalar_selection(selected: String) -> Result<String, JsonAcquisitionFailure> {
    match json_scalar_value(&selected) {
        Ok(_) => Ok(selected),
        Err(JsonScalarError::Invalid(message)) => Err(JsonAcquisitionFailure::malformed(message)),
        Err(JsonScalarError::NonScalar) => Err(JsonAcquisitionFailure::non_scalar_pointer_target()),
    }
}

pub(super) fn parse_canonical_value(
    kind: DeclaredType,
    params: &TypeParams,
    raw: &str,
) -> Result<String, String> {
    match kind {
        DeclaredType::Integer => raw
            .parse::<i128>()
            .map(|value| value.to_string())
            .map_err(|error| format!("integer value is not valid i128: {error}")),
        DeclaredType::Decimal | DeclaredType::Money => {
            let normalized = normalize_decimal_input(raw, params.locale)?;
            Decimal::from_str(&normalized)
                .map(|value| value.normalize().to_string())
                .map_err(|error| format!("decimal value is out of range or invalid: {error}"))
        }
        DeclaredType::Semver => Version::parse(raw)
            .map(|value| value.to_string())
            .map_err(|error| format!("semver value is invalid: {error}")),
        DeclaredType::Datetime => parse_datetime(raw, params),
    }
}

pub(super) fn normalized_raw_token(raw: &str) -> Result<String, String> {
    let token = raw.trim();
    if token.is_empty() {
        return Err("JSON scalar token is empty".to_owned());
    }
    serde_json::from_str::<Box<RawValue>>(token)
        .map_err(|error| format!("source is not valid JSON: {error}"))?;
    Ok(token.to_owned())
}

pub(super) fn select_json_pointer_child(
    raw_parent: &str,
    token: &str,
) -> Result<String, JsonAcquisitionFailure> {
    let child = match raw_parent.as_bytes().first() {
        Some(b'{') => {
            let entries: BTreeMap<String, Box<RawValue>> = serde_json::from_str(raw_parent)
                .map_err(|error| {
                    JsonAcquisitionFailure::malformed(format!("source is not valid JSON: {error}"))
                })?;
            entries.get(token).map(|value| value.get().to_owned())
        }
        Some(b'[') => {
            let index = parse_json_array_index(token);
            let entries: Vec<Box<RawValue>> =
                serde_json::from_str(raw_parent).map_err(|error| {
                    JsonAcquisitionFailure::malformed(format!("source is not valid JSON: {error}"))
                })?;
            index.and_then(|index| entries.get(index).map(|value| value.get().to_owned()))
        }
        _ => None,
    }
    .ok_or_else(JsonAcquisitionFailure::missing_pointer_target)?;
    normalized_raw_token(&child).map_err(JsonAcquisitionFailure::malformed)
}

pub(super) fn decode_json_pointer_token(encoded: &str) -> Result<String, String> {
    let mut decoded = String::with_capacity(encoded.len());
    let mut characters = encoded.chars();
    while let Some(character) = characters.next() {
        if character != '~' {
            decoded.push(character);
            continue;
        }
        match characters.next() {
            Some('0') => decoded.push('~'),
            Some('1') => decoded.push('/'),
            _ => return Err("projection.pointer must use RFC 6901 escapes".to_owned()),
        }
    }
    Ok(decoded)
}

pub(super) fn parse_json_array_index(token: &str) -> Option<usize> {
    if token == "0" {
        Some(0)
    } else if token.starts_with('0') {
        None
    } else {
        token.parse::<usize>().ok()
    }
}

pub(super) fn json_scalar_value(raw_token: &str) -> Result<String, JsonScalarError> {
    if raw_token != raw_token.trim() {
        return Err(JsonScalarError::Invalid(
            "JSON scalar token must not contain outer whitespace".to_owned(),
        ));
    }
    let value: serde_json::Value = serde_json::from_str(raw_token).map_err(|error| {
        JsonScalarError::Invalid(format!("JSON scalar token is invalid: {error}"))
    })?;
    match value {
        serde_json::Value::String(value) => Ok(value),
        serde_json::Value::Number(_) | serde_json::Value::Bool(_) | serde_json::Value::Null => {
            Ok(raw_token.to_owned())
        }
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err(JsonScalarError::NonScalar)
        }
    }
}

pub(super) fn normalize_decimal_input(
    raw: &str,
    locale: Option<NumericLocale>,
) -> Result<String, String> {
    match locale.unwrap_or(NumericLocale::Invariant) {
        NumericLocale::Invariant if raw.contains([',', ' ']) => {
            Err("invariant decimal values cannot use grouping or spaces".to_owned())
        }
        NumericLocale::Invariant => Ok(raw.to_owned()),
        NumericLocale::EnUs => normalize_grouped_decimal(raw, ',', '.'),
        NumericLocale::DeDe => normalize_grouped_decimal(raw, '.', ','),
    }
}

pub(super) fn normalize_grouped_decimal(
    raw: &str,
    group: char,
    decimal: char,
) -> Result<String, String> {
    let (sign, value) = raw.strip_prefix('-').map_or(("", raw), |rest| ("-", rest));
    let mut pieces = value.split(decimal);
    let whole = pieces.next().unwrap_or_default();
    let fraction = pieces.next();
    if pieces.next().is_some() || whole.is_empty() || fraction.is_some_and(|part| part.is_empty()) {
        return Err("decimal presentation is invalid for configured locale".to_owned());
    }
    let groups = whole.split(group).collect::<Vec<_>>();
    if groups
        .iter()
        .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
        || (groups.len() > 1
            && (groups[0].len() > 3 || groups.iter().skip(1).any(|part| part.len() != 3)))
    {
        return Err("decimal grouping is invalid for configured locale".to_owned());
    }
    let mut normalized = format!("{sign}{}", groups.concat());
    if let Some(fraction) = fraction {
        if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("decimal fraction is invalid for configured locale".to_owned());
        }
        normalized.push('.');
        normalized.push_str(fraction);
    }
    Ok(normalized)
}

pub(super) fn parse_datetime(raw: &str, params: &TypeParams) -> Result<String, String> {
    let format = params.format.as_deref().expect("validated datetime params");
    let utc = if format == "rfc3339" {
        OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
            .map_err(|error| format!("datetime is not RFC3339 with an explicit offset: {error}"))?
    } else {
        let description = format_description::parse_borrowed::<3>(format)
            .map_err(|error| format!("configured datetime format is invalid: {error}"))?;
        match OffsetDateTime::parse(raw, &description) {
            Ok(value) => value,
            Err(offset_error) => {
                let offset = params.assumed_offset.as_deref().ok_or_else(|| format!("datetime requires an explicit numeric offset or type_params.assumed_offset: {offset_error}"))?;
                let primitive = PrimitiveDateTime::parse(raw, &description)
                    .map_err(|error| format!("datetime value is invalid: {error}"))?;
                primitive.assume_offset(parse_offset(offset)?)
            }
        }
    };
    utc.to_offset(UtcOffset::UTC)
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|error| format!("datetime could not be normalized: {error}"))
}

pub(super) fn parse_offset(value: &str) -> Result<UtcOffset, String> {
    let bytes = value.as_bytes();
    if bytes.len() != 6 || !matches!(bytes[0], b'+' | b'-') || bytes[3] != b':' {
        return Err("assumed_offset must use +HH:MM or -HH:MM".to_owned());
    }
    let hours = value[1..3]
        .parse::<i8>()
        .map_err(|_| "assumed_offset hour is invalid")?;
    let minutes = value[4..6]
        .parse::<i8>()
        .map_err(|_| "assumed_offset minute is invalid")?;
    let sign = if bytes[0] == b'-' { -1 } else { 1 };
    UtcOffset::from_hms(sign * hours, sign * minutes, 0)
        .map_err(|error| format!("assumed_offset is invalid: {error}"))
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn validated_scalar_selection_classifies_internal_invalid_and_non_scalar_tokens() {
        assert!(matches!(
            validated_scalar_selection(" 1".to_owned()),
            Err(JsonAcquisitionFailure::Malformed(_))
        ));
        assert!(matches!(
            validated_scalar_selection("[]".to_owned()),
            Err(JsonAcquisitionFailure::NonScalarPointerTarget(_))
        ));
        assert_eq!(
            JsonScalarError::NonScalar.message(),
            "projection.pointer must select a scalar JSON leaf"
        );
    }
}

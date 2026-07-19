use std::collections::BTreeMap;
use std::str::FromStr;

use rust_decimal::Decimal;
use semver::Version;
use serde_json::value::RawValue;
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset, format_description};

use super::super::failure::SourceSuspectReason;
use super::super::{
    DeclaredType, DiagnosticDetail, DiagnosticKind, DiagnosticOperation, NumericLocale,
    TargetDocument, TypeParams,
};
use super::record::Observation;
use super::types::HtmlObservationInput;
use crate::model::plain_detail;

/// A structured JSON-acquisition failure before it becomes a persisted health fact.
#[derive(Debug)]
pub(crate) enum JsonAcquisitionFailure {
    Malformed(DiagnosticDetail),
    MissingPointerTarget(DiagnosticDetail),
    NonScalarPointerTarget(DiagnosticDetail),
}

impl JsonAcquisitionFailure {
    pub(crate) const fn reason(&self) -> SourceSuspectReason {
        match self {
            Self::Malformed(_) => SourceSuspectReason::JsonMalformed,
            Self::MissingPointerTarget(_) => SourceSuspectReason::JsonMissingPointerTarget,
            Self::NonScalarPointerTarget(_) => SourceSuspectReason::JsonNonScalarPointerTarget,
        }
    }

    pub(crate) fn into_detail(self) -> DiagnosticDetail {
        match self {
            Self::Malformed(detail)
            | Self::MissingPointerTarget(detail)
            | Self::NonScalarPointerTarget(detail) => detail,
        }
    }

    fn malformed(message: impl Into<String>) -> Self {
        Self::Malformed(plain_detail(
            DiagnosticKind::Json,
            DiagnosticOperation::JsonPointerSelection,
            message,
            None,
        ))
    }

    fn missing_pointer_target() -> Self {
        Self::MissingPointerTarget(plain_detail(
            DiagnosticKind::Json,
            DiagnosticOperation::JsonPointerSelection,
            "projection.pointer did not select a JSON value",
            None,
        ))
    }

    fn non_scalar_pointer_target() -> Self {
        Self::NonScalarPointerTarget(plain_detail(
            DiagnosticKind::Json,
            DiagnosticOperation::JsonPointerSelection,
            "projection.pointer must select a scalar JSON leaf",
            None,
        ))
    }
}

#[derive(Debug)]
pub(in crate::model) enum JsonScalarError {
    Invalid(String),
    NonScalar,
}

impl JsonScalarError {
    pub(in crate::model) fn message(&self) -> &str {
        match self {
            Self::Invalid(message) => message,
            Self::NonScalar => "projection.pointer must select a scalar JSON leaf",
        }
    }
}

pub(in crate::model) fn parse_json_scalar_token(
    target: &TargetDocument,
    raw_selected: String,
) -> Result<Observation, DiagnosticDetail> {
    let parser_input = json_input_for_declared_type(target.declared_type(), &raw_selected)
        .map_err(|error| {
            plain_detail(
                DiagnosticKind::ValueUnparseable,
                DiagnosticOperation::ValueParse,
                error.message(),
                None,
            )
        })?;
    let canonical_value =
        parse_canonical_value(target.declared_type(), target.type_params(), &parser_input)
            .map_err(|message| {
                plain_detail(
                    DiagnosticKind::ValueUnparseable,
                    DiagnosticOperation::ValueParse,
                    message,
                    None,
                )
            })?;
    Ok(Observation::json(
        raw_selected,
        target.declared_type(),
        target.type_params().clone(),
        canonical_value,
    ))
}

pub(in crate::model) fn parse_html_projection(
    target: &TargetDocument,
    input: HtmlObservationInput,
) -> Result<Observation, DiagnosticDetail> {
    let canonical_value = parse_canonical_value(
        target.declared_type(),
        target.type_params(),
        &input.comparison_projection,
    )
    .map_err(|message| {
        plain_detail(
            DiagnosticKind::ValueUnparseable,
            DiagnosticOperation::ValueParse,
            message,
            None,
        )
    })?;
    Ok(Observation::html(
        input,
        target.declared_type(),
        target.type_params().clone(),
        canonical_value,
    ))
}

/// Selects one RFC 6901 JSON Pointer scalar while preserving its exact JSON token.
pub(crate) fn select_json_scalar_token(
    body: &str,
    pointer: &str,
) -> Result<String, JsonAcquisitionFailure> {
    let root: Box<RawValue> = serde_json::from_str(body)
        .map_err(|_| JsonAcquisitionFailure::malformed("source is not valid JSON"))?;
    let mut selected =
        normalized_raw_token(root.get()).map_err(JsonAcquisitionFailure::malformed)?;
    for encoded_token in pointer.split('/').skip(1) {
        let token =
            decode_json_pointer_token(encoded_token).map_err(JsonAcquisitionFailure::malformed)?;
        selected = select_json_pointer_child(&selected, &token)?;
    }
    validated_scalar_selection(selected)
}

pub(super) fn validated_scalar_selection(
    selected: String,
) -> Result<String, JsonAcquisitionFailure> {
    match json_scalar_value(&selected) {
        Ok(_) => Ok(selected),
        Err(JsonScalarError::Invalid(message)) => Err(JsonAcquisitionFailure::malformed(message)),
        Err(JsonScalarError::NonScalar) => Err(JsonAcquisitionFailure::non_scalar_pointer_target()),
    }
}

pub(in crate::model) fn parse_canonical_value(
    kind: DeclaredType,
    params: &TypeParams,
    raw: &str,
) -> Result<String, String> {
    match kind {
        DeclaredType::Text => Ok(raw.to_owned()),
        DeclaredType::Integer => raw
            .parse::<i128>()
            .map(|value| value.to_string())
            .map_err(|_| "integer value is not a valid signed 128-bit integer".to_owned()),
        DeclaredType::Decimal | DeclaredType::Money => {
            let normalized = normalize_decimal_input(raw, params.locale)?;
            Decimal::from_str(&normalized)
                .map(|value| value.normalize().to_string())
                .map_err(|_| "decimal value is invalid or out of the supported range".to_owned())
        }
        DeclaredType::Semver => Version::parse(raw)
            .map(|value| value.to_string())
            .map_err(|_| "semantic version value is invalid".to_owned()),
        DeclaredType::Datetime => parse_datetime(raw, params),
    }
}

/// Decodes JSON evidence into the parser input allowed by one declared type.
///
/// Most declared types accept any JSON scalar because their own parser owns the final grammar.
/// Text is deliberately narrower: only a JSON string supplies its canonical Unicode scalar
/// sequence. This same boundary validates persisted JSON observations before policy can use them.
pub(in crate::model) fn json_input_for_declared_type(
    declared_type: DeclaredType,
    raw_token: &str,
) -> Result<String, JsonScalarError> {
    match declared_type {
        DeclaredType::Text => json_string_value(raw_token),
        DeclaredType::Integer
        | DeclaredType::Decimal
        | DeclaredType::Money
        | DeclaredType::Semver
        | DeclaredType::Datetime => json_scalar_value(raw_token),
    }
}

pub(in crate::model) fn normalized_raw_token(raw: &str) -> Result<String, String> {
    let token = raw.trim();
    if token.is_empty() {
        return Err("JSON scalar token is empty".to_owned());
    }
    serde_json::from_str::<Box<RawValue>>(token)
        .map_err(|_| "source is not valid JSON".to_owned())?;
    Ok(token.to_owned())
}

pub(in crate::model) fn select_json_pointer_child(
    raw_parent: &str,
    token: &str,
) -> Result<String, JsonAcquisitionFailure> {
    let child = match raw_parent.as_bytes().first() {
        Some(b'{') => {
            let entries: BTreeMap<String, Box<RawValue>> = serde_json::from_str(raw_parent)
                .map_err(|_| JsonAcquisitionFailure::malformed("source is not valid JSON"))?;
            entries.get(token).map(|value| value.get().to_owned())
        }
        Some(b'[') => {
            let index = parse_json_array_index(token);
            let entries: Vec<Box<RawValue>> = serde_json::from_str(raw_parent)
                .map_err(|_| JsonAcquisitionFailure::malformed("source is not valid JSON"))?;
            index.and_then(|index| entries.get(index).map(|value| value.get().to_owned()))
        }
        _ => None,
    }
    .ok_or_else(JsonAcquisitionFailure::missing_pointer_target)?;
    normalized_raw_token(&child).map_err(JsonAcquisitionFailure::malformed)
}

pub(in crate::model) fn decode_json_pointer_token(encoded: &str) -> Result<String, String> {
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

pub(in crate::model) fn parse_json_array_index(token: &str) -> Option<usize> {
    if token == "0" {
        Some(0)
    } else if token.starts_with('0') {
        None
    } else {
        token.parse::<usize>().ok()
    }
}

pub(in crate::model) fn json_scalar_value(raw_token: &str) -> Result<String, JsonScalarError> {
    if raw_token != raw_token.trim() {
        return Err(JsonScalarError::Invalid(
            "JSON scalar token must not contain outer whitespace".to_owned(),
        ));
    }
    let value: serde_json::Value = serde_json::from_str(raw_token)
        .map_err(|_| JsonScalarError::Invalid("JSON scalar token is invalid".to_owned()))?;
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

/// Decodes one JSON string while refusing other scalar kinds.
pub(in crate::model) fn json_string_value(raw_token: &str) -> Result<String, JsonScalarError> {
    if raw_token != raw_token.trim() {
        return Err(JsonScalarError::Invalid(
            "JSON scalar token must not contain outer whitespace".to_owned(),
        ));
    }
    let value: serde_json::Value = serde_json::from_str(raw_token)
        .map_err(|_| JsonScalarError::Invalid("JSON scalar token is invalid".to_owned()))?;
    match value {
        serde_json::Value::String(value) => Ok(value),
        serde_json::Value::Number(_) | serde_json::Value::Bool(_) | serde_json::Value::Null => Err(
            JsonScalarError::Invalid("text declared_type requires a JSON string".to_owned()),
        ),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err(JsonScalarError::NonScalar)
        }
    }
}

pub(in crate::model) fn normalize_decimal_input(
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

pub(in crate::model) fn normalize_grouped_decimal(
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

pub(in crate::model) fn parse_datetime(raw: &str, params: &TypeParams) -> Result<String, String> {
    let format = params.format.as_deref().expect("validated datetime params");
    let utc = if format == "rfc3339" {
        OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
            .map_err(|_| "datetime must be RFC 3339 with an explicit offset".to_owned())?
    } else {
        let description = format_description::parse_borrowed::<3>(format)
            .map_err(|_| "configured datetime format is invalid".to_owned())?;
        match OffsetDateTime::parse(raw, &description) {
            Ok(value) => value,
            Err(_) => {
                let offset = params.assumed_offset.as_deref().ok_or_else(|| {
                    "datetime requires an explicit numeric offset or type_params.assumed_offset"
                        .to_owned()
                })?;
                let primitive = PrimitiveDateTime::parse(raw, &description)
                    .map_err(|_| "datetime value is invalid".to_owned())?;
                primitive.assume_offset(parse_offset(offset)?)
            }
        }
    };
    utc.to_offset(UtcOffset::UTC)
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| "datetime value could not be normalized to canonical UTC".to_owned())
}

pub(in crate::model) fn parse_offset(value: &str) -> Result<UtcOffset, String> {
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
        .map_err(|_| "assumed_offset is outside the supported UTC-offset range".to_owned())
}

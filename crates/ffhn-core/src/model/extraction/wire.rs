use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::{Extensions, ExtractionRecord, SnapshotReference};
use crate::CoreError;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawExtractionRecord {
    schema_name: String,
    schema_version: u32,
    interop_profile: String,
    htmlcut_plan_digest_sha256: String,
    htmlcut_result_digest_sha256: String,
    comparison_input_sha256: String,
    outer_html_sha256: String,
    strategy_kind: crate::SelectionKind,
    selection_mode: crate::SelectionMatch,
    output_kind: crate::OutputKind,
    candidate_count: usize,
    selected_candidate_index: usize,
    match_metadata: Value,
    warning_codes: Vec<String>,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawSnapshotReference {
    slot: crate::SnapshotSlot,
    canonical_text_sha256: String,
    outer_html_sha256: String,
    extraction_record_path: String,
    canonical_text_path: String,
    outer_html_path: String,
    captured_at: String,
}

impl TryFrom<RawExtractionRecord> for ExtractionRecord {
    type Error = CoreError;

    fn try_from(raw: RawExtractionRecord) -> Result<Self, Self::Error> {
        let record = Self {
            schema_name: raw.schema_name,
            schema_version: raw.schema_version,
            interop_profile: raw.interop_profile,
            htmlcut_plan_digest_sha256: raw.htmlcut_plan_digest_sha256,
            htmlcut_result_digest_sha256: raw.htmlcut_result_digest_sha256,
            comparison_input_sha256: raw.comparison_input_sha256,
            outer_html_sha256: raw.outer_html_sha256,
            strategy_kind: raw.strategy_kind,
            selection_mode: raw.selection_mode,
            output_kind: raw.output_kind,
            candidate_count: raw.candidate_count,
            selected_candidate_index: raw.selected_candidate_index,
            match_metadata: raw.match_metadata,
            warning_codes: raw.warning_codes,
            created_at: raw.created_at,
            extensions: raw.extensions,
        };
        record.validate()?;
        Ok(record)
    }
}

impl TryFrom<RawSnapshotReference> for SnapshotReference {
    type Error = CoreError;

    fn try_from(raw: RawSnapshotReference) -> Result<Self, Self::Error> {
        let reference = Self {
            slot: raw.slot,
            canonical_text_sha256: raw.canonical_text_sha256,
            outer_html_sha256: raw.outer_html_sha256,
            extraction_record_path: raw.extraction_record_path.try_into()?,
            canonical_text_path: raw.canonical_text_path.try_into()?,
            outer_html_path: raw.outer_html_path.try_into()?,
            captured_at: raw.captured_at,
        };
        reference.validate()?;
        Ok(reference)
    }
}

impl From<&ExtractionRecord> for RawExtractionRecord {
    fn from(record: &ExtractionRecord) -> Self {
        Self {
            schema_name: record.schema_name.clone(),
            schema_version: record.schema_version,
            interop_profile: record.interop_profile.clone(),
            htmlcut_plan_digest_sha256: record.htmlcut_plan_digest_sha256.clone(),
            htmlcut_result_digest_sha256: record.htmlcut_result_digest_sha256.clone(),
            comparison_input_sha256: record.comparison_input_sha256.clone(),
            outer_html_sha256: record.outer_html_sha256.clone(),
            strategy_kind: record.strategy_kind,
            selection_mode: record.selection_mode,
            output_kind: record.output_kind,
            candidate_count: record.candidate_count,
            selected_candidate_index: record.selected_candidate_index,
            match_metadata: record.match_metadata.clone(),
            warning_codes: record.warning_codes.clone(),
            created_at: record.created_at.clone(),
            extensions: record.extensions.clone(),
        }
    }
}

impl From<&SnapshotReference> for RawSnapshotReference {
    fn from(reference: &SnapshotReference) -> Self {
        Self {
            slot: reference.slot,
            canonical_text_sha256: reference.canonical_text_sha256.clone(),
            outer_html_sha256: reference.outer_html_sha256.clone(),
            extraction_record_path: reference.extraction_record_path.as_str().to_owned(),
            canonical_text_path: reference.canonical_text_path.as_str().to_owned(),
            outer_html_path: reference.outer_html_path.as_str().to_owned(),
            captured_at: reference.captured_at.clone(),
        }
    }
}

impl Serialize for ExtractionRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawExtractionRecord::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ExtractionRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawExtractionRecord::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

impl Serialize for SnapshotReference {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawSnapshotReference::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SnapshotReference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawSnapshotReference::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

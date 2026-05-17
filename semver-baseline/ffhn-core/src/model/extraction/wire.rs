use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    CompareBasis, Extensions, ExtractionRecord, SelectionEvidence, SelectionKind, SelectionMatch,
};
use crate::CoreError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExtractionRecord {
    schema_name: String,
    schema_version: u32,
    compare_source_sha256: String,
    outer_html_sha256: String,
    selection_kind: SelectionKind,
    selection_match: SelectionMatch,
    compare_basis: CompareBasis,
    candidate_count: usize,
    selected_candidate_index: usize,
    selection_evidence: SelectionEvidence,
    warning_codes: Vec<String>,
    created_at: String,
    monitoring_contract_digest_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
}

impl TryFrom<RawExtractionRecord> for ExtractionRecord {
    type Error = CoreError;

    fn try_from(raw: RawExtractionRecord) -> Result<Self, Self::Error> {
        let record = Self {
            schema_name: raw.schema_name,
            schema_version: raw.schema_version,
            compare_source_sha256: raw.compare_source_sha256,
            outer_html_sha256: raw.outer_html_sha256,
            selection_kind: raw.selection_kind,
            selection_match: raw.selection_match,
            compare_basis: raw.compare_basis,
            candidate_count: raw.candidate_count,
            selected_candidate_index: raw.selected_candidate_index,
            selection_evidence: raw.selection_evidence,
            warning_codes: raw.warning_codes,
            created_at: raw.created_at,
            monitoring_contract_digest_sha256: raw.monitoring_contract_digest_sha256,
            extensions: raw.extensions,
        };
        record.validate()?;
        Ok(record)
    }
}

impl From<&ExtractionRecord> for RawExtractionRecord {
    fn from(record: &ExtractionRecord) -> Self {
        Self {
            schema_name: record.schema_name.clone(),
            schema_version: record.schema_version,
            compare_source_sha256: record.compare_source_sha256.clone(),
            outer_html_sha256: record.outer_html_sha256.clone(),
            selection_kind: record.selection_kind,
            selection_match: record.selection_match,
            compare_basis: record.compare_basis,
            candidate_count: record.candidate_count,
            selected_candidate_index: record.selected_candidate_index,
            selection_evidence: record.selection_evidence.clone(),
            warning_codes: record.warning_codes.clone(),
            created_at: record.created_at.clone(),
            monitoring_contract_digest_sha256: record.monitoring_contract_digest_sha256.clone(),
            extensions: record.extensions.clone(),
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

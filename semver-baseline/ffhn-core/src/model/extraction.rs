use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::CoreError;

use super::schema::{
    EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION, HTMLCUT_INTEROP_PROFILE,
};
use super::validate::{validate_identity, validate_sha256, validate_timestamp};
use super::{
    Extensions, OutputKind, RelativeArtifactPath, SelectionKind, SelectionMatch, SnapshotSlot,
};

/// Persisted extraction-record schema.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(try_from = "RawExtractionRecord")]
pub struct ExtractionRecord {
    /// Frozen schema identity.
    pub(crate) schema_name: String,
    /// Frozen schema version.
    pub(crate) schema_version: u32,
    /// Frozen HTMLCut interop profile.
    pub(crate) interop_profile: String,
    /// Digest of the exact HTMLCut plan document.
    pub(crate) htmlcut_plan_digest_sha256: String,
    /// Digest of the exact HTMLCut result document.
    pub(crate) htmlcut_result_digest_sha256: String,
    /// Digest of `comparison_input_text` after LF normalization.
    pub(crate) comparison_input_sha256: String,
    /// Digest of persisted `outer.html`.
    pub(crate) outer_html_sha256: String,
    /// Strategy kind echoed from HTMLCut.
    pub(crate) strategy_kind: SelectionKind,
    /// Selection mode echoed from HTMLCut.
    pub(crate) selection_mode: SelectionMatch,
    /// Output kind echoed from HTMLCut.
    pub(crate) output_kind: OutputKind,
    /// Total candidate count.
    pub(crate) candidate_count: usize,
    /// Selected one-based candidate index.
    pub(crate) selected_candidate_index: usize,
    /// Stable selected-match metadata object copied from HTMLCut.
    pub(crate) match_metadata: Value,
    /// Warning codes only.
    pub(crate) warning_codes: Vec<String>,
    /// Record creation timestamp.
    pub(crate) created_at: String,
    /// Reserved extensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) extensions: Extensions,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExtractionRecord {
    schema_name: String,
    schema_version: u32,
    interop_profile: String,
    htmlcut_plan_digest_sha256: String,
    htmlcut_result_digest_sha256: String,
    comparison_input_sha256: String,
    outer_html_sha256: String,
    strategy_kind: SelectionKind,
    selection_mode: SelectionMatch,
    output_kind: OutputKind,
    candidate_count: usize,
    selected_candidate_index: usize,
    match_metadata: Value,
    warning_codes: Vec<String>,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
}

impl ExtractionRecord {
    /// Validates one persisted extraction record.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_extraction_record_identity(&self.schema_name, self.schema_version)?;
        if self.interop_profile != HTMLCUT_INTEROP_PROFILE {
            return Err(CoreError::contract(
                "ffhn.extraction_record interop_profile must match the FFHN HTMLCut profile",
            ));
        }
        validate_sha256(&self.htmlcut_plan_digest_sha256)?;
        validate_sha256(&self.htmlcut_result_digest_sha256)?;
        validate_sha256(&self.comparison_input_sha256)?;
        validate_sha256(&self.outer_html_sha256)?;
        if self.candidate_count == 0 || self.selected_candidate_index == 0 {
            return Err(CoreError::contract(
                "ffhn.extraction_record candidate counts must be positive",
            ));
        }
        if self.selected_candidate_index > self.candidate_count {
            return Err(CoreError::contract(
                "ffhn.extraction_record selected_candidate_index must be within candidate_count",
            ));
        }
        if !self.match_metadata.is_object() {
            return Err(CoreError::contract(
                "ffhn.extraction_record match_metadata must be an object",
            ));
        }
        validate_timestamp(&self.created_at)
    }
}

fn validate_extraction_record_identity(name: &str, version: u32) -> Result<(), CoreError> {
    validate_identity(
        name,
        EXTRACTION_RECORD_SCHEMA_NAME,
        version,
        EXTRACTION_RECORD_SCHEMA_VERSION,
    )
}

/// Snapshot reference in `ffhn.state`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "RawSnapshotReference")]
pub struct SnapshotReference {
    /// Slot discriminator.
    pub(crate) slot: SnapshotSlot,
    /// Digest of persisted canonical artifact.
    pub(crate) canonical_text_sha256: String,
    /// Digest of persisted outer HTML artifact.
    pub(crate) outer_html_sha256: String,
    /// Relative extraction-record path.
    pub(crate) extraction_record_path: RelativeArtifactPath,
    /// Relative canonical artifact path.
    pub(crate) canonical_text_path: RelativeArtifactPath,
    /// Relative outer HTML artifact path.
    pub(crate) outer_html_path: RelativeArtifactPath,
    /// Capture timestamp.
    pub(crate) captured_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSnapshotReference {
    slot: SnapshotSlot,
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

impl SnapshotReference {
    /// Validates one snapshot reference.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_sha256(&self.canonical_text_sha256)?;
        validate_sha256(&self.outer_html_sha256)?;
        validate_timestamp(&self.captured_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION, HTMLCUT_INTEROP_PROFILE,
    };
    use serde_json::json;

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn valid_record() -> ExtractionRecord {
        ExtractionRecord {
            schema_name: EXTRACTION_RECORD_SCHEMA_NAME.to_owned(),
            schema_version: EXTRACTION_RECORD_SCHEMA_VERSION,
            interop_profile: HTMLCUT_INTEROP_PROFILE.to_owned(),
            htmlcut_plan_digest_sha256: DIGEST.to_owned(),
            htmlcut_result_digest_sha256: DIGEST.to_owned(),
            comparison_input_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            strategy_kind: SelectionKind::CssSelector,
            selection_mode: SelectionMatch::Single,
            output_kind: OutputKind::OuterHtml,
            candidate_count: 1,
            selected_candidate_index: 1,
            match_metadata: json!({"selector": "main"}),
            warning_codes: vec!["warn".to_owned()],
            created_at: "2026-04-05T10:15:30Z".to_owned(),
            extensions: None,
        }
    }

    #[test]
    fn extraction_record_validation_accepts_the_canonical_shape() {
        valid_record().validate().expect("record");
    }

    #[test]
    fn extraction_record_validation_rejects_invalid_contract_data() {
        let mut record = valid_record();
        record.interop_profile = "other".to_owned();
        assert!(record.validate().is_err());

        let mut record = valid_record();
        record.htmlcut_plan_digest_sha256 = "bad".to_owned();
        assert!(record.validate().is_err());

        let mut record = valid_record();
        record.htmlcut_result_digest_sha256 = "bad".to_owned();
        assert!(record.validate().is_err());

        let mut record = valid_record();
        record.candidate_count = 0;
        assert!(record.validate().is_err());

        let mut record = valid_record();
        record.selected_candidate_index = 0;
        assert!(record.validate().is_err());

        let mut record = valid_record();
        record.selected_candidate_index = 2;
        assert!(record.validate().is_err());

        let mut record = valid_record();
        record.match_metadata = json!(["not", "an", "object"]);
        assert!(record.validate().is_err());

        let mut record = valid_record();
        record.created_at = "bad".to_owned();
        assert!(record.validate().is_err());
    }

    #[test]
    fn snapshot_reference_validation_checks_digests_paths_and_timestamp() {
        SnapshotReference {
            slot: SnapshotSlot::Current,
            canonical_text_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            extraction_record_path: RelativeArtifactPath::new("snapshots/current/extraction.json")
                .expect("relative path"),
            canonical_text_path: RelativeArtifactPath::new("snapshots/current/canonical.txt")
                .expect("relative path"),
            outer_html_path: RelativeArtifactPath::new("snapshots/current/outer.html")
                .expect("relative path"),
            captured_at: "2026-04-05T10:15:30Z".to_owned(),
        }
        .validate()
        .expect("snapshot reference");

        assert!(
            serde_json::from_str::<SnapshotReference>(
                r#"{
                    "slot":"current",
                    "canonical_text_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "outer_html_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "extraction_record_path":"../escape",
                    "canonical_text_path":"snapshots/current/canonical.txt",
                    "outer_html_path":"snapshots/current/outer.html",
                    "captured_at":"2026-04-05T10:15:30Z"
                }"#
            )
            .is_err()
        );
    }
}

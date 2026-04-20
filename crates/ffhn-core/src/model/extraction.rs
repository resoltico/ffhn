use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::CoreError;

use super::schema::{
    EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION, HTMLCUT_INTEROP_PROFILE,
};
use super::validate::{
    validate_identity, validate_relative_path, validate_sha256, validate_timestamp,
};
use super::{Extensions, OutputKind, SelectionKind, SelectionMatch, SnapshotSlot};

/// Persisted extraction-record schema.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExtractionRecord {
    /// Frozen schema identity.
    pub schema_name: String,
    /// Frozen schema version.
    pub schema_version: u32,
    /// Frozen HTMLCut interop profile.
    pub interop_profile: String,
    /// Digest of the exact HTMLCut plan document.
    pub htmlcut_plan_digest_sha256: String,
    /// Digest of the exact HTMLCut result document.
    pub htmlcut_result_digest_sha256: String,
    /// Digest of `comparison_input_text` after LF normalization.
    pub comparison_input_sha256: String,
    /// Digest of persisted `outer.html`.
    pub outer_html_sha256: String,
    /// Strategy kind echoed from HTMLCut.
    pub strategy_kind: SelectionKind,
    /// Selection mode echoed from HTMLCut.
    pub selection_mode: SelectionMatch,
    /// Output kind echoed from HTMLCut.
    pub output_kind: OutputKind,
    /// Total candidate count.
    pub candidate_count: usize,
    /// Selected one-based candidate index.
    pub selected_candidate_index: usize,
    /// Stable selected-match metadata object copied from HTMLCut.
    pub match_metadata: Value,
    /// Warning codes only.
    pub warning_codes: Vec<String>,
    /// Record creation timestamp.
    pub created_at: String,
    /// Reserved extensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Extensions,
}

impl ExtractionRecord {
    /// Validates one persisted extraction record.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_extraction_record_identity(&self.schema_name, self.schema_version)?;
        if self.interop_profile != HTMLCUT_INTEROP_PROFILE {
            return Err(CoreError::htmlcut(
                "ffhn.extraction_record interop_profile must match the FFHN HTMLCut profile",
            ));
        }
        validate_sha256(&self.htmlcut_plan_digest_sha256)?;
        validate_sha256(&self.htmlcut_result_digest_sha256)?;
        validate_sha256(&self.comparison_input_sha256)?;
        validate_sha256(&self.outer_html_sha256)?;
        if self.candidate_count == 0 || self.selected_candidate_index == 0 {
            return Err(CoreError::htmlcut(
                "ffhn.extraction_record candidate counts must be positive",
            ));
        }
        if self.selected_candidate_index > self.candidate_count {
            return Err(CoreError::htmlcut(
                "ffhn.extraction_record selected_candidate_index must be within candidate_count",
            ));
        }
        if !self.match_metadata.is_object() {
            return Err(CoreError::htmlcut(
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
#[serde(deny_unknown_fields)]
pub struct SnapshotReference {
    /// Slot discriminator.
    pub slot: SnapshotSlot,
    /// Digest of persisted canonical artifact.
    pub canonical_text_sha256: String,
    /// Digest of persisted outer HTML artifact.
    pub outer_html_sha256: String,
    /// Relative extraction-record path.
    pub extraction_record_path: String,
    /// Relative canonical artifact path.
    pub canonical_text_path: String,
    /// Relative outer HTML artifact path.
    pub outer_html_path: String,
    /// Capture timestamp.
    pub captured_at: String,
}

impl SnapshotReference {
    /// Validates one snapshot reference.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_sha256(&self.canonical_text_sha256)?;
        validate_sha256(&self.outer_html_sha256)?;
        validate_relative_path("extraction_record_path", &self.extraction_record_path)?;
        validate_relative_path("canonical_text_path", &self.canonical_text_path)?;
        validate_relative_path("outer_html_path", &self.outer_html_path)?;
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
            extraction_record_path: "snapshots/current/extraction.json".to_owned(),
            canonical_text_path: "snapshots/current/canonical.txt".to_owned(),
            outer_html_path: "snapshots/current/outer.html".to_owned(),
            captured_at: "2026-04-05T10:15:30Z".to_owned(),
        }
        .validate()
        .expect("snapshot reference");

        assert!(
            SnapshotReference {
                slot: SnapshotSlot::Current,
                canonical_text_sha256: DIGEST.to_owned(),
                outer_html_sha256: DIGEST.to_owned(),
                extraction_record_path: "../escape".to_owned(),
                canonical_text_path: "snapshots/current/canonical.txt".to_owned(),
                outer_html_path: "snapshots/current/outer.html".to_owned(),
                captured_at: "2026-04-05T10:15:30Z".to_owned(),
            }
            .validate()
            .is_err()
        );
    }
}

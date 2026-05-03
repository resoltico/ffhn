use super::super::schema::{
    EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION, HTMLCUT_INTEROP_PROFILE,
};
use super::super::validate::{validate_identity, validate_sha256, validate_timestamp};
use super::*;

impl ExtractionRecord {
    /// Validates one persisted extraction record.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the schema identity, HTMLCut interop profile, digests, selected
    /// match counts, metadata object, or timestamp violates FFHN's frozen extraction-record
    /// contract.
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

impl SnapshotReference {
    /// Validates one snapshot reference.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the stored digests or capture timestamp violate FFHN's frozen
    /// snapshot-reference contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_sha256(&self.canonical_text_sha256)?;
        validate_sha256(&self.outer_html_sha256)?;
        validate_timestamp(&self.captured_at)
    }
}

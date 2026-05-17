use super::super::schema::{EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION};
use super::super::validate::{
    require_non_empty, validate_identity, validate_sha256, validate_timestamp,
};
use super::*;

impl ExtractionRecord {
    /// Validates one persisted extraction record.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the schema identity, digests, selected match counts, selection
    /// evidence, or timestamp violates FFHN's frozen extraction-record contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_extraction_record_identity(&self.schema_name, self.schema_version)?;
        validate_sha256(&self.compare_source_sha256)?;
        validate_sha256(&self.outer_html_sha256)?;
        validate_sha256(&self.monitoring_contract_digest_sha256)?;
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
        self.selection_evidence.validate(self.selection_kind)?;
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

impl SelectionEvidence {
    fn validate(&self, selection_kind: SelectionKind) -> Result<(), CoreError> {
        match (selection_kind, self) {
            (SelectionKind::CssSelector, Self::CssSelector { path, tag_name }) => {
                require_non_empty("selection_evidence.path", path)?;
                require_non_empty("selection_evidence.tag_name", tag_name)
            }
            (
                SelectionKind::DelimiterPair,
                Self::DelimiterPair {
                    selected_range,
                    inner_range,
                    outer_range,
                    ..
                },
            ) => {
                selected_range.validate("selection_evidence.selected_range")?;
                inner_range.validate("selection_evidence.inner_range")?;
                outer_range.validate("selection_evidence.outer_range")
            }
            (SelectionKind::CssSelector, _) => Err(CoreError::contract(
                "ffhn.extraction_record selection_kind css_selector requires css_selector evidence",
            )),
            (SelectionKind::DelimiterPair, _) => Err(CoreError::contract(
                "ffhn.extraction_record selection_kind delimiter_pair requires delimiter_pair evidence",
            )),
        }
    }
}

impl SelectionRange {
    fn validate(&self, field: &str) -> Result<(), CoreError> {
        if self.end_byte < self.start_byte {
            return Err(CoreError::contract(format!(
                "{field}.end_byte must be at least {field}.start_byte",
            )));
        }
        Ok(())
    }
}

impl SnapshotReference {
    /// Validates one snapshot reference.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the stored digests or capture timestamp violate FFHN's frozen
    /// snapshot-reference contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_sha256(&self.compare_digest_sha256)?;
        validate_sha256(&self.outer_html_sha256)?;
        validate_timestamp(&self.captured_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_evidence_validation_rejects_kind_mismatches_and_reversed_ranges() {
        let css = SelectionEvidence::CssSelector {
            path: "html > body > main".to_owned(),
            tag_name: "main".to_owned(),
        };
        assert!(css.validate(SelectionKind::DelimiterPair).is_err());

        let delimiter = SelectionEvidence::DelimiterPair {
            selected_range: SelectionRange {
                start_byte: 4,
                end_byte: 3,
            },
            inner_range: SelectionRange {
                start_byte: 4,
                end_byte: 4,
            },
            outer_range: SelectionRange {
                start_byte: 3,
                end_byte: 5,
            },
            include_start: true,
            include_end: false,
        };
        assert!(delimiter.validate(SelectionKind::DelimiterPair).is_err());
    }
}

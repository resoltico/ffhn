use super::*;

impl ExtractionRecord {
    /// Returns the frozen schema name.
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    /// Returns the frozen schema version.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the frozen HTMLCut interop profile.
    pub fn interop_profile(&self) -> &str {
        &self.interop_profile
    }

    /// Returns the exact HTMLCut plan digest.
    pub fn htmlcut_plan_digest_sha256(&self) -> &str {
        &self.htmlcut_plan_digest_sha256
    }

    /// Returns the exact HTMLCut result digest.
    pub fn htmlcut_result_digest_sha256(&self) -> &str {
        &self.htmlcut_result_digest_sha256
    }

    /// Returns the normalized comparison-input digest.
    pub fn comparison_input_sha256(&self) -> &str {
        &self.comparison_input_sha256
    }

    /// Returns the persisted outer-HTML digest.
    pub fn outer_html_sha256(&self) -> &str {
        &self.outer_html_sha256
    }

    /// Returns the echoed selection strategy kind.
    pub fn strategy_kind(&self) -> SelectionKind {
        self.strategy_kind
    }

    /// Returns the echoed selection mode.
    pub fn selection_mode(&self) -> SelectionMatch {
        self.selection_mode
    }

    /// Returns the echoed output kind.
    pub fn output_kind(&self) -> OutputKind {
        self.output_kind
    }

    /// Returns the total candidate count.
    pub fn candidate_count(&self) -> usize {
        self.candidate_count
    }

    /// Returns the selected one-based candidate index.
    pub fn selected_candidate_index(&self) -> usize {
        self.selected_candidate_index
    }

    /// Returns the stable selected-match metadata object.
    pub fn match_metadata(&self) -> &Value {
        &self.match_metadata
    }

    /// Returns warning codes emitted by HTMLCut.
    pub fn warning_codes(&self) -> &[String] {
        &self.warning_codes
    }

    /// Returns the record creation timestamp.
    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    /// Returns any reserved extensions.
    pub fn extensions(&self) -> Option<&std::collections::BTreeMap<String, Value>> {
        self.extensions.as_ref()
    }
}

impl SnapshotReference {
    /// Returns the snapshot slot.
    pub fn slot(&self) -> SnapshotSlot {
        self.slot
    }

    /// Returns the canonical-text digest.
    pub fn canonical_text_sha256(&self) -> &str {
        &self.canonical_text_sha256
    }

    /// Returns the outer-HTML digest.
    pub fn outer_html_sha256(&self) -> &str {
        &self.outer_html_sha256
    }

    /// Returns the relative extraction-record path.
    pub fn extraction_record_path(&self) -> &RelativeArtifactPath {
        &self.extraction_record_path
    }

    /// Returns the relative canonical-text path.
    pub fn canonical_text_path(&self) -> &RelativeArtifactPath {
        &self.canonical_text_path
    }

    /// Returns the relative outer-HTML path.
    pub fn outer_html_path(&self) -> &RelativeArtifactPath {
        &self.outer_html_path
    }

    /// Returns the capture timestamp.
    pub fn captured_at(&self) -> &str {
        &self.captured_at
    }
}

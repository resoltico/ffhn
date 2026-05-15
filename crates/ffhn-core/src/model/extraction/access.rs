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

    /// Returns the normalized comparison-input digest.
    pub fn comparison_input_sha256(&self) -> &str {
        &self.comparison_input_sha256
    }

    /// Returns the persisted outer-HTML digest.
    pub fn outer_html_sha256(&self) -> &str {
        &self.outer_html_sha256
    }

    /// Returns the selection strategy kind.
    pub fn selection_kind(&self) -> SelectionKind {
        self.selection_kind
    }

    /// Returns the selection mode.
    pub fn selection_match(&self) -> SelectionMatch {
        self.selection_match
    }

    /// Returns the output kind.
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

    /// Returns the FFHN-owned selection evidence.
    pub const fn selection_evidence(&self) -> &SelectionEvidence {
        &self.selection_evidence
    }

    /// Returns warning codes surfaced through FFHN's extractor seam.
    pub fn warning_codes(&self) -> &[String] {
        &self.warning_codes
    }

    /// Returns the record creation timestamp.
    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    /// Returns any reserved extensions.
    pub fn extensions(&self) -> Option<&std::collections::BTreeMap<String, serde_json::Value>> {
        self.extensions.as_ref()
    }
}

impl SelectionRange {
    /// Returns the inclusive start byte offset.
    pub const fn start_byte(&self) -> usize {
        self.start_byte
    }

    /// Returns the exclusive end byte offset.
    pub const fn end_byte(&self) -> usize {
        self.end_byte
    }
}

impl SelectionEvidence {
    /// Returns the selection kind represented by this evidence.
    pub const fn kind(&self) -> SelectionKind {
        match self {
            Self::CssSelector { .. } => SelectionKind::CssSelector,
            Self::DelimiterPair { .. } => SelectionKind::DelimiterPair,
        }
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

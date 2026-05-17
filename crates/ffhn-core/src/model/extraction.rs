use crate::CoreError;

use super::{
    CompareBasis, Extensions, RelativeArtifactPath, SelectionKind, SelectionMatch, SnapshotSlot,
};

mod access;
#[cfg(test)]
mod tests;
mod validation;
mod wire;

/// FFHN-owned byte range captured from one selected extraction.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SelectionRange {
    /// Inclusive start byte offset.
    pub(crate) start_byte: usize,
    /// Exclusive end byte offset.
    pub(crate) end_byte: usize,
}

/// FFHN-owned selection evidence for one extracted payload.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SelectionEvidence {
    /// Selector-backed evidence.
    CssSelector {
        /// DOM path to the selected node.
        path: String,
        /// Matched tag name.
        tag_name: String,
    },
    /// Delimiter-backed evidence.
    DelimiterPair {
        /// Selected byte range after include-start/include-end policy.
        selected_range: SelectionRange,
        /// Inner byte range between the matched boundaries.
        inner_range: SelectionRange,
        /// Outer byte range including both matched boundaries.
        outer_range: SelectionRange,
        /// Whether the selected payload includes the matched start boundary.
        include_start: bool,
        /// Whether the selected payload includes the matched end boundary.
        include_end: bool,
    },
}

/// Persisted extraction-record schema.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtractionRecord {
    /// Frozen schema identity.
    pub(crate) schema_name: String,
    /// Frozen schema version.
    pub(crate) schema_version: u32,
    /// Digest of the selected compare-source projection after LF normalization.
    pub(crate) compare_source_sha256: String,
    /// Digest of persisted `outer.html`.
    pub(crate) outer_html_sha256: String,
    /// Selection strategy kind.
    pub(crate) selection_kind: SelectionKind,
    /// Selection mode.
    pub(crate) selection_match: SelectionMatch,
    /// Compare basis used to project the selected fragment.
    pub(crate) compare_basis: CompareBasis,
    /// Total candidate count.
    pub(crate) candidate_count: usize,
    /// Selected one-based candidate index.
    pub(crate) selected_candidate_index: usize,
    /// FFHN-owned evidence for the selected payload.
    pub(crate) selection_evidence: SelectionEvidence,
    /// Warning codes surfaced through FFHN's extractor seam.
    pub(crate) warning_codes: Vec<String>,
    /// Record creation timestamp.
    pub(crate) created_at: String,
    /// Digest of the monitoring contract that produced the snapshot.
    pub(crate) monitoring_contract_digest_sha256: String,
    /// Reserved extensions.
    pub(crate) extensions: Extensions,
}

/// Snapshot reference in `ffhn.state`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotReference {
    /// Slot discriminator.
    pub(crate) slot: SnapshotSlot,
    /// Digest of persisted compare artifact.
    pub(crate) compare_digest_sha256: String,
    /// Digest of persisted outer HTML artifact.
    pub(crate) outer_html_sha256: String,
    /// Relative extraction-record path.
    pub(crate) extraction_record_path: RelativeArtifactPath,
    /// Relative compare artifact path.
    pub(crate) compare_path: RelativeArtifactPath,
    /// Relative outer HTML artifact path.
    pub(crate) outer_html_path: RelativeArtifactPath,
    /// Capture timestamp.
    pub(crate) captured_at: String,
}

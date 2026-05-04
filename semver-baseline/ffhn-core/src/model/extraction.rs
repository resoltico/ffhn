use serde_json::Value;

use crate::CoreError;

use super::{
    Extensions, OutputKind, RelativeArtifactPath, SelectionKind, SelectionMatch, SnapshotSlot,
};

mod access;
#[cfg(test)]
mod tests;
mod validation;
mod wire;

/// Persisted extraction-record schema.
#[derive(Clone, Debug, PartialEq)]
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
    pub(crate) extensions: Extensions,
}

/// Snapshot reference in `ffhn.state`.
#[derive(Clone, Debug, PartialEq, Eq)]
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

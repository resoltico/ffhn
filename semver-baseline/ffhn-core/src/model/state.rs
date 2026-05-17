use crate::CoreError;

use super::TargetId;
use super::{
    BaselinePhase, Extensions, RunFailureCause, RunOutcome, SnapshotReference, SnapshotSlot,
};

mod access;
#[cfg(test)]
mod tests;
mod validation;
mod wire;

/// Persisted baseline state inside `ffhn.state`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum StoredBaseline {
    /// No successful baseline exists yet.
    Pending,
    /// One current baseline exists and may carry retained history snapshots.
    Ready {
        /// Current snapshot ref.
        current_snapshot: SnapshotReference,
        /// Older retained snapshots, newest first.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        snapshot_history: Vec<SnapshotReference>,
    },
}

/// Last persisted live-run summary inside `ffhn.state`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LastRunRecord {
    /// Most recent attempted live-run timestamp.
    run_at: String,
    /// Stable run outcome for the last live attempt.
    outcome: RunOutcome,
    /// Local cause for the failed run when the last attempt failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<RunFailureCause>,
}

/// Persisted FFHN state schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateDocument {
    /// Frozen schema identity.
    pub(crate) schema_name: String,
    /// Frozen schema version.
    pub(crate) schema_version: u32,
    /// Target id.
    pub(crate) target_id: TargetId,
    /// Digest of the monitoring contract that owns the stored baseline.
    pub(crate) monitoring_contract_digest_sha256: String,
    /// Current baseline state.
    pub(crate) baseline: StoredBaseline,
    /// Most recent attempted live run when one exists.
    pub(crate) last_run: Option<LastRunRecord>,
    /// Reserved extensions.
    pub(crate) extensions: Extensions,
}

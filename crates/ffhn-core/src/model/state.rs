use crate::CoreError;

use super::TargetId;
use super::{Extensions, ReasonCode, RunOutcome, SnapshotReference, SnapshotSlot, StatePhase};

mod access;
#[cfg(test)]
mod tests;
mod validation;
mod wire;

/// Persisted FFHN state schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateDocument {
    /// Frozen schema identity.
    pub(crate) schema_name: String,
    /// Frozen schema version.
    pub(crate) schema_version: u32,
    /// Target id.
    pub(crate) target_id: TargetId,
    /// Current state phase.
    pub(crate) state_phase: StatePhase,
    /// Most recent attempted run time.
    pub(crate) last_run_at: Option<String>,
    /// Most recent attempted run outcome.
    pub(crate) last_run_outcome: Option<RunOutcome>,
    /// Most recent attempted run reason.
    pub(crate) last_reason_code: Option<ReasonCode>,
    /// Current snapshot ref.
    pub(crate) current_snapshot: Option<SnapshotReference>,
    /// Older retained snapshots, newest first.
    pub(crate) snapshot_history: Vec<SnapshotReference>,
    /// Reserved extensions.
    pub(crate) extensions: Extensions,
}

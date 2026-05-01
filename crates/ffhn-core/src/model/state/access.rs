use super::*;

impl StateDocument {
    /// Returns the frozen schema name.
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    /// Returns the frozen schema version.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the target id encoded in the state document.
    pub fn target_id(&self) -> &str {
        self.target_id.as_str()
    }

    /// Returns the persisted state phase.
    pub fn state_phase(&self) -> StatePhase {
        self.state_phase
    }

    /// Returns the last persisted run timestamp when one exists.
    pub fn last_run_at(&self) -> Option<&str> {
        self.last_run_at.as_deref()
    }

    /// Returns the last persisted run outcome when one exists.
    pub fn last_run_outcome(&self) -> Option<RunOutcome> {
        self.last_run_outcome
    }

    /// Returns the last persisted reason code when one exists.
    pub fn last_reason_code(&self) -> Option<ReasonCode> {
        self.last_reason_code
    }

    /// Returns the current snapshot reference when one exists.
    pub fn current_snapshot(&self) -> Option<&SnapshotReference> {
        self.current_snapshot.as_ref()
    }

    /// Returns retained historical snapshots in newest-first order.
    pub fn snapshot_history(&self) -> &[SnapshotReference] {
        &self.snapshot_history
    }

    /// Returns any reserved extensions.
    pub fn extensions(&self) -> Option<&std::collections::BTreeMap<String, serde_json::Value>> {
        self.extensions.as_ref()
    }
}

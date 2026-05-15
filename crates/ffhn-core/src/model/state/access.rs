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

    /// Returns the persisted baseline state.
    pub const fn baseline(&self) -> &StoredBaseline {
        &self.baseline
    }

    /// Returns the persisted baseline phase.
    pub fn baseline_phase(&self) -> BaselinePhase {
        self.baseline.baseline_phase()
    }

    /// Returns the last persisted live-run summary when one exists.
    pub const fn last_run(&self) -> Option<&LastRunRecord> {
        self.last_run.as_ref()
    }

    /// Returns the current snapshot reference when one exists.
    pub fn current_snapshot(&self) -> Option<&SnapshotReference> {
        match &self.baseline {
            StoredBaseline::Pending => None,
            StoredBaseline::Ready {
                current_snapshot, ..
            } => Some(current_snapshot),
        }
    }

    /// Returns retained historical snapshots in newest-first order.
    pub fn snapshot_history(&self) -> &[SnapshotReference] {
        match &self.baseline {
            StoredBaseline::Pending => &[],
            StoredBaseline::Ready {
                snapshot_history, ..
            } => snapshot_history,
        }
    }

    /// Returns any reserved extensions.
    pub fn extensions(&self) -> Option<&std::collections::BTreeMap<String, serde_json::Value>> {
        self.extensions.as_ref()
    }
}

impl StoredBaseline {
    /// Returns the baseline phase summarized by this stored state.
    pub const fn baseline_phase(&self) -> BaselinePhase {
        match self {
            Self::Pending => BaselinePhase::NeverSucceeded,
            Self::Ready { .. } => BaselinePhase::HasBaseline,
        }
    }
}

impl LastRunRecord {
    pub(crate) const fn new(
        run_at: String,
        outcome: crate::RunOutcome,
        cause: Option<RunFailureCause>,
    ) -> Self {
        Self {
            run_at,
            outcome,
            cause,
        }
    }

    /// Returns the stable run outcome represented by this stored summary.
    pub const fn outcome(&self) -> crate::RunOutcome {
        self.outcome
    }

    /// Returns the persisted live-run timestamp.
    pub fn run_at(&self) -> &str {
        &self.run_at
    }

    /// Returns the run-failure cause when the stored outcome failed.
    pub const fn failure_cause(&self) -> Option<RunFailureCause> {
        self.cause
    }
}

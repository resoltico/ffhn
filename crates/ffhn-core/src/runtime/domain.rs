use crate::{
    BaselinePhase, CoreError, LastRunRecord, STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION,
    SnapshotReference, StateDocument, StoredBaseline, TargetId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PersistedBaselineState {
    Pending,
    Ready {
        current_snapshot: SnapshotReference,
        snapshot_history: Vec<SnapshotReference>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PersistedState {
    pub(crate) monitoring_contract_digest_sha256: String,
    pub(crate) baseline: PersistedBaselineState,
    pub(crate) last_run: Option<LastRunRecord>,
}

impl PersistedState {
    pub(crate) fn from_document(document: StateDocument) -> Self {
        Self {
            monitoring_contract_digest_sha256: document.monitoring_contract_digest_sha256,
            baseline: match document.baseline {
                StoredBaseline::Pending => PersistedBaselineState::Pending,
                StoredBaseline::Ready {
                    current_snapshot,
                    snapshot_history,
                } => PersistedBaselineState::Ready {
                    current_snapshot,
                    snapshot_history,
                },
            },
            last_run: document.last_run,
        }
    }

    pub(crate) fn to_document(&self, target_id: TargetId) -> Result<StateDocument, CoreError> {
        let document = StateDocument {
            schema_name: STATE_SCHEMA_NAME.to_owned(),
            schema_version: STATE_SCHEMA_VERSION,
            target_id,
            monitoring_contract_digest_sha256: self.monitoring_contract_digest_sha256.clone(),
            baseline: match &self.baseline {
                PersistedBaselineState::Pending => StoredBaseline::Pending,
                PersistedBaselineState::Ready {
                    current_snapshot,
                    snapshot_history,
                } => StoredBaseline::Ready {
                    current_snapshot: current_snapshot.clone(),
                    snapshot_history: snapshot_history.clone(),
                },
            },
            last_run: self.last_run.clone(),
            extensions: None,
        };
        document.validate()?;
        Ok(document)
    }

    pub(crate) fn monitoring_contract_digest_sha256(&self) -> &str {
        &self.monitoring_contract_digest_sha256
    }

    pub(crate) const fn baseline_phase(&self) -> BaselinePhase {
        match self.baseline {
            PersistedBaselineState::Pending => BaselinePhase::NeverSucceeded,
            PersistedBaselineState::Ready { .. } => BaselinePhase::HasBaseline,
        }
    }

    pub(crate) fn current_snapshot(&self) -> Option<&SnapshotReference> {
        match &self.baseline {
            PersistedBaselineState::Pending => None,
            PersistedBaselineState::Ready {
                current_snapshot, ..
            } => Some(current_snapshot),
        }
    }

    pub(crate) fn snapshot_history(&self) -> &[SnapshotReference] {
        match &self.baseline {
            PersistedBaselineState::Pending => &[],
            PersistedBaselineState::Ready {
                snapshot_history, ..
            } => snapshot_history,
        }
    }

    pub(crate) fn with_last_run(mut self, last_run: LastRunRecord) -> Self {
        self.last_run = Some(last_run);
        self
    }
}

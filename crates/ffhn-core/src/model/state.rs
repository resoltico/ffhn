use serde::{Deserialize, Serialize};

use crate::CoreError;

use super::schema::{STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION};
use super::validate::{validate_identity, validate_target_id, validate_timestamp};
use super::{Extensions, ReasonCode, RunOutcome, SnapshotReference, SnapshotSlot, StatePhase};

/// Persisted FFHN state schema.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StateDocument {
    /// Frozen schema identity.
    pub schema_name: String,
    /// Frozen schema version.
    pub schema_version: u32,
    /// Target id.
    pub target_id: String,
    /// Current state phase.
    pub state_phase: StatePhase,
    /// Most recent attempted run time.
    pub last_run_at: Option<String>,
    /// Most recent attempted run outcome.
    pub last_run_outcome: Option<RunOutcome>,
    /// Most recent attempted run reason.
    pub last_reason_code: Option<ReasonCode>,
    /// Current snapshot ref.
    pub current_snapshot: Option<SnapshotReference>,
    /// Older retained snapshots, newest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snapshot_history: Vec<SnapshotReference>,
    /// Reserved extensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Extensions,
}

impl StateDocument {
    /// Validates one state document.
    pub fn validate(&self) -> Result<(), CoreError> {
        let schema_name = &self.schema_name;
        let schema_version = self.schema_version;
        validate_identity(
            schema_name,
            STATE_SCHEMA_NAME,
            schema_version,
            STATE_SCHEMA_VERSION,
        )?;
        validate_target_id(&self.target_id)?;
        if let Some(last_run_at) = &self.last_run_at {
            validate_timestamp(last_run_at)?;
        }
        match self.state_phase {
            StatePhase::NeverSucceeded => {
                if self.current_snapshot.is_some() {
                    return Err(CoreError::htmlcut(
                        "state_phase never_succeeded requires null snapshots",
                    ));
                }
                if !self.snapshot_history.is_empty() {
                    return Err(CoreError::htmlcut(
                        "state_phase never_succeeded requires null snapshots",
                    ));
                }
            }
            StatePhase::HasBaseline => {
                if self.current_snapshot.is_none() {
                    return Err(CoreError::htmlcut(
                        "state_phase has_baseline requires current_snapshot",
                    ));
                }
            }
        }
        if let Some(snapshot) = &self.current_snapshot {
            snapshot.validate()?;
            if snapshot.slot != SnapshotSlot::Current {
                return Err(CoreError::htmlcut("current_snapshot.slot must be current"));
            }
        }
        for snapshot in &self.snapshot_history {
            snapshot.validate()?;
            if snapshot.slot != SnapshotSlot::History {
                return Err(CoreError::htmlcut(
                    "snapshot_history entries must use slot = history",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION};

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn snapshot(slot: SnapshotSlot) -> SnapshotReference {
        SnapshotReference {
            slot,
            canonical_text_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            extraction_record_path: format!("snapshots/{}/extraction.json", slot_name(slot)),
            canonical_text_path: format!("snapshots/{}/canonical.txt", slot_name(slot)),
            outer_html_path: format!("snapshots/{}/outer.html", slot_name(slot)),
            captured_at: "2026-04-05T10:15:30Z".to_owned(),
        }
    }

    fn slot_name(slot: SnapshotSlot) -> &'static str {
        match slot {
            SnapshotSlot::Current => "current",
            SnapshotSlot::History => "history",
        }
    }

    #[test]
    fn state_document_validation_accepts_valid_states() {
        StateDocument {
            schema_name: STATE_SCHEMA_NAME.to_owned(),
            schema_version: STATE_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            state_phase: StatePhase::NeverSucceeded,
            last_run_at: Some("2026-04-05T10:15:30Z".to_owned()),
            last_run_outcome: Some(RunOutcome::SkippedDisabled),
            last_reason_code: Some(ReasonCode::Disabled),
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        }
        .validate()
        .expect("never succeeded state");

        StateDocument {
            schema_name: STATE_SCHEMA_NAME.to_owned(),
            schema_version: STATE_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            state_phase: StatePhase::HasBaseline,
            last_run_at: Some("2026-04-05T10:15:30Z".to_owned()),
            last_run_outcome: Some(RunOutcome::Initialized),
            last_reason_code: Some(ReasonCode::Ok),
            current_snapshot: Some(snapshot(SnapshotSlot::Current)),
            snapshot_history: vec![snapshot(SnapshotSlot::History)],
            extensions: None,
        }
        .validate()
        .expect("baseline state");

        StateDocument {
            schema_name: STATE_SCHEMA_NAME.to_owned(),
            schema_version: STATE_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            state_phase: StatePhase::NeverSucceeded,
            last_run_at: None,
            last_run_outcome: None,
            last_reason_code: None,
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        }
        .validate()
        .expect("minimal never succeeded state");
    }

    #[test]
    fn state_document_validation_rejects_invalid_snapshot_invariants() {
        let invalid_identity = StateDocument {
            schema_name: "wrong".to_owned(),
            schema_version: STATE_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            state_phase: StatePhase::NeverSucceeded,
            last_run_at: None,
            last_run_outcome: None,
            last_reason_code: None,
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        };
        assert!(invalid_identity.validate().is_err());

        let mut state = StateDocument {
            schema_name: STATE_SCHEMA_NAME.to_owned(),
            schema_version: STATE_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            state_phase: StatePhase::NeverSucceeded,
            last_run_at: None,
            last_run_outcome: None,
            last_reason_code: None,
            current_snapshot: Some(snapshot(SnapshotSlot::Current)),
            snapshot_history: Vec::new(),
            extensions: None,
        };
        assert!(state.validate().is_err());

        state.state_phase = StatePhase::HasBaseline;
        state.current_snapshot = None;
        assert!(state.validate().is_err());

        state.state_phase = StatePhase::NeverSucceeded;
        state.snapshot_history = vec![snapshot(SnapshotSlot::History)];
        assert!(state.validate().is_err());

        state.state_phase = StatePhase::HasBaseline;
        state.snapshot_history = Vec::new();
        state.current_snapshot = Some(snapshot(SnapshotSlot::History));
        assert!(state.validate().is_err());

        state.current_snapshot = Some(snapshot(SnapshotSlot::Current));
        state.snapshot_history = vec![snapshot(SnapshotSlot::Current)];
        assert!(state.validate().is_err());
    }
}

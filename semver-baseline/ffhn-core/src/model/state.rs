use serde::{Deserialize, Serialize};

use crate::CoreError;

use super::TargetId;
use super::schema::{STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION};
use super::validate::{validate_identity, validate_timestamp};
use super::{Extensions, ReasonCode, RunOutcome, SnapshotReference, SnapshotSlot, StatePhase};

/// Persisted FFHN state schema.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "RawStateDocument")]
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) snapshot_history: Vec<SnapshotReference>,
    /// Reserved extensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) extensions: Extensions,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStateDocument {
    schema_name: String,
    schema_version: u32,
    target_id: String,
    state_phase: StatePhase,
    last_run_at: Option<String>,
    last_run_outcome: Option<RunOutcome>,
    last_reason_code: Option<ReasonCode>,
    current_snapshot: Option<SnapshotReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    snapshot_history: Vec<SnapshotReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
}

impl TryFrom<RawStateDocument> for StateDocument {
    type Error = CoreError;

    fn try_from(raw: RawStateDocument) -> Result<Self, Self::Error> {
        let document = Self {
            schema_name: raw.schema_name,
            schema_version: raw.schema_version,
            target_id: raw.target_id.try_into()?,
            state_phase: raw.state_phase,
            last_run_at: raw.last_run_at,
            last_run_outcome: raw.last_run_outcome,
            last_reason_code: raw.last_reason_code,
            current_snapshot: raw.current_snapshot,
            snapshot_history: raw.snapshot_history,
            extensions: raw.extensions,
        };
        document.validate()?;
        Ok(document)
    }
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
        if let Some(last_run_at) = &self.last_run_at {
            validate_timestamp(last_run_at)?;
        }
        match self.state_phase {
            StatePhase::NeverSucceeded => {
                if self.current_snapshot.is_some() {
                    return Err(CoreError::contract(
                        "state_phase never_succeeded requires null snapshots",
                    ));
                }
                if !self.snapshot_history.is_empty() {
                    return Err(CoreError::contract(
                        "state_phase never_succeeded requires null snapshots",
                    ));
                }
            }
            StatePhase::HasBaseline => {
                if self.current_snapshot.is_none() {
                    return Err(CoreError::contract(
                        "state_phase has_baseline requires current_snapshot",
                    ));
                }
            }
        }
        if let Some(snapshot) = &self.current_snapshot {
            snapshot.validate()?;
            if snapshot.slot != SnapshotSlot::Current {
                return Err(CoreError::contract("current_snapshot.slot must be current"));
            }
        }
        for snapshot in &self.snapshot_history {
            snapshot.validate()?;
            if snapshot.slot != SnapshotSlot::History {
                return Err(CoreError::contract(
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
    use crate::{RelativeArtifactPath, STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION, TargetId};

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn snapshot(slot: SnapshotSlot) -> SnapshotReference {
        SnapshotReference {
            slot,
            canonical_text_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            extraction_record_path: RelativeArtifactPath::new(format!(
                "snapshots/{}/extraction.json",
                slot_name(slot)
            ))
            .expect("relative path"),
            canonical_text_path: RelativeArtifactPath::new(format!(
                "snapshots/{}/canonical.txt",
                slot_name(slot)
            ))
            .expect("relative path"),
            outer_html_path: RelativeArtifactPath::new(format!(
                "snapshots/{}/outer.html",
                slot_name(slot)
            ))
            .expect("relative path"),
            captured_at: "2026-04-05T10:15:30Z".to_owned(),
        }
    }

    fn target_id() -> TargetId {
        TargetId::new("demo").expect("target id")
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
            target_id: target_id(),
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
            target_id: target_id(),
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
            target_id: target_id(),
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
            target_id: target_id(),
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
            target_id: target_id(),
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

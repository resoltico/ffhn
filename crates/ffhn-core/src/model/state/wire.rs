use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Extensions, ReasonCode, RunOutcome, SnapshotReference, StateDocument, StatePhase};
use crate::CoreError;

#[derive(Debug, Serialize, Deserialize)]
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

impl From<&StateDocument> for RawStateDocument {
    fn from(document: &StateDocument) -> Self {
        Self {
            schema_name: document.schema_name.clone(),
            schema_version: document.schema_version,
            target_id: document.target_id.as_str().to_owned(),
            state_phase: document.state_phase,
            last_run_at: document.last_run_at.clone(),
            last_run_outcome: document.last_run_outcome,
            last_reason_code: document.last_reason_code,
            current_snapshot: document.current_snapshot.clone(),
            snapshot_history: document.snapshot_history.clone(),
            extensions: document.extensions.clone(),
        }
    }
}

impl Serialize for StateDocument {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawStateDocument::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StateDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawStateDocument::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::canonical::normalize_line_endings;
use crate::stable_json::sha256_hex;
use crate::{
    CoreError, ExtractionRecord, ProcessErrorDetail, SnapshotDigestSummary, SnapshotReference,
    StateDocument, StatePhase, TargetPaths, TargetStatus,
};

use super::storage::read_text;

#[derive(Clone, Debug)]
pub(crate) struct SnapshotArtifacts {
    pub(crate) reference: SnapshotReference,
    pub(crate) canonical_text: String,
    pub(crate) outer_html: String,
    pub(crate) extraction_json: String,
}

#[derive(Clone, Debug)]
pub(crate) struct LoadedState {
    pub(crate) document: StateDocument,
    pub(crate) current: Option<SnapshotArtifacts>,
}

#[derive(Clone, Debug)]
pub(crate) enum StateLoad {
    Missing,
    Valid(Box<LoadedState>),
    Unreadable(ProcessErrorDetail),
    InvalidDocument {
        phase: Option<StatePhase>,
        detail: ProcessErrorDetail,
    },
    IntegrityMismatch {
        phase: Option<StatePhase>,
        detail: ProcessErrorDetail,
    },
}

pub(crate) fn load_state(paths: &TargetPaths) -> StateLoad {
    let state_path = paths.state_file();
    if !state_path.is_file() {
        return StateLoad::Missing;
    }

    let state_json = match read_text(&state_path) {
        Ok(state_json) => state_json,
        Err(error) => {
            return StateLoad::Unreadable(
                ProcessErrorDetail::from(&error)
                    .with_fallback_path(state_path.display().to_string()),
            );
        }
    };
    let (document, parsed_phase) = match decode_state_document(&state_path, &state_json) {
        Ok(document) => document,
        Err(detail) => {
            return StateLoad::InvalidDocument {
                phase: recover_state_phase(&state_json),
                detail,
            };
        }
    };

    if document.target_id.as_str() != paths.target_id() {
        return StateLoad::InvalidDocument {
            phase: parsed_phase,
            detail: ProcessErrorDetail::new(
                crate::ProcessErrorKind::Contract,
                "state.json target_id does not match its directory name",
                Some(state_path.display().to_string()),
            )
            .expect("state contract detail"),
        };
    }

    let current = if let Some(reference) = &document.current_snapshot {
        match load_snapshot(paths.target_dir(), reference) {
            Ok(snapshot) => snapshot,
            Err(detail) => {
                return StateLoad::IntegrityMismatch {
                    phase: parsed_phase,
                    detail,
                };
            }
        }
    } else {
        None
    };
    for reference in &document.snapshot_history {
        if let Err(detail) = load_snapshot(paths.target_dir(), reference) {
            return StateLoad::IntegrityMismatch {
                phase: parsed_phase,
                detail,
            };
        }
    }

    StateLoad::Valid(Box::new(LoadedState { document, current }))
}

fn decode_state_document(
    state_path: &Path,
    state_json: &str,
) -> Result<(StateDocument, Option<StatePhase>), ProcessErrorDetail> {
    let value =
        decode_json_value(state_json).map_err(|error| detail_with_path(&error, state_path))?;
    let phase = recover_state_phase_value(&value);
    let document = decode_json_contract::<StateDocument>(value)
        .map_err(|error| detail_with_path(&error, state_path))?;
    Ok((document, phase))
}

fn recover_state_phase(state_json: &str) -> Option<StatePhase> {
    let value = serde_json::from_str::<Value>(state_json).ok()?;
    recover_state_phase_value(&value)
}

fn recover_state_phase_value(value: &Value) -> Option<StatePhase> {
    let phase = value.get("state_phase")?.clone();
    serde_json::from_value(phase).ok()
}

fn load_snapshot(
    target_dir: PathBuf,
    reference: &SnapshotReference,
) -> Result<Option<SnapshotArtifacts>, ProcessErrorDetail> {
    let canonical_path = target_dir.join(&reference.canonical_text_path);
    let outer_html_path = target_dir.join(&reference.outer_html_path);
    let extraction_path = target_dir.join(&reference.extraction_record_path);

    let canonical_text =
        read_text(&canonical_path).map_err(|error| detail_with_path(&error, &canonical_path))?;
    let outer_html =
        read_text(&outer_html_path).map_err(|error| detail_with_path(&error, &outer_html_path))?;
    let extraction_json =
        read_text(&extraction_path).map_err(|error| detail_with_path(&error, &extraction_path))?;
    let extraction_record = decode_extraction_record(&extraction_path, &extraction_json)?;

    validate_snapshot_integrity(reference, &canonical_text, &outer_html, &extraction_record)
        .map_err(|error| ProcessErrorDetail::from(&error))?;

    Ok(Some(SnapshotArtifacts {
        reference: reference.clone(),
        canonical_text,
        outer_html,
        extraction_json,
    }))
}

fn decode_extraction_record(
    extraction_path: &Path,
    extraction_json: &str,
) -> Result<ExtractionRecord, ProcessErrorDetail> {
    let value = decode_json_value(extraction_json)
        .map_err(|error| detail_with_path(&error, extraction_path))?;
    decode_json_contract::<ExtractionRecord>(value)
        .map_err(|error| detail_with_path(&error, extraction_path))
}

fn decode_json_value(text: &str) -> Result<Value, CoreError> {
    serde_json::from_str(text).map_err(CoreError::Json)
}

fn decode_json_contract<T: DeserializeOwned>(value: Value) -> Result<T, CoreError> {
    serde_json::from_value(value).map_err(json_contract_error)
}

fn json_contract_error(error: serde_json::Error) -> CoreError {
    let message = error.to_string();
    let message = message
        .strip_prefix("contract error: ")
        .unwrap_or(message.as_str())
        .trim_end()
        .to_owned();
    CoreError::contract(message)
}

fn detail_with_path(error: &CoreError, path: &Path) -> ProcessErrorDetail {
    ProcessErrorDetail::from(error).with_fallback_path(path.display().to_string())
}

fn validate_snapshot_integrity(
    reference: &SnapshotReference,
    canonical_text: &str,
    outer_html: &str,
    extraction_record: &ExtractionRecord,
) -> Result<(), CoreError> {
    let canonical_digest = sha256_hex(normalize_line_endings(canonical_text).as_bytes());
    if canonical_digest != reference.canonical_text_sha256 {
        return Err(CoreError::contract(
            "snapshot artifact digests do not match state",
        ));
    }

    let outer_html_digest = sha256_hex(normalize_line_endings(outer_html).as_bytes());
    if outer_html_digest != reference.outer_html_sha256 {
        return Err(CoreError::contract(
            "snapshot artifact digests do not match state",
        ));
    }

    if extraction_record.outer_html_sha256 != reference.outer_html_sha256 {
        return Err(CoreError::contract(
            "snapshot artifact digests do not match state",
        ));
    }

    Ok(())
}

pub(crate) fn prior_valid_state(state: &StateLoad) -> Option<&LoadedState> {
    match state {
        StateLoad::Valid(state) => Some(state.as_ref()),
        StateLoad::Missing
        | StateLoad::Unreadable(_)
        | StateLoad::InvalidDocument { .. }
        | StateLoad::IntegrityMismatch { .. } => None,
    }
}

pub(crate) fn prior_compare_digest(state: &StateLoad) -> Option<String> {
    prior_valid_state(state)
        .and_then(|state| state.document.current_snapshot.as_ref())
        .map(|snapshot| snapshot.canonical_text_sha256.clone())
}

pub(crate) fn state_phase_or_default(state: &StateLoad) -> StatePhase {
    match state {
        StateLoad::Missing => StatePhase::NeverSucceeded,
        StateLoad::Valid(state) => state.document.state_phase,
        StateLoad::Unreadable(_) => StatePhase::NeverSucceeded,
        StateLoad::InvalidDocument { phase, .. } | StateLoad::IntegrityMismatch { phase, .. } => {
            phase.unwrap_or(StatePhase::NeverSucceeded)
        }
    }
}

pub(crate) fn status_from_state(state: &StateLoad) -> TargetStatus {
    match state {
        StateLoad::Missing => TargetStatus::Pending,
        StateLoad::Valid(state) if state.document.state_phase == StatePhase::HasBaseline => {
            TargetStatus::Ready
        }
        StateLoad::Valid(_) => TargetStatus::Pending,
        StateLoad::Unreadable(_)
        | StateLoad::InvalidDocument { .. }
        | StateLoad::IntegrityMismatch { .. } => TargetStatus::Invalid,
    }
}

pub(crate) fn state_error_detail(state: &StateLoad) -> Option<&ProcessErrorDetail> {
    match state {
        StateLoad::Missing | StateLoad::Valid(_) => None,
        StateLoad::Unreadable(detail)
        | StateLoad::InvalidDocument { detail, .. }
        | StateLoad::IntegrityMismatch { detail, .. } => Some(detail),
    }
}

pub(crate) fn status_from_loaded_state(state: Option<&StateDocument>) -> TargetStatus {
    match state {
        Some(state) if state.state_phase == StatePhase::HasBaseline => TargetStatus::Ready,
        Some(_) => TargetStatus::Pending,
        None => TargetStatus::Pending,
    }
}

pub(crate) fn snapshot_digest_summary(snapshot: &SnapshotReference) -> SnapshotDigestSummary {
    SnapshotDigestSummary {
        canonical_text_sha256: snapshot.canonical_text_sha256.clone(),
        outer_html_sha256: snapshot.outer_html_sha256.clone(),
        captured_at: snapshot.captured_at.clone(),
    }
}

#[cfg(test)]
mod tests;

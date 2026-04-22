use std::path::PathBuf;

use crate::canonical::normalize_line_endings;
use crate::stable_json::sha256_hex;
use crate::{
    CoreError, ExtractionRecord, SnapshotDigestSummary, SnapshotReference, StateDocument,
    StatePhase, TargetPaths, TargetStatus,
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
    Unreadable,
    InvalidSchema(Option<StatePhase>),
    IntegrityMismatch(Option<StatePhase>),
}

pub(crate) fn load_state(paths: &TargetPaths) -> StateLoad {
    let state_path = paths.state_file();
    if !state_path.is_file() {
        return StateLoad::Missing;
    }

    let state_json = match read_text(&state_path) {
        Ok(state_json) => state_json,
        Err(_) => return StateLoad::Unreadable,
    };
    let document = match serde_json::from_str::<StateDocument>(&state_json) {
        Ok(document) => document,
        Err(_) => return StateLoad::InvalidSchema(None),
    };
    let parsed_phase = Some(document.state_phase);

    if document.target_id != paths.target_id() {
        return StateLoad::InvalidSchema(parsed_phase);
    }
    if document.validate().is_err() {
        return StateLoad::InvalidSchema(parsed_phase);
    }

    let current = if let Some(reference) = &document.current_snapshot {
        match load_snapshot(paths.target_dir(), reference) {
            Ok(snapshot) => snapshot,
            Err(_) => return StateLoad::IntegrityMismatch(parsed_phase),
        }
    } else {
        None
    };
    for reference in &document.snapshot_history {
        if load_snapshot(paths.target_dir(), reference).is_err() {
            return StateLoad::IntegrityMismatch(parsed_phase);
        }
    }

    StateLoad::Valid(Box::new(LoadedState { document, current }))
}

fn load_snapshot(
    target_dir: PathBuf,
    reference: &SnapshotReference,
) -> Result<Option<SnapshotArtifacts>, CoreError> {
    let canonical_path = target_dir.join(&reference.canonical_text_path);
    let outer_html_path = target_dir.join(&reference.outer_html_path);
    let extraction_path = target_dir.join(&reference.extraction_record_path);

    let canonical_text = read_text(&canonical_path)?;
    let outer_html = read_text(&outer_html_path)?;
    let extraction_json = read_text(&extraction_path)?;
    let extraction_record = serde_json::from_str::<ExtractionRecord>(&extraction_json)?;
    extraction_record.validate()?;

    validate_snapshot_integrity(reference, &canonical_text, &outer_html, &extraction_record)?;

    Ok(Some(SnapshotArtifacts {
        reference: reference.clone(),
        canonical_text,
        outer_html,
        extraction_json,
    }))
}

fn validate_snapshot_integrity(
    reference: &SnapshotReference,
    canonical_text: &str,
    outer_html: &str,
    extraction_record: &ExtractionRecord,
) -> Result<(), CoreError> {
    let canonical_digest = sha256_hex(normalize_line_endings(canonical_text).as_bytes());
    if canonical_digest != reference.canonical_text_sha256 {
        return Err(CoreError::htmlcut(
            "snapshot artifact digests do not match state",
        ));
    }

    let outer_html_digest = sha256_hex(normalize_line_endings(outer_html).as_bytes());
    if outer_html_digest != reference.outer_html_sha256 {
        return Err(CoreError::htmlcut(
            "snapshot artifact digests do not match state",
        ));
    }

    if extraction_record.outer_html_sha256 != reference.outer_html_sha256 {
        return Err(CoreError::htmlcut(
            "snapshot artifact digests do not match state",
        ));
    }

    Ok(())
}

pub(crate) fn prior_valid_state(state: &StateLoad) -> Option<&LoadedState> {
    match state {
        StateLoad::Valid(state) => Some(state.as_ref()),
        StateLoad::Missing
        | StateLoad::Unreadable
        | StateLoad::InvalidSchema(_)
        | StateLoad::IntegrityMismatch(_) => None,
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
        StateLoad::Unreadable => StatePhase::NeverSucceeded,
        StateLoad::InvalidSchema(phase) | StateLoad::IntegrityMismatch(phase) => {
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
        StateLoad::Unreadable | StateLoad::InvalidSchema(_) | StateLoad::IntegrityMismatch(_) => {
            TargetStatus::Invalid
        }
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

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
mod tests {
    use super::super::storage::{write_exact_text, write_json};
    use super::*;
    use crate::{
        EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION, HTMLCUT_INTEROP_PROFILE,
        OutputKind, ReasonCode, RunOutcome, SelectionKind, SelectionMatch, SnapshotSlot,
    };
    use serde_json::json;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn snapshot(slot: SnapshotSlot, canonical_text: &str, outer_html: &str) -> SnapshotReference {
        SnapshotReference {
            slot,
            canonical_text_sha256: sha256_hex(normalize_line_endings(canonical_text).as_bytes()),
            outer_html_sha256: sha256_hex(normalize_line_endings(outer_html).as_bytes()),
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

    fn extraction_record(outer_html_sha256: &str) -> ExtractionRecord {
        ExtractionRecord {
            schema_name: EXTRACTION_RECORD_SCHEMA_NAME.to_owned(),
            schema_version: EXTRACTION_RECORD_SCHEMA_VERSION,
            interop_profile: HTMLCUT_INTEROP_PROFILE.to_owned(),
            htmlcut_plan_digest_sha256: DIGEST.to_owned(),
            htmlcut_result_digest_sha256: DIGEST.to_owned(),
            comparison_input_sha256: DIGEST.to_owned(),
            outer_html_sha256: outer_html_sha256.to_owned(),
            strategy_kind: SelectionKind::CssSelector,
            selection_mode: SelectionMatch::Single,
            output_kind: OutputKind::OuterHtml,
            candidate_count: 1,
            selected_candidate_index: 1,
            match_metadata: json!({"selector": "main"}),
            warning_codes: Vec::new(),
            created_at: "2026-04-05T10:15:30Z".to_owned(),
            extensions: None,
        }
    }

    fn write_snapshot(
        paths: &TargetPaths,
        reference: &SnapshotReference,
        canonical: &str,
        outer: &str,
    ) {
        write_exact_text(
            paths.target_dir().join(&reference.canonical_text_path),
            &normalize_line_endings(canonical),
        )
        .expect("write canonical");
        write_exact_text(
            paths.target_dir().join(&reference.outer_html_path),
            &normalize_line_endings(outer),
        )
        .expect("write outer html");
        write_json(
            paths.target_dir().join(&reference.extraction_record_path),
            &extraction_record(&reference.outer_html_sha256),
        )
        .expect("write extraction record");
    }

    fn state_document(
        current: Option<SnapshotReference>,
        history: Vec<SnapshotReference>,
    ) -> StateDocument {
        StateDocument {
            schema_name: crate::STATE_SCHEMA_NAME.to_owned(),
            schema_version: crate::STATE_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            state_phase: if current.is_some() {
                StatePhase::HasBaseline
            } else {
                StatePhase::NeverSucceeded
            },
            last_run_at: Some("2026-04-05T10:15:30Z".to_owned()),
            last_run_outcome: Some(RunOutcome::Changed),
            last_reason_code: Some(ReasonCode::Ok),
            current_snapshot: current,
            snapshot_history: history,
            extensions: None,
        }
    }

    #[test]
    fn load_state_covers_missing_valid_invalid_and_integrity_mismatch_cases() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");

        assert!(matches!(load_state(&paths), StateLoad::Missing));
        assert_eq!(
            state_phase_or_default(&StateLoad::Missing),
            StatePhase::NeverSucceeded
        );
        assert_eq!(
            status_from_state(&StateLoad::Missing),
            TargetStatus::Pending
        );
        assert_eq!(status_from_loaded_state(None), TargetStatus::Pending);
        assert!(prior_valid_state(&StateLoad::Missing).is_none());
        assert!(prior_compare_digest(&StateLoad::Missing).is_none());

        let current = snapshot(SnapshotSlot::Current, "hello", "<main>Hello</main>");
        let previous = snapshot(SnapshotSlot::History, "before", "<main>Before</main>");
        write_snapshot(&paths, &current, "hello", "<main>Hello</main>");
        write_snapshot(&paths, &previous, "before", "<main>Before</main>");
        write_json(
            paths.state_file(),
            &state_document(Some(current.clone()), vec![previous.clone()]),
        )
        .expect("write valid state");

        let loaded_state = load_state(&paths);
        assert!(matches!(loaded_state, StateLoad::Valid(_)));
        let loaded = prior_valid_state(&loaded_state)
            .expect("prior valid state")
            .to_owned();
        assert_eq!(
            loaded
                .current
                .as_ref()
                .expect("current")
                .reference
                .canonical_text_sha256,
            current.canonical_text_sha256
        );
        assert_eq!(
            prior_compare_digest(&StateLoad::Valid(Box::new(loaded.clone()))),
            Some(current.canonical_text_sha256.clone())
        );
        assert_eq!(
            state_phase_or_default(&StateLoad::Valid(Box::new(loaded.clone()))),
            StatePhase::HasBaseline
        );
        assert_eq!(
            status_from_state(&StateLoad::Valid(Box::new(loaded.clone()))),
            TargetStatus::Ready
        );
        assert_eq!(
            status_from_loaded_state(Some(&loaded.document)),
            TargetStatus::Ready
        );
        assert_eq!(
            snapshot_digest_summary(&current),
            SnapshotDigestSummary {
                canonical_text_sha256: current.canonical_text_sha256.clone(),
                outer_html_sha256: current.outer_html_sha256.clone(),
                captured_at: current.captured_at.clone(),
            }
        );

        write_json(paths.state_file(), &state_document(None, Vec::new()))
            .expect("write pending state");
        let pending = load_state(&paths);
        assert!(matches!(pending, StateLoad::Valid(_)));
        assert_eq!(status_from_state(&pending), TargetStatus::Pending);
        assert_eq!(
            status_from_loaded_state(prior_valid_state(&pending).map(|state| &state.document)),
            TargetStatus::Pending
        );

        write_json(
            paths.state_file(),
            &StateDocument {
                schema_name: "wrong".to_owned(),
                ..state_document(Some(current.clone()), vec![previous.clone()])
            },
        )
        .expect("write invalid schema state");
        assert!(matches!(
            load_state(&paths),
            StateLoad::InvalidSchema(Some(StatePhase::HasBaseline))
        ));
        assert_eq!(
            state_phase_or_default(&StateLoad::InvalidSchema(None)),
            StatePhase::NeverSucceeded
        );

        write_exact_text(paths.state_file(), "{not json").expect("write malformed state");
        assert!(matches!(load_state(&paths), StateLoad::InvalidSchema(None)));

        #[cfg(unix)]
        {
            write_json(
                paths.state_file(),
                &state_document(Some(current.clone()), vec![]),
            )
            .expect("write unreadable state");
            let metadata = std::fs::metadata(paths.state_file()).expect("state metadata");
            let original = metadata.permissions();
            let mut denied = original.clone();
            denied.set_mode(0o000);
            std::fs::set_permissions(paths.state_file(), denied).expect("deny state permissions");
            let unreadable = load_state(&paths);
            std::fs::set_permissions(paths.state_file(), original)
                .expect("restore state permissions");
            assert!(matches!(unreadable, StateLoad::Unreadable));
            assert_eq!(
                state_phase_or_default(&unreadable),
                StatePhase::NeverSucceeded
            );
            assert_eq!(status_from_state(&unreadable), TargetStatus::Invalid);
        }

        write_json(
            paths.state_file(),
            &StateDocument {
                target_id: "other".to_owned(),
                ..state_document(Some(current.clone()), vec![previous.clone()])
            },
        )
        .expect("write mismatched target state");
        assert!(matches!(
            load_state(&paths),
            StateLoad::InvalidSchema(Some(StatePhase::HasBaseline))
        ));

        write_json(
            paths.state_file(),
            &state_document(Some(current.clone()), vec![previous.clone()]),
        )
        .expect("rewrite valid state");
        write_exact_text(
            paths.target_dir().join(&current.canonical_text_path),
            "tampered",
        )
        .expect("tamper canonical");
        assert!(matches!(
            load_state(&paths),
            StateLoad::IntegrityMismatch(Some(StatePhase::HasBaseline))
        ));

        write_snapshot(&paths, &current, "hello", "<main>Hello</main>");
        write_exact_text(
            paths.target_dir().join(&current.outer_html_path),
            "<main>Tampered</main>",
        )
        .expect("tamper outer html");
        assert!(matches!(
            load_state(&paths),
            StateLoad::IntegrityMismatch(Some(StatePhase::HasBaseline))
        ));
        assert_eq!(
            state_phase_or_default(&StateLoad::IntegrityMismatch(Some(StatePhase::HasBaseline))),
            StatePhase::HasBaseline
        );
        assert_eq!(
            status_from_state(&StateLoad::IntegrityMismatch(Some(StatePhase::HasBaseline))),
            TargetStatus::Invalid
        );

        write_snapshot(&paths, &current, "hello", "<main>Hello</main>");
        write_exact_text(
            paths.target_dir().join(&previous.outer_html_path),
            "<main>Tampered</main>",
        )
        .expect("tamper previous outer html");
        assert!(matches!(
            load_state(&paths),
            StateLoad::IntegrityMismatch(Some(StatePhase::HasBaseline))
        ));

        write_snapshot(&paths, &current, "hello", "<main>Hello</main>");
        write_snapshot(&paths, &previous, "before", "<main>Before</main>");
        write_json(
            paths.target_dir().join(&current.extraction_record_path),
            &extraction_record(DIGEST),
        )
        .expect("write mismatched extraction record");
        assert!(matches!(
            load_state(&paths),
            StateLoad::IntegrityMismatch(Some(StatePhase::HasBaseline))
        ));
    }

    #[test]
    fn load_state_rejects_missing_history_snapshot_artifacts() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        let current = snapshot(SnapshotSlot::Current, "hello", "<main>Hello</main>");
        let history = snapshot(SnapshotSlot::History, "before", "<main>Before</main>");
        write_snapshot(&paths, &current, "hello", "<main>Hello</main>");
        write_json(
            paths.state_file(),
            &state_document(Some(current), vec![history]),
        )
        .expect("write state");

        let loaded = load_state(&paths);
        assert!(matches!(
            loaded,
            StateLoad::IntegrityMismatch(Some(StatePhase::HasBaseline))
        ));
    }
}

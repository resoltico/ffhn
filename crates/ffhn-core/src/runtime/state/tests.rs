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

    write_json(paths.state_file(), &state_document(None, Vec::new())).expect("write pending state");
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
        std::fs::set_permissions(paths.state_file(), original).expect("restore state permissions");
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

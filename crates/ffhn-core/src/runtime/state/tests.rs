use super::super::storage::{write_exact_text, write_json};
use super::*;
use crate::{
    CoreError, EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION,
    HTMLCUT_INTEROP_PROFILE, OutputKind, ProcessErrorDetail, ProcessErrorKind, ReasonCode,
    RelativeArtifactPath, RunOutcome, SelectionKind, SelectionMatch, SnapshotSlot, TargetId,
};
use serde_json::json;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn target_id(value: &str) -> TargetId {
    TargetId::new(value).expect("target id")
}

fn artifact_path(path: impl Into<String>) -> RelativeArtifactPath {
    RelativeArtifactPath::new(path).expect("relative artifact path")
}

fn snapshot(slot: SnapshotSlot, canonical_text: &str, outer_html: &str) -> SnapshotReference {
    SnapshotReference {
        slot,
        canonical_text_sha256: sha256_hex(normalize_line_endings(canonical_text).as_bytes()),
        outer_html_sha256: sha256_hex(normalize_line_endings(outer_html).as_bytes()),
        extraction_record_path: artifact_path(format!(
            "snapshots/{}/extraction.json",
            slot_name(slot)
        )),
        canonical_text_path: artifact_path(format!("snapshots/{}/canonical.txt", slot_name(slot))),
        outer_html_path: artifact_path(format!("snapshots/{}/outer.html", slot_name(slot))),
        captured_at: "2026-04-05T10:15:30Z".to_owned(),
    }
}

fn slot_name(slot: SnapshotSlot) -> &'static str {
    match slot {
        SnapshotSlot::Current => "current",
        SnapshotSlot::History => "history",
    }
}

fn test_state_detail() -> ProcessErrorDetail {
    ProcessErrorDetail::from(&CoreError::contract("state test detail"))
        .with_fallback_path("demo/state.json".to_owned())
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
        target_id: target_id("demo"),
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
        StateLoad::InvalidDocument {
            phase: Some(StatePhase::HasBaseline),
            ..
        }
    ));
    assert_eq!(
        state_phase_or_default(&StateLoad::InvalidDocument {
            phase: None,
            detail: test_state_detail(),
        }),
        StatePhase::NeverSucceeded
    );

    write_exact_text(paths.state_file(), "{not json").expect("write malformed state");
    assert!(matches!(
        load_state(&paths),
        StateLoad::InvalidDocument { phase: None, .. }
    ));

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
        assert!(matches!(unreadable, StateLoad::Unreadable(_)));
        assert_eq!(
            state_phase_or_default(&unreadable),
            StatePhase::NeverSucceeded
        );
        assert_eq!(status_from_state(&unreadable), TargetStatus::Invalid);
    }

    write_json(
        paths.state_file(),
        &StateDocument {
            target_id: target_id("other"),
            ..state_document(Some(current.clone()), vec![previous.clone()])
        },
    )
    .expect("write mismatched target state");
    assert!(matches!(
        load_state(&paths),
        StateLoad::InvalidDocument {
            phase: Some(StatePhase::HasBaseline),
            ..
        }
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
        StateLoad::IntegrityMismatch {
            phase: Some(StatePhase::HasBaseline),
            ..
        }
    ));

    write_snapshot(&paths, &current, "hello", "<main>Hello</main>");
    write_exact_text(
        paths.target_dir().join(&current.outer_html_path),
        "<main>Tampered</main>",
    )
    .expect("tamper outer html");
    assert!(matches!(
        load_state(&paths),
        StateLoad::IntegrityMismatch {
            phase: Some(StatePhase::HasBaseline),
            ..
        }
    ));
    assert_eq!(
        state_phase_or_default(&StateLoad::IntegrityMismatch {
            phase: Some(StatePhase::HasBaseline),
            detail: test_state_detail(),
        }),
        StatePhase::HasBaseline
    );
    assert_eq!(
        status_from_state(&StateLoad::IntegrityMismatch {
            phase: Some(StatePhase::HasBaseline),
            detail: test_state_detail(),
        }),
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
        StateLoad::IntegrityMismatch {
            phase: Some(StatePhase::HasBaseline),
            ..
        }
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
        StateLoad::IntegrityMismatch {
            phase: Some(StatePhase::HasBaseline),
            ..
        }
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
        StateLoad::IntegrityMismatch {
            phase: Some(StatePhase::HasBaseline),
            ..
        }
    ));
}

#[test]
fn load_state_classifies_contract_invalid_state_json_as_invalid_document() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");

    write_exact_text(
        paths.state_file(),
        r#"{
  "schema_name": "ffhn.state",
  "schema_version": 1,
  "target_id": "demo",
  "state_phase": "has_baseline",
  "last_run_at": "2026-04-05T10:15:30Z",
  "last_run_outcome": "changed",
  "last_reason_code": "ok",
  "current_snapshot": null,
  "snapshot_history": []
}"#,
    )
    .expect("write contract-invalid state");

    let loaded = load_state(&paths);
    let StateLoad::InvalidDocument {
        phase: Some(StatePhase::HasBaseline),
        detail,
    } = loaded
    else {
        panic!("expected InvalidDocument with recovered phase");
    };
    let state_path = paths.state_file().display().to_string();
    assert_eq!(detail.kind(), ProcessErrorKind::Contract);
    assert_eq!(
        detail.message(),
        "state_phase has_baseline requires current_snapshot"
    );
    assert_eq!(detail.path(), Some(state_path.as_str()));
}

#[test]
fn load_state_classifies_contract_invalid_extraction_records_with_their_path() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let current = snapshot(SnapshotSlot::Current, "hello", "<main>Hello</main>");

    write_snapshot(&paths, &current, "hello", "<main>Hello</main>");
    write_json(
        paths.state_file(),
        &state_document(Some(current.clone()), Vec::new()),
    )
    .expect("write valid state");
    write_json(
        paths.target_dir().join(&current.extraction_record_path),
        &ExtractionRecord {
            interop_profile: "htmlcut-v0".to_owned(),
            ..extraction_record(&current.outer_html_sha256)
        },
    )
    .expect("write contract-invalid extraction record");

    let loaded = load_state(&paths);
    let StateLoad::IntegrityMismatch {
        phase: Some(StatePhase::HasBaseline),
        detail,
    } = loaded
    else {
        panic!("expected IntegrityMismatch with recovered phase");
    };
    let extraction_path = paths
        .target_dir()
        .join(&current.extraction_record_path)
        .display()
        .to_string();
    assert_eq!(detail.kind(), ProcessErrorKind::Contract);
    assert_eq!(
        detail.message(),
        "ffhn.extraction_record interop_profile must match the FFHN HTMLCut profile"
    );
    assert_eq!(detail.path(), Some(extraction_path.as_str()));
}

#[test]
fn state_error_detail_reflects_the_richer_state_load_variants() {
    assert!(state_error_detail(&StateLoad::Missing).is_none());

    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    write_json(paths.state_file(), &state_document(None, Vec::new())).expect("write state");
    let valid = load_state(&paths);
    assert!(matches!(valid, StateLoad::Valid(_)));
    assert!(state_error_detail(&valid).is_none());

    let detail = test_state_detail();
    assert_eq!(
        state_error_detail(&StateLoad::Unreadable(detail.clone()))
            .expect("unreadable detail")
            .message(),
        detail.message()
    );
    assert_eq!(
        state_error_detail(&StateLoad::InvalidDocument {
            phase: None,
            detail: detail.clone(),
        })
        .expect("invalid-schema detail")
        .message(),
        detail.message()
    );
    assert_eq!(
        state_error_detail(&StateLoad::IntegrityMismatch {
            phase: Some(StatePhase::HasBaseline),
            detail: detail.clone(),
        })
        .expect("integrity detail")
        .message(),
        detail.message()
    );
}

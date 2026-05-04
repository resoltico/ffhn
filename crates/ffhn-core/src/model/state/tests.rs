use super::*;
use crate::{RelativeArtifactPath, STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION, TargetId};
use serde_json::json;
use std::collections::BTreeMap;

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
fn state_document_accessors_expose_the_public_contract() {
    let state = StateDocument {
        schema_name: STATE_SCHEMA_NAME.to_owned(),
        schema_version: STATE_SCHEMA_VERSION,
        target_id: target_id(),
        state_phase: StatePhase::HasBaseline,
        last_run_at: Some("2026-04-05T10:15:30Z".to_owned()),
        last_run_outcome: Some(RunOutcome::Changed),
        last_reason_code: Some(ReasonCode::Ok),
        current_snapshot: Some(snapshot(SnapshotSlot::Current)),
        snapshot_history: vec![snapshot(SnapshotSlot::History)],
        extensions: Some(BTreeMap::from([(
            "demo".to_owned(),
            json!({"kind": "ext"}),
        )])),
    };

    assert_eq!(state.schema_name(), STATE_SCHEMA_NAME);
    assert_eq!(state.schema_version(), STATE_SCHEMA_VERSION);
    assert_eq!(state.target_id(), "demo");
    assert_eq!(state.state_phase(), StatePhase::HasBaseline);
    assert_eq!(state.last_run_at(), Some("2026-04-05T10:15:30Z"));
    assert_eq!(state.last_run_outcome(), Some(RunOutcome::Changed));
    assert_eq!(state.last_reason_code(), Some(ReasonCode::Ok));
    assert_eq!(
        state.current_snapshot().expect("current").slot(),
        SnapshotSlot::Current
    );
    assert_eq!(state.snapshot_history().len(), 1);
    assert_eq!(
        state.extensions().expect("extensions").get("demo"),
        Some(&json!({"kind": "ext"}))
    );
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

#[test]
fn state_document_validation_rejects_partial_last_run_and_unordered_history() {
    let mut state = StateDocument {
        schema_name: STATE_SCHEMA_NAME.to_owned(),
        schema_version: STATE_SCHEMA_VERSION,
        target_id: target_id(),
        state_phase: StatePhase::NeverSucceeded,
        last_run_at: Some("2026-04-05T10:15:30Z".to_owned()),
        last_run_outcome: None,
        last_reason_code: None,
        current_snapshot: None,
        snapshot_history: Vec::new(),
        extensions: None,
    };
    assert!(state.validate().is_err());

    state.last_run_outcome = Some(RunOutcome::FailedTransient);
    state.last_reason_code = Some(ReasonCode::FetchSourceError);
    assert!(state.validate().is_err());

    state = StateDocument {
        schema_name: STATE_SCHEMA_NAME.to_owned(),
        schema_version: STATE_SCHEMA_VERSION,
        target_id: target_id(),
        state_phase: StatePhase::HasBaseline,
        last_run_at: Some("2026-04-05T10:15:30Z".to_owned()),
        last_run_outcome: Some(RunOutcome::Changed),
        last_reason_code: Some(ReasonCode::Ok),
        current_snapshot: Some(snapshot(SnapshotSlot::Current)),
        snapshot_history: vec![
            SnapshotReference {
                captured_at: "2026-04-05T10:15:31Z".to_owned(),
                ..snapshot(SnapshotSlot::History)
            },
            SnapshotReference {
                captured_at: "2026-04-05T10:15:29Z".to_owned(),
                ..snapshot(SnapshotSlot::History)
            },
        ],
        extensions: None,
    };
    assert!(state.validate().is_err());
}

#[test]
fn state_document_validation_rejects_reason_mismatches_and_history_reordering() {
    let invalid_success = StateDocument {
        schema_name: STATE_SCHEMA_NAME.to_owned(),
        schema_version: STATE_SCHEMA_VERSION,
        target_id: target_id(),
        state_phase: StatePhase::NeverSucceeded,
        last_run_at: Some("2026-04-05T10:15:30Z".to_owned()),
        last_run_outcome: Some(RunOutcome::Initialized),
        last_reason_code: Some(ReasonCode::Disabled),
        current_snapshot: None,
        snapshot_history: Vec::new(),
        extensions: None,
    };
    assert!(invalid_success.validate().is_err());

    let invalid_skipped = StateDocument {
        last_run_outcome: Some(RunOutcome::SkippedDisabled),
        last_reason_code: Some(ReasonCode::Ok),
        ..invalid_success.clone()
    };
    assert!(invalid_skipped.validate().is_err());

    let invalid_permanent = StateDocument {
        state_phase: StatePhase::HasBaseline,
        last_run_outcome: Some(RunOutcome::FailedPermanent),
        last_reason_code: Some(ReasonCode::FetchTimeout),
        current_snapshot: Some(SnapshotReference {
            captured_at: "2026-04-05T10:16:00Z".to_owned(),
            ..snapshot(SnapshotSlot::Current)
        }),
        snapshot_history: vec![
            SnapshotReference {
                captured_at: "2026-04-05T10:14:00Z".to_owned(),
                ..snapshot(SnapshotSlot::History)
            },
            SnapshotReference {
                captured_at: "2026-04-05T10:15:00Z".to_owned(),
                ..snapshot(SnapshotSlot::History)
            },
        ],
        ..invalid_success
    };
    assert!(invalid_permanent.validate().is_err());

    let unordered_history = StateDocument {
        state_phase: StatePhase::HasBaseline,
        last_run_outcome: Some(RunOutcome::Changed),
        last_reason_code: Some(ReasonCode::Ok),
        current_snapshot: Some(SnapshotReference {
            captured_at: "2026-04-05T10:16:00Z".to_owned(),
            ..snapshot(SnapshotSlot::Current)
        }),
        snapshot_history: vec![
            SnapshotReference {
                captured_at: "2026-04-05T10:14:00Z".to_owned(),
                ..snapshot(SnapshotSlot::History)
            },
            SnapshotReference {
                captured_at: "2026-04-05T10:15:00Z".to_owned(),
                ..snapshot(SnapshotSlot::History)
            },
        ],
        ..StateDocument {
            schema_name: STATE_SCHEMA_NAME.to_owned(),
            schema_version: STATE_SCHEMA_VERSION,
            target_id: target_id(),
            state_phase: StatePhase::NeverSucceeded,
            last_run_at: Some("2026-04-05T10:15:30Z".to_owned()),
            last_run_outcome: Some(RunOutcome::Changed),
            last_reason_code: Some(ReasonCode::Ok),
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        }
    };
    assert!(unordered_history.validate().is_err());
}

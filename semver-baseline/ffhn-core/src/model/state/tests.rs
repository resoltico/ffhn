use super::*;
use crate::{
    BaselinePhase, LastRunRecord, RelativeArtifactPath, RunFailureCause, RunOutcome,
    STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION, TargetId,
};
use serde_json::json;
use std::collections::BTreeMap;

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn snapshot(slot: SnapshotSlot, captured_at: &str) -> SnapshotReference {
    SnapshotReference {
        slot,
        compare_digest_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        extraction_record_path: RelativeArtifactPath::new(format!(
            "snapshots/{}/extraction.json",
            slot_name(slot)
        ))
        .expect("relative path"),
        compare_path: RelativeArtifactPath::new(format!(
            "snapshots/{}/compare.txt",
            slot_name(slot)
        ))
        .expect("relative path"),
        outer_html_path: RelativeArtifactPath::new(format!(
            "snapshots/{}/outer.html",
            slot_name(slot)
        ))
        .expect("relative path"),
        captured_at: captured_at.to_owned(),
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

fn pending_state() -> StateDocument {
    StateDocument {
        schema_name: STATE_SCHEMA_NAME.to_owned(),
        schema_version: STATE_SCHEMA_VERSION,
        target_id: target_id(),
        monitoring_contract_digest_sha256: DIGEST.to_owned(),
        baseline: StoredBaseline::Pending,
        last_run: None,
        extensions: None,
    }
}

fn last_run(outcome: RunOutcome, cause: Option<RunFailureCause>) -> LastRunRecord {
    LastRunRecord::new("2026-04-05T10:15:30Z".to_owned(), outcome, cause)
}

fn ready_state() -> StateDocument {
    StateDocument {
        baseline: StoredBaseline::Ready {
            current_snapshot: snapshot(SnapshotSlot::Current, "2026-04-05T10:15:30Z"),
            snapshot_history: vec![snapshot(SnapshotSlot::History, "2026-04-05T10:14:30Z")],
        },
        last_run: Some(last_run(RunOutcome::Changed, None)),
        ..pending_state()
    }
}

#[test]
fn state_document_validation_accepts_valid_states() {
    StateDocument {
        last_run: Some(last_run(RunOutcome::SkippedDisabled, None)),
        ..pending_state()
    }
    .validate()
    .expect("never succeeded state");

    StateDocument {
        last_run: Some(last_run(RunOutcome::Initialized, None)),
        ..ready_state()
    }
    .validate()
    .expect("baseline state");

    pending_state()
        .validate()
        .expect("minimal never succeeded state");
}

#[test]
fn state_document_accessors_expose_the_public_contract() {
    let mut state = ready_state();
    state.extensions = Some(BTreeMap::from([(
        "demo".to_owned(),
        json!({"kind": "ext"}),
    )]));

    assert_eq!(state.schema_name(), STATE_SCHEMA_NAME);
    assert_eq!(state.schema_version(), STATE_SCHEMA_VERSION);
    assert_eq!(state.target_id(), "demo");
    assert_eq!(state.monitoring_contract_digest_sha256(), DIGEST);
    assert!(matches!(state.baseline(), StoredBaseline::Ready { .. }));
    assert_eq!(state.baseline_phase(), BaselinePhase::HasBaseline);
    let last_run = state.last_run().expect("last run");
    assert_eq!(last_run.outcome(), crate::RunOutcome::Changed);
    assert_eq!(last_run.run_at(), "2026-04-05T10:15:30Z");
    assert_eq!(last_run.failure_cause(), None);
    assert_eq!(
        state.current_snapshot().expect("current").slot(),
        SnapshotSlot::Current
    );
    assert_eq!(state.snapshot_history().len(), 1);
    assert_eq!(
        state.extensions().expect("extensions").get("demo"),
        Some(&json!({"kind": "ext"}))
    );

    let pending = pending_state();
    assert_eq!(pending.monitoring_contract_digest_sha256(), DIGEST);
    assert!(matches!(pending.baseline(), StoredBaseline::Pending));
    assert_eq!(pending.baseline_phase(), BaselinePhase::NeverSucceeded);
    assert!(pending.last_run().is_none());
    assert!(pending.current_snapshot().is_none());
    assert!(pending.snapshot_history().is_empty());

    let failed = StateDocument {
        last_run: Some(LastRunRecord::new(
            "2026-04-05T10:15:31Z".to_owned(),
            RunOutcome::FailedPermanent,
            Some(RunFailureCause::CompareError),
        )),
        ..ready_state()
    };
    let failed_last_run = failed.last_run().expect("failed last run");
    assert_eq!(
        failed_last_run.outcome(),
        crate::RunOutcome::FailedPermanent
    );
    assert_eq!(
        failed_last_run.failure_cause(),
        Some(RunFailureCause::CompareError)
    );

    let initialized = LastRunRecord::new(
        "2026-04-05T10:15:32Z".to_owned(),
        RunOutcome::Initialized,
        None,
    );
    assert_eq!(initialized.outcome(), crate::RunOutcome::Initialized);
    assert!(initialized.failure_cause().is_none());

    let unchanged = LastRunRecord::new(
        "2026-04-05T10:15:33Z".to_owned(),
        RunOutcome::Unchanged,
        None,
    );
    assert_eq!(unchanged.outcome(), crate::RunOutcome::Unchanged);
    assert!(unchanged.failure_cause().is_none());

    let skipped = LastRunRecord::new(
        "2026-04-05T10:15:34Z".to_owned(),
        RunOutcome::SkippedDisabled,
        None,
    );
    assert_eq!(skipped.outcome(), crate::RunOutcome::SkippedDisabled);
    assert!(skipped.failure_cause().is_none());

    let transient = LastRunRecord::new(
        "2026-04-05T10:15:35Z".to_owned(),
        RunOutcome::FailedTransient,
        Some(RunFailureCause::FetchTimeout),
    );
    assert_eq!(transient.outcome(), crate::RunOutcome::FailedTransient);
    assert_eq!(
        transient.failure_cause(),
        Some(RunFailureCause::FetchTimeout)
    );
}

#[test]
fn state_document_validation_rejects_invalid_snapshot_invariants() {
    let invalid_identity = StateDocument {
        schema_name: "wrong".to_owned(),
        ..pending_state()
    };
    assert!(invalid_identity.validate().is_err());

    let invalid_pending = StateDocument {
        baseline: StoredBaseline::Ready {
            current_snapshot: snapshot(SnapshotSlot::Current, "2026-04-05T10:15:30Z"),
            snapshot_history: Vec::new(),
        },
        ..pending_state()
    };
    assert!(invalid_pending.validate().is_err());

    let invalid_ready = StateDocument {
        baseline: StoredBaseline::Pending,
        ..ready_state()
    };
    assert!(invalid_ready.validate().is_err());

    let wrong_current_slot = StateDocument {
        baseline: StoredBaseline::Ready {
            current_snapshot: snapshot(SnapshotSlot::History, "2026-04-05T10:15:30Z"),
            snapshot_history: Vec::new(),
        },
        ..pending_state()
    };
    assert!(wrong_current_slot.validate().is_err());

    let wrong_history_slot = StateDocument {
        baseline: StoredBaseline::Ready {
            current_snapshot: snapshot(SnapshotSlot::Current, "2026-04-05T10:15:30Z"),
            snapshot_history: vec![snapshot(SnapshotSlot::Current, "2026-04-05T10:14:30Z")],
        },
        ..pending_state()
    };
    assert!(wrong_history_slot.validate().is_err());
}

#[test]
fn state_document_validation_rejects_invalid_last_run_and_unordered_history() {
    let invalid_transient = StateDocument {
        last_run: Some(last_run(
            RunOutcome::FailedTransient,
            Some(RunFailureCause::ConfigInvalid),
        )),
        ..pending_state()
    };
    assert!(invalid_transient.validate().is_err());

    let invalid_permanent = StateDocument {
        last_run: Some(last_run(
            RunOutcome::FailedPermanent,
            Some(RunFailureCause::FetchTimeout),
        )),
        ..ready_state()
    };
    assert!(invalid_permanent.validate().is_err());

    let invalid_success_cause = StateDocument {
        last_run: Some(last_run(
            RunOutcome::Changed,
            Some(RunFailureCause::FetchTimeout),
        )),
        ..ready_state()
    };
    assert!(invalid_success_cause.validate().is_err());

    let invalid_skipped_cause = StateDocument {
        last_run: Some(last_run(
            RunOutcome::SkippedDisabled,
            Some(RunFailureCause::FetchTimeout),
        )),
        ..pending_state()
    };
    assert!(invalid_skipped_cause.validate().is_err());

    let invalid_timestamp = StateDocument {
        last_run: Some(LastRunRecord::new(
            "bad".to_owned(),
            RunOutcome::Changed,
            None,
        )),
        ..ready_state()
    };
    assert!(invalid_timestamp.validate().is_err());

    let invalid_history_order = StateDocument {
        baseline: StoredBaseline::Ready {
            current_snapshot: snapshot(SnapshotSlot::Current, "2026-04-05T10:15:30Z"),
            snapshot_history: vec![
                snapshot(SnapshotSlot::History, "2026-04-05T10:15:31Z"),
                snapshot(SnapshotSlot::History, "2026-04-05T10:14:30Z"),
            ],
        },
        ..ready_state()
    };
    assert!(invalid_history_order.validate().is_err());

    let invalid_current_recency = StateDocument {
        baseline: StoredBaseline::Ready {
            current_snapshot: snapshot(SnapshotSlot::Current, "2026-04-05T10:14:29Z"),
            snapshot_history: vec![snapshot(SnapshotSlot::History, "2026-04-05T10:14:30Z")],
        },
        ..ready_state()
    };
    assert!(invalid_current_recency.validate().is_err());

    let internally_unordered_history = StateDocument {
        baseline: StoredBaseline::Ready {
            current_snapshot: snapshot(SnapshotSlot::Current, "2026-04-05T10:16:00Z"),
            snapshot_history: vec![
                snapshot(SnapshotSlot::History, "2026-04-05T10:14:30Z"),
                snapshot(SnapshotSlot::History, "2026-04-05T10:15:30Z"),
            ],
        },
        ..ready_state()
    };
    assert!(internally_unordered_history.validate().is_err());
}

#[test]
fn state_document_round_trips_through_the_new_wire_shape() {
    let state = StateDocument {
        last_run: Some(last_run(
            RunOutcome::FailedPermanent,
            Some(RunFailureCause::CompareError),
        )),
        ..ready_state()
    };
    let json = serde_json::to_string(&state).expect("state json");
    assert!(json.contains("\"baseline\""));
    assert!(json.contains("\"last_run\""));
    assert!(!json.contains("\"last_reason_code\""));
    assert!(!json.contains("\"state_phase\""));

    let parsed: StateDocument = serde_json::from_str(&json).expect("state round trip");
    assert_eq!(parsed, state);

    let legacy_json = serde_json::json!({
        "schema_name": STATE_SCHEMA_NAME,
        "schema_version": STATE_SCHEMA_VERSION,
        "target_id": "demo",
        "state_phase": "has_baseline",
        "current_snapshot": {
            "slot": "current",
            "compare_digest_sha256": DIGEST,
            "outer_html_sha256": DIGEST,
            "extraction_record_path": "snapshots/current/extraction.json",
            "compare_path": "snapshots/current/compare.txt",
            "outer_html_path": "snapshots/current/outer.html",
            "captured_at": "2026-04-05T10:15:30Z"
        }
    });
    assert!(serde_json::from_value::<StateDocument>(legacy_json).is_err());
}

use super::*;
use serde_json::json;
use std::collections::BTreeMap;

fn valid_ready_status() -> StatusReport {
    StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: target_id("demo"),
        target_status: TargetStatus::Ready,
        reason_code: ReasonCode::Ok,
        error_detail: None,
        state_phase: Some(StatePhase::HasBaseline),
        current_snapshot: Some(SnapshotDigestSummary {
            canonical_text_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            captured_at: "2026-04-05T10:15:30Z".to_owned(),
        }),
        snapshot_history: vec![SnapshotDigestSummary {
            canonical_text_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            captured_at: "2026-04-05T10:14:30Z".to_owned(),
        }],
        extensions: None,
    }
}

#[test]
fn status_report_accessors_expose_the_public_contract() {
    let mut report = valid_ready_status();
    report.extensions = Some(BTreeMap::from([(
        "demo".to_owned(),
        json!({"kind": "ext"}),
    )]));

    assert_eq!(report.schema_name(), STATUS_REPORT_SCHEMA_NAME);
    assert_eq!(report.schema_version(), STATUS_REPORT_SCHEMA_VERSION);
    assert_eq!(report.target_id(), "demo");
    assert_eq!(report.target_status(), TargetStatus::Ready);
    assert_eq!(report.reason_code(), ReasonCode::Ok);
    assert!(report.error_detail().is_none());
    assert_eq!(report.state_phase(), Some(StatePhase::HasBaseline));
    let current_snapshot = report.current_snapshot().expect("current snapshot");
    assert_eq!(current_snapshot.canonical_text_sha256(), DIGEST);
    assert_eq!(current_snapshot.outer_html_sha256(), DIGEST);
    assert_eq!(current_snapshot.captured_at(), "2026-04-05T10:15:30Z");
    assert_eq!(report.snapshot_history().len(), 1);
    let history_snapshot = &report.snapshot_history()[0];
    assert_eq!(history_snapshot.canonical_text_sha256(), DIGEST);
    assert_eq!(history_snapshot.outer_html_sha256(), DIGEST);
    assert_eq!(history_snapshot.captured_at(), "2026-04-05T10:14:30Z");
    assert_eq!(
        report.extensions().expect("extensions").get("demo"),
        Some(&json!({"kind": "ext"}))
    );
}

#[test]
fn status_report_validation_enforces_state_phase_rules() {
    StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: target_id("demo"),
        target_status: TargetStatus::Invalid,
        reason_code: ReasonCode::ConfigInvalid,
        error_detail: Some(valid_process_error()),
        state_phase: None,
        current_snapshot: None,
        snapshot_history: Vec::new(),
        extensions: None,
    }
    .validate()
    .expect("config invalid report");

    let invalid = StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: target_id("demo"),
        target_status: TargetStatus::Ready,
        reason_code: ReasonCode::Ok,
        error_detail: None,
        state_phase: None,
        current_snapshot: None,
        snapshot_history: Vec::new(),
        extensions: None,
    };
    assert!(invalid.validate().is_err());

    let wrong_target_status = StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: target_id("demo"),
        target_status: TargetStatus::Ready,
        reason_code: ReasonCode::ConfigInvalid,
        error_detail: Some(valid_process_error()),
        state_phase: None,
        current_snapshot: None,
        snapshot_history: Vec::new(),
        extensions: None,
    };
    assert!(wrong_target_status.validate().is_err());

    let invalid_identity = StatusReport {
        schema_name: "wrong".to_owned(),
        ..StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: target_id("demo"),
            target_status: TargetStatus::Ready,
            reason_code: ReasonCode::Ok,
            error_detail: None,
            state_phase: Some(StatePhase::HasBaseline),
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        }
    };
    assert!(invalid_identity.validate().is_err());

    let mut invalid_snapshot = StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: target_id("demo"),
        target_status: TargetStatus::Ready,
        reason_code: ReasonCode::Ok,
        error_detail: None,
        state_phase: Some(StatePhase::HasBaseline),
        current_snapshot: Some(SnapshotDigestSummary {
            canonical_text_sha256: "bad".to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            captured_at: "2026-04-05T10:15:30Z".to_owned(),
        }),
        snapshot_history: Vec::new(),
        extensions: None,
    };
    assert!(invalid_snapshot.validate().is_err());

    invalid_snapshot.current_snapshot = Some(SnapshotDigestSummary {
        canonical_text_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        captured_at: "2026-04-05T10:15:30Z".to_owned(),
    });
    invalid_snapshot.snapshot_history = vec![SnapshotDigestSummary {
        canonical_text_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        captured_at: "2026-04-05T10:14:30Z".to_owned(),
    }];
    invalid_snapshot.validate().expect("ready status report");
}

#[test]
fn status_report_validation_enforces_error_detail_presence_and_absence() {
    let missing_invalid_detail = StatusReport {
        target_status: TargetStatus::Invalid,
        reason_code: ReasonCode::ConfigInvalid,
        error_detail: None,
        state_phase: None,
        current_snapshot: None,
        snapshot_history: Vec::new(),
        ..valid_ready_status()
    };
    assert!(missing_invalid_detail.validate().is_err());

    let pending_with_detail = StatusReport {
        target_status: TargetStatus::Pending,
        reason_code: ReasonCode::Ok,
        error_detail: Some(valid_process_error()),
        state_phase: Some(StatePhase::NeverSucceeded),
        current_snapshot: None,
        snapshot_history: Vec::new(),
        ..valid_ready_status()
    };
    assert!(pending_with_detail.validate().is_err());

    let ready_with_detail = StatusReport {
        error_detail: Some(valid_process_error()),
        ..valid_ready_status()
    };
    assert!(ready_with_detail.validate().is_err());
}

#[test]
fn status_report_deserialization_revalidates_raw_documents() {
    let report = valid_ready_status();

    let json = serde_json::to_string(&report).expect("status json");
    assert!(!json.contains("\"artifacts\""));
    let parsed: StatusReport = serde_json::from_str(&json).expect("status report");
    assert_eq!(parsed, report);

    let mut invalid = report;
    invalid.state_phase = None;
    let invalid_json = serde_json::to_string(&invalid).expect("invalid status json");
    assert!(serde_json::from_str::<StatusReport>(&invalid_json).is_err());

    let legacy_json = serde_json::json!({
        "schema_name": STATUS_REPORT_SCHEMA_NAME,
        "schema_version": STATUS_REPORT_SCHEMA_VERSION,
        "target_id": "demo",
        "target_status": "ready",
        "reason_code": "ok",
        "state_phase": "has_baseline",
        "artifacts": {
            "current_valid": true,
            "previous_valid": true
        },
        "current_snapshot": {
            "canonical_text_sha256": DIGEST,
            "outer_html_sha256": DIGEST,
            "captured_at": "2026-04-05T10:15:30Z"
        }
    });
    assert!(serde_json::from_value::<StatusReport>(legacy_json).is_err());
}

#[test]
fn status_report_validation_rejects_snapshot_mismatches_and_unordered_history() {
    let invalid_pending = StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: target_id("demo"),
        target_status: TargetStatus::Pending,
        reason_code: ReasonCode::Ok,
        error_detail: None,
        state_phase: Some(StatePhase::NeverSucceeded),
        current_snapshot: Some(SnapshotDigestSummary {
            canonical_text_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            captured_at: "2026-04-05T10:15:30Z".to_owned(),
        }),
        snapshot_history: Vec::new(),
        extensions: None,
    };
    assert!(invalid_pending.validate().is_err());

    let unordered_ready = StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: target_id("demo"),
        target_status: TargetStatus::Ready,
        reason_code: ReasonCode::Ok,
        error_detail: None,
        state_phase: Some(StatePhase::HasBaseline),
        current_snapshot: Some(SnapshotDigestSummary {
            canonical_text_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            captured_at: "2026-04-05T10:15:30Z".to_owned(),
        }),
        snapshot_history: vec![SnapshotDigestSummary {
            canonical_text_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            captured_at: "2026-04-05T10:15:31Z".to_owned(),
        }],
        extensions: None,
    };
    assert!(unordered_ready.validate().is_err());

    let invalid_ready = StatusReport {
        current_snapshot: None,
        snapshot_history: Vec::new(),
        ..valid_ready_status()
    };
    assert!(invalid_ready.validate().is_err());

    let invalid_null_phase = StatusReport {
        target_status: TargetStatus::Invalid,
        reason_code: ReasonCode::StateInvalid,
        state_phase: None,
        current_snapshot: None,
        snapshot_history: Vec::new(),
        ..valid_ready_status()
    };
    assert!(invalid_null_phase.validate().is_err());

    let unordered_history = StatusReport {
        current_snapshot: Some(SnapshotDigestSummary {
            canonical_text_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            captured_at: "2026-04-05T10:16:00Z".to_owned(),
        }),
        snapshot_history: vec![
            SnapshotDigestSummary {
                canonical_text_sha256: DIGEST.to_owned(),
                outer_html_sha256: DIGEST.to_owned(),
                captured_at: "2026-04-05T10:14:00Z".to_owned(),
            },
            SnapshotDigestSummary {
                canonical_text_sha256: DIGEST.to_owned(),
                outer_html_sha256: DIGEST.to_owned(),
                captured_at: "2026-04-05T10:15:00Z".to_owned(),
            },
        ],
        ..valid_ready_status()
    };
    assert!(unordered_history.validate().is_err());

    let pending_with_history_only = StatusReport {
        target_status: TargetStatus::Pending,
        reason_code: ReasonCode::Ok,
        state_phase: Some(StatePhase::NeverSucceeded),
        current_snapshot: None,
        snapshot_history: vec![SnapshotDigestSummary {
            canonical_text_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            captured_at: "2026-04-05T10:14:30Z".to_owned(),
        }],
        ..valid_ready_status()
    };
    assert!(pending_with_history_only.validate().is_err());

    let ready_without_history = StatusReport {
        snapshot_history: Vec::new(),
        ..valid_ready_status()
    };
    ready_without_history
        .validate()
        .expect("ready status can omit history");

    let ordered_history = StatusReport {
        current_snapshot: Some(SnapshotDigestSummary {
            canonical_text_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            captured_at: "2026-04-05T10:16:00Z".to_owned(),
        }),
        snapshot_history: vec![
            SnapshotDigestSummary {
                canonical_text_sha256: DIGEST.to_owned(),
                outer_html_sha256: DIGEST.to_owned(),
                captured_at: "2026-04-05T10:15:00Z".to_owned(),
            },
            SnapshotDigestSummary {
                canonical_text_sha256: DIGEST.to_owned(),
                outer_html_sha256: DIGEST.to_owned(),
                captured_at: "2026-04-05T10:14:00Z".to_owned(),
            },
        ],
        ..valid_ready_status()
    };
    ordered_history
        .validate()
        .expect("ready status keeps newest-first history");
}

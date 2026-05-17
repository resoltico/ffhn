use super::*;
use serde_json::json;
use std::collections::BTreeMap;

fn digest_summary(captured_at: &str) -> SnapshotDigestSummary {
    SnapshotDigestSummary {
        compare_digest_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        captured_at: captured_at.to_owned(),
    }
}

fn valid_ready_status() -> StatusReport {
    StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: target_id("demo"),
        display_name: Some("Demo".to_owned()),
        enabled: Some(true),
        status: StatusSummary::Ready {
            current_snapshot: digest_summary("2026-04-05T10:15:30Z"),
            snapshot_history: vec![digest_summary("2026-04-05T10:14:30Z")],
        },
        extensions: None,
    }
}

#[test]
fn status_report_accessors_expose_the_public_contract() {
    let is_invalid: fn(&StatusSummary) -> bool = StatusSummary::is_invalid;
    let mut report = valid_ready_status();
    report.extensions = Some(BTreeMap::from([(
        "demo".to_owned(),
        json!({"kind": "ext"}),
    )]));

    assert_eq!(report.schema_name(), STATUS_REPORT_SCHEMA_NAME);
    assert_eq!(report.schema_version(), STATUS_REPORT_SCHEMA_VERSION);
    assert_eq!(report.target_id(), "demo");
    assert_eq!(report.enabled(), Some(true));
    assert!(matches!(report.status(), StatusSummary::Ready { .. }));
    assert!(report.status().is_ready());
    assert_eq!(report.status().kind_str(), "ready");
    assert_eq!(report.baseline_phase(), Some(BaselinePhase::HasBaseline));
    assert!(report.error_detail().is_none());
    let current_snapshot = report.current_snapshot().expect("current snapshot");
    assert_eq!(current_snapshot.compare_digest_sha256(), DIGEST);
    assert_eq!(current_snapshot.outer_html_sha256(), DIGEST);
    assert_eq!(current_snapshot.captured_at(), "2026-04-05T10:15:30Z");
    assert_eq!(report.snapshot_history().len(), 1);
    let history_snapshot = &report.snapshot_history()[0];
    assert_eq!(history_snapshot.compare_digest_sha256(), DIGEST);
    assert_eq!(history_snapshot.outer_html_sha256(), DIGEST);
    assert_eq!(history_snapshot.captured_at(), "2026-04-05T10:14:30Z");
    assert_eq!(
        report.extensions().expect("extensions").get("demo"),
        Some(&json!({"kind": "ext"}))
    );

    let invalid_config = StatusSummary::InvalidConfig {
        error_detail: valid_process_error(),
    };
    assert!(is_invalid(&invalid_config));
    assert_eq!(invalid_config.kind_str(), "invalid_config");
    assert_eq!(invalid_config.baseline_phase(), None);
    assert!(invalid_config.current_snapshot().is_none());
    assert!(invalid_config.snapshot_history().is_empty());

    let invalid_state = StatusSummary::InvalidState {
        baseline_phase: BaselinePhase::NeverSucceeded,
        error_detail: valid_process_error(),
    };
    assert!(is_invalid(&invalid_state));
    assert_eq!(invalid_state.kind_str(), "invalid_state");
    assert_eq!(
        invalid_state.baseline_phase(),
        Some(BaselinePhase::NeverSucceeded)
    );
    assert!(invalid_state.current_snapshot().is_none());
    assert!(invalid_state.snapshot_history().is_empty());

    let integrity_mismatch = StatusSummary::IntegrityMismatch {
        baseline_phase: BaselinePhase::HasBaseline,
        error_detail: valid_process_error(),
    };
    assert!(is_invalid(&integrity_mismatch));
    assert_eq!(integrity_mismatch.kind_str(), "integrity_mismatch");
    assert_eq!(
        integrity_mismatch.baseline_phase(),
        Some(BaselinePhase::HasBaseline)
    );
    assert!(integrity_mismatch.current_snapshot().is_none());
    assert!(integrity_mismatch.snapshot_history().is_empty());

    let incompatible_baseline = StatusSummary::IncompatibleBaseline {
        baseline_phase: BaselinePhase::HasBaseline,
        error_detail: valid_process_error(),
    };
    assert!(is_invalid(&incompatible_baseline));
    assert_eq!(incompatible_baseline.kind_str(), "incompatible_baseline");
    assert_eq!(
        incompatible_baseline.baseline_phase(),
        Some(BaselinePhase::HasBaseline)
    );
    assert!(incompatible_baseline.current_snapshot().is_none());
    assert!(incompatible_baseline.snapshot_history().is_empty());
}

#[test]
fn status_report_validation_enforces_status_summary_rules() {
    StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: target_id("demo"),
        display_name: None,
        enabled: None,
        status: StatusSummary::InvalidConfig {
            error_detail: valid_process_error(),
        },
        extensions: None,
    }
    .validate()
    .expect("config invalid report");

    let invalid_identity = StatusReport {
        schema_name: "wrong".to_owned(),
        ..valid_ready_status()
    };
    assert!(invalid_identity.validate().is_err());

    let invalid_snapshot = StatusReport {
        status: StatusSummary::Ready {
            current_snapshot: SnapshotDigestSummary {
                compare_digest_sha256: "bad".to_owned(),
                ..digest_summary("2026-04-05T10:15:30Z")
            },
            snapshot_history: Vec::new(),
        },
        ..valid_ready_status()
    };
    assert!(invalid_snapshot.validate().is_err());

    let invalid_detail = StatusReport {
        enabled: Some(true),
        status: StatusSummary::InvalidState {
            baseline_phase: BaselinePhase::HasBaseline,
            error_detail: ProcessErrorDetail {
                kind: ProcessErrorKind::Io,
                message: String::new(),
                path: None,
            },
        },
        ..valid_ready_status()
    };
    assert!(invalid_detail.validate().is_err());

    let missing_enabled = StatusReport {
        enabled: None,
        ..valid_ready_status()
    };
    assert!(missing_enabled.validate().is_err());

    let invalid_config_with_enabled = StatusReport {
        enabled: Some(true),
        status: StatusSummary::InvalidConfig {
            error_detail: valid_process_error(),
        },
        ..valid_ready_status()
    };
    assert!(invalid_config_with_enabled.validate().is_err());
}

#[test]
fn status_report_deserialization_revalidates_raw_documents() {
    let report = valid_ready_status();

    let json = serde_json::to_string(&report).expect("status json");
    let parsed: StatusReport = serde_json::from_str(&json).expect("status report");
    assert_eq!(parsed, report);

    let invalid_json = serde_json::json!({
        "schema_name": STATUS_REPORT_SCHEMA_NAME,
        "schema_version": STATUS_REPORT_SCHEMA_VERSION,
        "target_id": "demo",
        "enabled": true,
        "status": {
            "kind": "ready",
            "snapshot_history": []
        }
    });
    assert!(serde_json::from_value::<StatusReport>(invalid_json).is_err());

    let missing_enabled_json = serde_json::json!({
        "schema_name": STATUS_REPORT_SCHEMA_NAME,
        "schema_version": STATUS_REPORT_SCHEMA_VERSION,
        "target_id": "demo",
        "status": {
            "kind": "pending"
        }
    });
    assert!(serde_json::from_value::<StatusReport>(missing_enabled_json).is_err());

    let legacy_json = serde_json::json!({
        "schema_name": STATUS_REPORT_SCHEMA_NAME,
        "schema_version": STATUS_REPORT_SCHEMA_VERSION,
        "target_id": "demo",
        "target_status": "ready",
        "reason_code": "ok",
        "state_phase": "has_baseline",
        "current_snapshot": {
            "compare_digest_sha256": DIGEST,
            "outer_html_sha256": DIGEST,
            "captured_at": "2026-04-05T10:15:30Z"
        }
    });
    assert!(serde_json::from_value::<StatusReport>(legacy_json).is_err());
}

#[test]
fn status_report_validation_rejects_unordered_or_mismatched_snapshots() {
    let invalid_pending = StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: target_id("demo"),
        display_name: Some("Demo".to_owned()),
        enabled: Some(false),
        status: StatusSummary::Pending,
        extensions: None,
    };
    invalid_pending.validate().expect("pending report");
    assert_eq!(invalid_pending.status().kind_str(), "pending");

    let unordered_ready = StatusReport {
        status: StatusSummary::Ready {
            current_snapshot: digest_summary("2026-04-05T10:15:30Z"),
            snapshot_history: vec![digest_summary("2026-04-05T10:15:31Z")],
        },
        ..valid_ready_status()
    };
    assert!(unordered_ready.validate().is_err());

    let ordered_history = StatusReport {
        status: StatusSummary::Ready {
            current_snapshot: digest_summary("2026-04-05T10:16:00Z"),
            snapshot_history: vec![
                digest_summary("2026-04-05T10:15:00Z"),
                digest_summary("2026-04-05T10:14:00Z"),
            ],
        },
        ..valid_ready_status()
    };
    ordered_history
        .validate()
        .expect("ready status keeps newest-first history");

    let internally_unordered_history = StatusReport {
        status: StatusSummary::Ready {
            current_snapshot: digest_summary("2026-04-05T10:16:00Z"),
            snapshot_history: vec![
                digest_summary("2026-04-05T10:14:00Z"),
                digest_summary("2026-04-05T10:15:00Z"),
            ],
        },
        ..valid_ready_status()
    };
    assert!(internally_unordered_history.validate().is_err());

    let invalid_state = StatusReport {
        status: StatusSummary::InvalidState {
            baseline_phase: BaselinePhase::NeverSucceeded,
            error_detail: valid_process_error(),
        },
        ..valid_ready_status()
    };
    invalid_state.validate().expect("invalid-state report");

    let integrity_mismatch = StatusReport {
        status: StatusSummary::IntegrityMismatch {
            baseline_phase: BaselinePhase::HasBaseline,
            error_detail: valid_process_error(),
        },
        ..valid_ready_status()
    };
    integrity_mismatch
        .validate()
        .expect("integrity-mismatch report");
}

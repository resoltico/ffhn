use crate::model::StatusReportParts;
use crate::{RunReport, StatusReport};

fn healthy_snapshot() -> serde_json::Value {
    serde_json::json!({
        "source_health": {
            "state": "healthy",
            "reason_class": null,
            "consecutive_unresolved": 0,
            "first_unresolved_at": null,
            "last_details": null,
        },
        "permanent_error_episode": null,
        "integration_fault_episode": null,
    })
}

fn valid_report() -> serde_json::Value {
    let snapshot = healthy_snapshot();
    serde_json::json!({
        "schema_name": "ffhn.run_report",
        "schema_version": 17,
        "target_id": "demo",
        "run_mode": "live",
        "outcome": "initialized",
        "run_started_at": "2026-07-18T00:00:00Z",
        "run_finished_at": "2026-07-18T00:00:01Z",
        "policy_evaluation": {"status": "not_evaluated", "event_eligibilities": []},
        "lifecycle": {"before": snapshot.clone(), "after": snapshot},
        "state_persisted": true,
        "delivery_outcomes": [],
        "outbox_overflow": [],
    })
}

#[test]
fn lifecycle_reports_require_all_axes_and_reject_incoherent_or_missing_staged_state() {
    let report = valid_report();
    assert!(serde_json::from_value::<RunReport>(report.clone()).is_ok());

    let mut missing_required_after = report.clone();
    missing_required_after["outcome"] = serde_json::json!("changed");
    missing_required_after["lifecycle"]["after"] = serde_json::Value::Null;
    missing_required_after["state_persisted"] = serde_json::json!(false);
    assert!(serde_json::from_value::<RunReport>(missing_required_after).is_err());

    let mut persisted_without_after = report.clone();
    persisted_without_after["lifecycle"]["after"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<RunReport>(persisted_without_after).is_err());

    for outcome in [
        "skipped_disabled",
        "refused_contract_digest",
        "target_unavailable",
        "state_invalid",
        "lock_unavailable",
    ] {
        let mut forbidden_after = report.clone();
        forbidden_after["outcome"] = serde_json::json!(outcome);
        forbidden_after["state_persisted"] = serde_json::json!(false);
        assert!(
            serde_json::from_value::<RunReport>(forbidden_after).is_err(),
            "{outcome} must not claim a staged lifecycle"
        );
    }

    for outcome in [
        "initialized",
        "changed",
        "unchanged",
        "acquisition_failed",
        "value_unparseable",
        "fetch_failed",
        "persist_failed",
        "integration_fault",
    ] {
        let mut missing_after = report.clone();
        missing_after["outcome"] = serde_json::json!(outcome);
        missing_after["lifecycle"]["after"] = serde_json::Value::Null;
        missing_after["state_persisted"] = serde_json::json!(false);
        assert!(
            serde_json::from_value::<RunReport>(missing_after).is_err(),
            "{outcome} must carry its staged lifecycle"
        );
    }

    let mut unstaged_config_error = report.clone();
    unstaged_config_error["outcome"] = serde_json::json!("config_invalid");
    unstaged_config_error["lifecycle"]["after"] = serde_json::Value::Null;
    unstaged_config_error["state_persisted"] = serde_json::json!(false);
    assert!(serde_json::from_value::<RunReport>(unstaged_config_error).is_ok());

    let mut incoherent_health = report.clone();
    incoherent_health["lifecycle"]["before"]["source_health"]["consecutive_unresolved"] =
        serde_json::json!(1);
    assert!(serde_json::from_value::<RunReport>(incoherent_health).is_err());

    let mut legacy_integration_field = report;
    legacy_integration_field["lifecycle"]["after"]["integration_fault_episode"] = serde_json::json!({
        "integration_fault_code": "ffhn_boundary_invariant_violation",
        "first_seen_at": "2026-07-18T00:00:00Z",
    });
    assert!(serde_json::from_value::<RunReport>(legacy_integration_field).is_err());

    let status = serde_json::json!({
        "schema_name": "ffhn.status_report",
        "schema_version": 13,
        "target_id": "demo",
        "kind": "invalid_config",
        "display_name": "Demo",
        "enabled": true,
        "contract_digest_sha256": "a".repeat(64),
        "lifecycle": healthy_snapshot(),
    });
    let durable_status: StatusReport = serde_json::from_value(status.clone()).expect("status");
    let lifecycle = durable_status
        .lifecycle()
        .cloned()
        .expect("verified lifecycle");
    assert!(
        StatusReport::new(StatusReportParts {
            target_id: "demo".to_owned(),
            kind: crate::StatusKind::Pending,
            display_name: Some("Demo".to_owned()),
            enabled: Some(true),
            digest: Some("a".repeat(64)),
            observation: None,
            error: None,
            lifecycle: Some(lifecycle.clone()),
        })
        .is_ok()
    );
    assert!(
        StatusReport::new(StatusReportParts {
            target_id: "demo".to_owned(),
            kind: crate::StatusKind::InvalidState,
            display_name: None,
            enabled: None,
            digest: None,
            observation: None,
            error: None,
            lifecycle: Some(lifecycle),
        })
        .is_err()
    );
    let mut unverified_state = status;
    unverified_state["kind"] = serde_json::json!("invalid_state");
    assert!(serde_json::from_value::<StatusReport>(unverified_state).is_err());

    let mut unavailable_target = serde_json::to_value(&durable_status).expect("status JSON");
    unavailable_target["kind"] = serde_json::json!("unavailable_target");
    assert!(serde_json::from_value::<StatusReport>(unavailable_target).is_err());

    let mut ready_without_lifecycle = serde_json::to_value(&durable_status).expect("status JSON");
    ready_without_lifecycle["kind"] = serde_json::json!("ready");
    ready_without_lifecycle["lifecycle"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<StatusReport>(ready_without_lifecycle).is_err());

    for field in ["display_name", "enabled", "contract_digest_sha256"] {
        let mut missing_identity_fact = serde_json::to_value(&durable_status).expect("status JSON");
        missing_identity_fact[field] = serde_json::Value::Null;
        assert!(
            serde_json::from_value::<StatusReport>(missing_identity_fact).is_err(),
            "a lifecycle-bearing status must retain {field}"
        );
    }
}

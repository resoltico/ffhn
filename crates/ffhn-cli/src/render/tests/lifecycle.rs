use super::*;

#[test]
fn summary_renders_one_error_category_without_repeating_it_in_the_message() {
    let mut value: serde_json::Value = serde_json::from_str(
        r#"{"schema_name":"ffhn.run_report","schema_version":17,"target_id":"demo","run_mode":"live","outcome":"config_invalid","run_started_at":"2026-01-01T00:00:00Z","run_finished_at":"2026-01-01T00:00:01Z","error_detail":{"kind":"contract","operation":"state_load","message":"stored state schema is incompatible"},"policy_evaluation":{"status":"not_evaluated","event_eligibilities":[]},"state_persisted":false,"delivery_outcomes":[],"outbox_overflow":[]}"#,
    )
    .expect("error report JSON");
    value["lifecycle"] = healthy_lifecycle(false, false);
    let report: RunReport = serde_json::from_value(value).expect("error report");
    let mut summary = Vec::new();
    render_run_report(&mut summary, &report, OutputFormat::Summary).expect("summary");
    let summary = String::from_utf8(summary).expect("UTF-8");

    assert!(
        summary.contains("Error [contract] during state load: stored state schema is incompatible")
    );
    assert!(!summary.contains("contract error:"));
}

#[test]
fn summary_renders_every_populated_lifecycle_axis_and_source_health_detail() {
    let report: RunReport = serde_json::from_value(serde_json::json!({
        "schema_name": "ffhn.run_report",
        "schema_version": 17,
        "target_id": "demo",
        "run_mode": "live",
        "outcome": "config_invalid",
        "run_started_at": "2026-07-18T00:00:00Z",
        "run_finished_at": "2026-07-18T00:00:01Z",
        "policy_evaluation": {"status": "not_evaluated", "event_eligibilities": []},
        "lifecycle": {
            "before": null,
            "after": {
                "source_health": {
                    "state": "suspect",
                    "reason_class": "json_malformed",
                    "consecutive_unresolved": 3,
                    "first_unresolved_at": "2026-07-18T00:00:00Z",
                    "last_details": {
                        "kind": "json",
                        "operation": "json_pointer_selection",
                        "message": "source was not valid JSON"
                    }
                },
                "permanent_error_episode": {
                    "error_code": "invalid_json_pointer",
                    "first_seen_at": "2026-07-18T00:00:00Z"
                },
                "integration_fault_episode": {
                    "code": "ffhn_boundary_invariant_violation",
                    "first_seen_at": "2026-07-18T00:00:00Z"
                }
            }
        },
        "state_persisted": true,
        "delivery_outcomes": [],
        "outbox_overflow": []
    }))
    .expect("detailed lifecycle report");
    let mut summary = Vec::new();
    render_run_report(&mut summary, &report, OutputFormat::Summary).expect("summary");
    let summary = String::from_utf8(summary).expect("UTF-8 summary");

    assert!(summary.contains(
        "Lifecycle after: source_health.state=suspect, reason_class=json_malformed, consecutive_unresolved=3"
    ));
    assert!(
        summary.contains("Lifecycle after: source_health.first_unresolved_at=2026-07-18T00:00:00Z")
    );
    assert!(summary.contains("Lifecycle after: source_health.last_details:"));
    assert!(
        summary.contains("Error [json] during JSON Pointer selection: source was not valid JSON")
    );
    assert!(summary.contains(
        "Lifecycle after: permanent_error_episode.code=invalid_json_pointer, first_seen_at=2026-07-18T00:00:00Z"
    ));
    assert!(summary.contains(
        "Lifecycle after: integration_fault_episode.code=ffhn_boundary_invariant_violation, first_seen_at=2026-07-18T00:00:00Z"
    ));
}

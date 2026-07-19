use super::*;

mod delivery_process;
mod diagnostic;
mod lifecycle;

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("writer failed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn run_report() -> RunReport {
    let mut report: serde_json::Value = serde_json::from_str(
            r#"{"schema_name":"ffhn.run_report","schema_version":17,"target_id":"demo","run_mode":"live","outcome":"changed","run_started_at":"2026-01-01T00:00:00Z","run_finished_at":"2026-01-01T00:00:01Z","observation":{"raw_selected":"200","comparison_projection":"200","acquisition_kind":"json_pointer","parser_id":"ffhn.typed-value","parser_grammar_version":1,"declared_type":"integer","type_params":{},"canonical_value":"200","parse_diagnostics":[]},"previous_canonical_value":"110","policy_evaluation":{"status":"evaluated","condition_results":[{"condition_id":"price-rise","outcome":"satisfied","triggered":true,"active_before":false,"active_after":false,"reference":{"status":"resolved","reference":"last_accepted_observation","canonical_value":"110"}}],"event_eligibilities":[{"event_kind":"condition_satisfied","route_family":"on_condition","condition_id":"price-rise"}]},"state_persisted":true,"delivery_outcomes":[],"outbox_overflow":[]}"#,
        )
        .expect("run report JSON");
    report["lifecycle"] = healthy_lifecycle(true, true);
    serde_json::from_value(report).expect("run report")
}

fn healthy_lifecycle(before: bool, after: bool) -> serde_json::Value {
    let snapshot = serde_json::json!({
        "source_health": {
            "state": "healthy",
            "reason_class": null,
            "consecutive_unresolved": 0,
            "first_unresolved_at": null,
            "last_details": null,
        },
        "permanent_error_episode": null,
        "integration_fault_episode": null,
    });
    serde_json::json!({
        "before": before.then_some(snapshot.clone()),
        "after": after.then_some(snapshot),
    })
}

#[test]
fn renders_each_v2_document_in_every_output_mode() {
    let run = run_report();
    let batch: BatchRunReport = serde_json::from_value(serde_json::json!({
        "schema_name": "ffhn.batch_run_report",
        "schema_version": 17,
        "run_mode": "live",
        "requested_targets": ["demo"],
        "reports": [run],
    }))
    .expect("batch");
    let status: StatusReport = serde_json::from_str(
            r#"{"schema_name":"ffhn.status_report","schema_version":13,"target_id":"demo","kind":"pending"}"#,
        )
        .expect("status");
    let reset: ResetReport = serde_json::from_str(
            r#"{"schema_name":"ffhn.reset_report","schema_version":7,"target_id":"demo","storage_cleared":false,"delivery_outcomes":[],"outbox_overflow":[]}"#,
        )
        .expect("reset");
    let mut output = Vec::new();
    render_run_report(&mut output, &run_report(), OutputFormat::Json).expect("run JSON");
    render_batch_report(&mut output, &batch, OutputFormat::JsonPretty).expect("batch pretty");
    render_status_report(&mut output, &status, OutputFormat::Summary).expect("status summary");
    render_reset_report(&mut output, &reset, OutputFormat::Json).expect("reset JSON");
    assert!(
        String::from_utf8(output)
            .expect("UTF-8")
            .contains("ffhn.reset_report")
    );
    let mut failing = FailingWriter;
    assert!(render_run_report(&mut failing, &run_report(), OutputFormat::Json).is_err());
    assert!(failing.flush().is_ok());
}

#[test]
fn summary_is_human_text_and_exposes_policy_decisions_without_routes() {
    let mut summary = Vec::new();
    render_run_report(&mut summary, &run_report(), OutputFormat::Summary).expect("run summary");
    let summary = String::from_utf8(summary).expect("UTF-8");

    assert!(summary.starts_with("Run report\nTarget: demo\n"));
    assert!(summary.contains("Observation: 200"));
    assert!(summary.contains("Previous canonical value: 110"));
    assert!(summary.contains("Policy: evaluated (1 conditions)"));
    assert!(summary.contains(
            "Condition price-rise: outcome=satisfied, triggered=true, active=false -> false, reference=last_accepted_observation=110"
        ));
    assert!(summary.contains(
        "Event: kind=condition_satisfied, route_family=on_condition, condition=price-rise"
    ));
    assert!(summary.contains(
        "Lifecycle before: source_health.state=healthy, reason_class=none, consecutive_unresolved=0"
    ));
    assert!(summary.contains("Lifecycle after: permanent_error_episode=none"));
    assert!(summary.contains("Lifecycle after: integration_fault_episode=none"));
    assert!(!summary.contains("\"schema_name\""));
}

#[test]
fn summary_renders_a_closed_io_class_without_foreign_error_prose() {
    let mut value: serde_json::Value = serde_json::from_str(
        r#"{"schema_name":"ffhn.run_report","schema_version":17,"target_id":"demo","run_mode":"live","outcome":"fetch_failed","run_started_at":"2026-01-01T00:00:00Z","run_finished_at":"2026-01-01T00:00:01Z","error_detail":{"kind":"io","operation":"http_fetch","message":"the HTTP request could not be completed","io_error_class":"connection_refused"},"policy_evaluation":{"status":"not_evaluated","event_eligibilities":[]},"state_persisted":false,"delivery_outcomes":[],"outbox_overflow":[]}"#,
    )
    .expect("I/O error report JSON");
    value["lifecycle"] = healthy_lifecycle(false, true);
    let report: RunReport = serde_json::from_value(value).expect("I/O error report");
    let mut summary = Vec::new();
    render_run_report(&mut summary, &report, OutputFormat::Summary).expect("summary");
    let summary = String::from_utf8(summary).expect("UTF-8 summary");

    assert!(summary.contains(
        "Error [io] during HTTP fetch [connection_refused]: the HTTP request could not be completed"
    ));
    assert!(!summary.contains("Connection refused"));
}

#[test]
fn successful_delivery_stderr_anomalies_are_visible_in_every_format_without_leaking_stderr_to_summary()
 {
    let report: RunReport = serde_json::from_value(serde_json::json!({
        "schema_name": "ffhn.run_report",
        "schema_version": 17,
        "target_id": "demo",
        "run_mode": "live",
        "outcome": "unchanged",
        "run_started_at": "2026-01-01T00:00:00Z",
        "run_finished_at": "2026-01-01T00:00:01Z",
        "policy_evaluation": {"status": "not_evaluated", "event_eligibilities": []},
        "lifecycle": healthy_lifecycle(false, true),
        "state_persisted": true,
        "delivery_outcomes": [{
            "event_id": "a".repeat(64),
            "route_id": "process",
            "event_kind": "initialized",
            "status": "delivered",
            "attempt_count": 1,
            "delivery_observability_detail": {
                "kind": "delivery",
                "operation": "delivery_process",
                "message": "delivery completed but stderr capture was incomplete",
                "delivery_process": {
                    "kind": "observability",
                    "stderr_capture_problem": {
                        "kind": "read_failed",
                        "io": "broken_pipe",
                        "partial": {
                            "retained_bytes_base64": "cHJpdmF0ZSBzdGRlcnIgZXZpZGVuY2U=",
                            "original_len_bytes": "23",
                            "truncated": false
                        }
                    }
                }
            }
        }],
        "outbox_overflow": []
    }))
    .expect("observability report");

    for format in [
        OutputFormat::Json,
        OutputFormat::JsonPretty,
        OutputFormat::Summary,
    ] {
        let mut output = Vec::new();
        render_run_report(&mut output, &report, format).expect("render report");
        let output = String::from_utf8(output).expect("UTF-8 output");
        if format == OutputFormat::Summary {
            assert!(output.contains("Delivery observability"));
            assert!(output.contains("Delivery stderr: read_failed (broken_pipe)"));
            assert!(output.contains(
                "Delivery stderr metadata: retained_encoding=utf8, original_len_bytes=23, truncated=false"
            ));
            assert!(!output.contains("private stderr evidence"));
        } else {
            assert!(output.contains("delivery_observability_detail"));
            assert!(output.contains("retained_bytes_base64"));
            assert!(output.contains("cHJpdmF0ZSBzdGRlcnIgZXZpZGVuY2U="));
            assert!(!output.contains("private stderr evidence"));
        }
    }
}

#[test]
fn summary_renders_an_unavailable_stderr_reader_truthfully() {
    let report: RunReport = serde_json::from_value(serde_json::json!({
        "schema_name": "ffhn.run_report",
        "schema_version": 17,
        "target_id": "demo",
        "run_mode": "live",
        "outcome": "unchanged",
        "run_started_at": "2026-01-01T00:00:00Z",
        "run_finished_at": "2026-01-01T00:00:01Z",
        "policy_evaluation": {"status": "not_evaluated", "event_eligibilities": []},
        "lifecycle": healthy_lifecycle(false, true),
        "state_persisted": true,
        "delivery_outcomes": [{
            "event_id": "a".repeat(64),
            "route_id": "process",
            "event_kind": "initialized",
            "status": "delivered",
            "attempt_count": 1,
            "delivery_observability_detail": {
                "kind": "delivery",
                "operation": "delivery_process",
                "message": "delivery completed but stderr capture was incomplete",
                "delivery_process": {
                    "kind": "observability",
                    "stderr_capture_problem": {"kind": "reader_unavailable"}
                }
            }
        }],
        "outbox_overflow": []
    }))
    .expect("reader-unavailable report");

    let mut summary = Vec::new();
    render_run_report(&mut summary, &report, OutputFormat::Summary).expect("summary");
    assert!(
        String::from_utf8(summary)
            .expect("UTF-8 summary")
            .contains("Delivery stderr: reader_unavailable")
    );
}

#[test]
fn summary_renders_every_failed_delivery_process_fact_without_stderr_text() {
    let report: RunReport = serde_json::from_value(serde_json::json!({
        "schema_name": "ffhn.run_report",
        "schema_version": 17,
        "target_id": "demo",
        "run_mode": "live",
        "outcome": "unchanged",
        "run_started_at": "2026-01-01T00:00:00Z",
        "run_finished_at": "2026-01-01T00:00:01Z",
        "policy_evaluation": {"status": "not_evaluated", "event_eligibilities": []},
        "lifecycle": healthy_lifecycle(false, true),
        "state_persisted": true,
        "delivery_outcomes": [{
            "event_id": "a".repeat(64),
            "route_id": "process",
            "event_kind": "initialized",
            "status": "retry_scheduled",
            "attempt_count": 1,
            "error_detail": {
                "kind": "delivery",
                "operation": "delivery_process",
                "message": "delivery process did not complete successfully",
                "delivery_process": {
                    "kind": "failure",
                    "attempt": {
                        "terminal": {"kind": "exited", "exit_code": 7},
                        "writer": {"kind": "io_failed", "io": "broken_pipe"},
                        "stderr": {
                            "kind": "captured",
                            "retained_bytes_base64": "cHJpdmF0ZSBzdGRlcnIgZXZpZGVuY2U=",
                            "original_len_bytes": "23",
                            "truncated": false
                        }
                    },
                    "primary": "writer_io_failed"
                }
            }
        }],
        "outbox_overflow": []
    }))
    .expect("failed-delivery report");
    let mut summary = Vec::new();
    render_run_report(&mut summary, &report, OutputFormat::Summary).expect("summary");
    let summary = String::from_utf8(summary).expect("UTF-8 summary");

    assert!(summary.contains("Delivery terminal: exited (exit_code=7)"));
    assert!(summary.contains("Delivery writer: io_failed (broken_pipe)"));
    assert!(summary.contains("Delivery stderr: captured"));
    assert!(summary.contains(
        "Delivery stderr metadata: retained_encoding=utf8, original_len_bytes=23, truncated=false"
    ));
    assert!(summary.contains("Delivery primary: writer_io_failed"));
    assert!(!summary.contains("private stderr evidence"));
}

#[test]
fn summary_names_an_evaluated_empty_condition_set() {
    let policy: PolicyEvaluation = serde_json::from_str(
        r#"{"status":"evaluated","condition_results":[],"event_eligibilities":[]}"#,
    )
    .expect("empty evaluated policy");
    let mut summary = Vec::new();
    render_policy_evaluation(&mut summary, &policy).expect("policy summary");
    assert_eq!(
        String::from_utf8(summary).expect("UTF-8 summary"),
        "Policy: evaluated (0 conditions)\nPolicy conditions: none configured\nPolicy events: none eligible\n"
    );
}

#[test]
fn summary_renders_conditions_that_do_not_use_a_reference() {
    let policy: PolicyEvaluation = serde_json::from_str(
            r#"{"status":"evaluated","condition_results":[{"condition_id":"fresh","outcome":"satisfied","triggered":true,"active_before":false,"active_after":false}],"event_eligibilities":[]}"#,
        )
        .expect("reference-free condition");
    let mut summary = Vec::new();
    render_policy_evaluation(&mut summary, &policy).expect("policy summary");
    let summary = String::from_utf8(summary).expect("UTF-8 summary");
    assert!(summary.contains("Condition fresh: outcome=satisfied"));
    assert!(!summary.contains("reference="));
}

#[test]
fn every_output_format_preserves_complete_run_batch_status_and_reset_evidence() {
    let run: RunReport = serde_json::from_value(serde_json::json!({
        "schema_name": "ffhn.run_report",
        "schema_version": 17,
        "target_id": "demo",
        "display_name": "Demo",
        "run_mode": "dry_run",
        "outcome": "changed",
        "run_started_at": "2026-01-01T00:00:00Z",
        "run_finished_at": "2026-01-01T00:00:01Z",
        "contract_digest_sha256": "a".repeat(64),
        "observation": {
            "raw_selected": "200",
            "comparison_projection": "200",
            "acquisition_kind": "json_pointer",
            "parser_id": "ffhn.typed-value",
            "parser_grammar_version": 1,
            "declared_type": "integer",
            "type_params": {},
            "canonical_value": "200",
            "parse_diagnostics": []
        },
        "previous_canonical_value": "110",
        "error_detail": {
            "kind": "policy_invariant",
            "operation": "policy_evaluation",
            "message": "exact proof failed",
            "path": "/watchlist/demo/state.json",
            "integration_fault_code": "ffhn_policy_invariant_violation"
        },
        "policy_evaluation": {
            "status": "evaluated",
            "condition_results": [
                {
                    "condition_id": "price-rise",
                    "outcome": "unavailable",
                    "triggered": false,
                    "active_before": true,
                    "active_after": false,
                    "reference": {
                        "status": "unavailable",
                        "reference": "last_condition_transition"
                    }
                }
            ],
            "event_eligibilities": [
                {
                    "event_kind": "integration_fault",
                    "route_family": "on_run",
                    "integration_fault_code": "ffhn_policy_invariant_violation"
                },
                {
                    "event_kind": "source_suspect_escalated",
                    "route_family": "on_run",
                    "reason_class": "fetch_failed"
                },
                {
                    "event_kind": "permanent_contract_error",
                    "route_family": "on_run",
                    "error_code": "invalid_json_pointer"
                }
            ]
        },
        "lifecycle": healthy_lifecycle(true, true),
        "state_persisted": false,
        "delivery_outcomes": [{
            "event_id": "b".repeat(64),
            "route_id": "process",
            "event_kind": "condition_satisfied",
            "condition_id": "price-rise",
            "status": "retry_scheduled",
            "attempt_count": 2,
            "error_detail": {
                "kind": "delivery",
                "operation": "delivery_process",
                "message": "delivery process did not complete successfully",
                "delivery_process": {
                    "kind": "failure",
                    "attempt": {
                        "terminal": {"kind": "exited", "exit_code": 1},
                        "writer": {"kind": "completed"},
                        "stderr": {"kind": "captured", "retained_bytes_base64": "", "original_len_bytes": "0", "truncated": false}
                    },
                    "primary": "unsuccessful_exit"
                }
            }
        }],
        "outbox_overflow": [{
            "event_id": "c".repeat(64),
            "route_id": "overflow",
            "event_kind": "condition_satisfied",
            "condition_id": "price-rise"
        }],
        "outbox_error_detail": {"kind": "io", "operation": "outbox_state_commit", "message": "outbox persistence stopped", "io_error_class": "storage_full"}
    }))
    .expect("complete run report");
    let batch: BatchRunReport = serde_json::from_value(serde_json::json!({
        "schema_name": "ffhn.batch_run_report",
        "schema_version": 17,
        "run_mode": "dry_run",
        "requested_targets": ["demo"],
        "reports": [run],
    }))
    .expect("complete batch report");
    let status: StatusReport = serde_json::from_value(serde_json::json!({
        "schema_name": "ffhn.status_report",
        "schema_version": 13,
        "target_id": "demo",
        "display_name": "Demo",
        "enabled": false,
        "kind": "invalid_state",
        "contract_digest_sha256": "d".repeat(64),
        "error_detail": {
            "kind": "io",
            "operation": "state_load",
            "message": "state unavailable",
            "io_error_class": "not_found"
        }
    }))
    .expect("complete status report");
    let reset: ResetReport = serde_json::from_value(serde_json::json!({
        "schema_name": "ffhn.reset_report",
        "schema_version": 7,
        "target_id": "demo",
        "storage_cleared": true,
        "delivery_outcomes": [{
            "event_id": "e".repeat(64),
            "route_id": "reset-process",
            "event_kind": "reset",
            "status": "delivered",
            "attempt_count": 1
        }],
        "outbox_overflow": [{
            "event_id": "f".repeat(64),
            "route_id": "reset-overflow",
            "event_kind": "reset"
        }],
        "outbox_error_detail": {"kind": "io", "operation": "outbox_drain", "message": "reset delivery stopped", "io_error_class": "other"}
    }))
    .expect("complete reset report");

    for format in [
        OutputFormat::Json,
        OutputFormat::JsonPretty,
        OutputFormat::Summary,
    ] {
        let mut output = Vec::new();
        render_run_report(&mut output, &batch.reports()[0], format).expect("render run");
        render_batch_report(&mut output, &batch, format).expect("render batch");
        render_status_report(&mut output, &status, format).expect("render status");
        render_reset_report(&mut output, &reset, format).expect("render reset");
        if format == OutputFormat::Summary {
            let summary = String::from_utf8(output).expect("UTF-8 summary");
            assert!(
                summary.contains(
                    "Error [policy_invariant] during policy evaluation: exact proof failed"
                )
            );
            assert!(summary.contains("Error path: /watchlist/demo/state.json"));
            assert!(summary.contains("Integration fault: ffhn_policy_invariant_violation"));
            assert!(summary.contains("source_reason=fetch_failed"));
            assert!(summary.contains("permanent_error=invalid_json_pointer"));
            assert!(summary.contains("Delivery outcomes: 1"));
            assert!(summary.contains("Outbox overflow: 1"));
            assert!(summary.contains(
                "Error [io] during outbox state commit [storage_full]: outbox persistence stopped"
            ));
        }
    }
}

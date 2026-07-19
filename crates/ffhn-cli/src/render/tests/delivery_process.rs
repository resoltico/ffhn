use super::super::*;

fn detail(value: serde_json::Value) -> ffhn_core::DiagnosticDetail {
    serde_json::from_value(value).expect("diagnostic detail")
}

fn delivery_failure(
    terminal: serde_json::Value,
    writer: serde_json::Value,
    stderr: serde_json::Value,
    primary: &str,
) -> ffhn_core::DiagnosticDetail {
    detail(serde_json::json!({
        "kind": "delivery",
        "operation": "delivery_process",
        "message": "delivery process did not complete successfully",
        "delivery_process": {
            "kind": "failure",
            "attempt": {"terminal": terminal, "writer": writer, "stderr": stderr},
            "primary": primary,
        },
    }))
}

fn captured_stderr(encoding: &str) -> serde_json::Value {
    let (retained_bytes_base64, original_len_bytes, truncated) = match encoding {
        "utf8" => ("cHJpdmF0ZSBzdGRlcnIgZXZpZGVuY2U=", "23", false),
        "utf8_lossy" => ("/w==", "1", false),
        "utf8_incomplete_at_retention_boundary" => ("YcM=", "3", true),
        _ => panic!("test requires a closed stderr encoding"),
    };
    serde_json::json!({
        "kind": "captured",
        "retained_bytes_base64": retained_bytes_base64,
        "original_len_bytes": original_len_bytes,
        "truncated": truncated,
    })
}

#[test]
fn summary_renders_every_closed_delivery_process_and_operation_fact_without_stderr_text() {
    let cases = [
        (
            delivery_failure(
                serde_json::json!({"kind": "not_started", "io": "not_found"}),
                serde_json::json!({"kind": "not_attempted"}),
                serde_json::json!({"kind": "absent"}),
                "spawn_failed",
            ),
            [
                "Delivery terminal: not_started (not_found)",
                "Delivery writer: not_attempted",
                "Delivery stderr: absent",
            ],
        ),
        (
            delivery_failure(
                serde_json::json!({"kind": "stdin_unavailable"}),
                serde_json::json!({"kind": "not_attempted"}),
                serde_json::json!({"kind": "absent"}),
                "stdin_unavailable",
            ),
            [
                "Delivery terminal: stdin_unavailable",
                "Delivery writer: not_attempted",
                "Delivery stderr: absent",
            ],
        ),
        (
            delivery_failure(
                serde_json::json!({"kind": "exited", "exit_code": null}),
                serde_json::json!({"kind": "panicked"}),
                captured_stderr("utf8_lossy"),
                "writer_panicked",
            ),
            [
                "Delivery terminal: exited (exit_code=unavailable)",
                "Delivery writer: panicked",
                "retained_encoding=utf8_lossy",
            ],
        ),
        (
            delivery_failure(
                serde_json::json!({"kind": "timed_out", "timeout_ms": 17}),
                serde_json::json!({"kind": "completed"}),
                serde_json::json!({
                    "kind": "read_failed",
                    "io": "broken_pipe",
                    "partial": {"retained_bytes_base64": "cHJpdmF0ZSBzdGRlcnIgZXZpZGVuY2U=", "original_len_bytes": "23", "truncated": false},
                }),
                "timed_out",
            ),
            [
                "Delivery terminal: timed_out (timeout_ms=17)",
                "Delivery stderr: read_failed (broken_pipe)",
                "Delivery primary: timed_out",
            ],
        ),
        (
            delivery_failure(
                serde_json::json!({"kind": "wait_failed", "io": "interrupted"}),
                serde_json::json!({"kind": "completed"}),
                serde_json::json!({"kind": "reader_unavailable"}),
                "wait_failed",
            ),
            [
                "Delivery terminal: wait_failed (interrupted)",
                "Delivery stderr: reader_unavailable",
                "Delivery primary: wait_failed",
            ],
        ),
        (
            delivery_failure(
                serde_json::json!({"kind": "exited", "exit_code": 1}),
                serde_json::json!({"kind": "completed"}),
                serde_json::json!({"kind": "reader_panicked"}),
                "unsuccessful_exit",
            ),
            [
                "Delivery terminal: exited (exit_code=1)",
                "Delivery stderr: reader_panicked",
                "Delivery primary: unsuccessful_exit",
            ],
        ),
    ];

    for (detail, expected_lines) in cases {
        let mut summary = Vec::new();
        render_error_detail(&mut summary, &detail).expect("delivery summary");
        let summary = String::from_utf8(summary).expect("UTF-8 summary");
        for expected in expected_lines {
            assert!(
                summary.contains(expected),
                "missing {expected:?} in {summary:?}"
            );
        }
        assert!(!summary.contains("private stderr evidence"));
    }
}

#[test]
fn summary_names_an_incomplete_utf8_code_point_at_the_retention_boundary() {
    let detail = delivery_failure(
        serde_json::json!({"kind": "exited", "exit_code": 1}),
        serde_json::json!({"kind": "completed"}),
        captured_stderr("utf8_incomplete_at_retention_boundary"),
        "unsuccessful_exit",
    );
    let mut summary = Vec::new();
    render_error_detail(&mut summary, &detail).expect("delivery summary");
    let summary = String::from_utf8(summary).expect("UTF-8 summary");

    assert!(summary.contains("retained_encoding=utf8_incomplete_at_retention_boundary"));
    assert!(summary.contains("truncated=true"));
}

#[test]
fn summary_keeps_outbox_and_observability_evidence_distinct() {
    let outbox_error = detail(serde_json::json!({
        "kind": "io",
        "operation": "outbox_state_commit",
        "message": "outbox state update did not complete",
        "io_error_class": "storage_full",
    }));
    let observability = detail(serde_json::json!({
        "kind": "delivery",
        "operation": "delivery_process",
        "message": "delivery completed but stderr capture was incomplete",
        "delivery_process": {
            "kind": "observability",
            "stderr_capture_problem": {"kind": "reader_panicked"},
        },
    }));
    let outcome: ffhn_core::DeliveryOutcome = serde_json::from_value(serde_json::json!({
        "event_id": "a".repeat(64),
        "route_id": "process",
        "event_kind": "initialized",
        "status": "delivered_uncommitted",
        "attempt_count": 1,
        "outbox_error_detail": outbox_error,
        "delivery_observability_detail": observability,
    }))
    .expect("delivery outcome");
    let report_error = detail(serde_json::json!({
        "kind": "io",
        "operation": "outbox_drain",
        "message": "outbox drain stopped",
        "io_error_class": "other",
    }));
    let mut summary = Vec::new();
    render_delivery_evidence(&mut summary, &[outcome], &[], Some(&report_error))
        .expect("delivery evidence summary");
    let summary = String::from_utf8(summary).expect("UTF-8 summary");

    assert!(summary.contains("Outbox persistence failure:"));
    assert!(summary.contains("Delivery observability:"));
    assert!(summary.contains("Outbox error:"));
    assert!(summary.contains("Delivery stderr: reader_panicked"));
}

#[test]
fn summary_labels_every_closed_diagnostic_operation() {
    for (operation, label) in [
        (ffhn_core::DiagnosticOperation::TargetLoad, "target load"),
        (
            ffhn_core::DiagnosticOperation::TargetValidation,
            "target validation",
        ),
        (
            ffhn_core::DiagnosticOperation::LockAcquire,
            "lock acquisition",
        ),
        (ffhn_core::DiagnosticOperation::StateLoad, "state load"),
        (ffhn_core::DiagnosticOperation::StateCommit, "state commit"),
        (ffhn_core::DiagnosticOperation::HttpFetch, "HTTP fetch"),
        (ffhn_core::DiagnosticOperation::FileRead, "file read"),
        (
            ffhn_core::DiagnosticOperation::JsonPointerSelection,
            "JSON Pointer selection",
        ),
        (
            ffhn_core::DiagnosticOperation::HtmlExtraction,
            "HTML extraction",
        ),
        (ffhn_core::DiagnosticOperation::ValueParse, "value parse"),
        (
            ffhn_core::DiagnosticOperation::PolicyEvaluation,
            "policy evaluation",
        ),
        (
            ffhn_core::DiagnosticOperation::DeliveryProcess,
            "delivery process",
        ),
        (ffhn_core::DiagnosticOperation::OutboxDrain, "outbox drain"),
        (
            ffhn_core::DiagnosticOperation::OutboxStateCommit,
            "outbox state commit",
        ),
    ] {
        assert_eq!(operation_label(operation), label);
    }
}

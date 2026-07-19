use super::support::*;

#[test]
fn delivery_configuration_is_operational_but_pending_records_cannot_be_rerouted() {
    let document = target("integer", "");
    let measurement_digest = document
        .contract_digest_sha256()
        .expect("measurement digest");
    let routed = mutate_target(&document, |wire| {
        wire["outbox"] = serde_json::json!({
            "max_pending": 9,
            "max_attempts": 3,
            "base_backoff_ms": 250,
            "max_backoff_ms": 500,
        });
        wire["routes"] = serde_json::json!([{
            "route_id": "run",
            "route_family": "on_run",
            "adapter": {
                "kind": "process_stdin",
                "program": crate::test_support::PROCESS_PROGRAM,
                "args": [],
                "timeout_ms": 1000,
            }
        }]);
    });
    routed.validate().expect("valid delivery configuration");
    assert_eq!(routed.routes().len(), 1);
    assert_eq!(routed.routes()[0].route_id(), "run");
    assert_eq!(
        routed.contract_digest_sha256().expect("operational digest"),
        measurement_digest
    );

    let empty_state = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        measurement_digest,
    );
    let route_id = RouteId::new("run").expect("route id");
    let payload = ProcessStdinPayload::new(
        &route_id,
        RouteFamily::OnRun,
        &TargetId::new("demo").expect("target id"),
        "Demo",
        ProcessStdinEventKey::Reset {
            contract_digest_sha256: empty_state.contract_digest_sha256().to_owned(),
        },
        "Demo: reset",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("payload");
    let event_id = payload.event_id().to_owned();
    let payload = payload.immutable_bytes().expect("payload bytes");
    let pending_state = mutate_state(&empty_state, |wire| {
        wire["outbox"] = serde_json::json!([{
            "event_id": event_id,
            "route_id": "run",
            "route_family": "on_run",
            "event_kind": "reset",
            "immutable_payload": payload,
            "attempt_count": 0,
            "next_retry_at": "2026-07-15T00:00:00Z",
        }]);
    });
    pending_state.validate().expect("valid pending record");
    pending_state
        .validate_for_target(&routed)
        .expect("matching pending route");
    let attempt_without_error = mutate_state(&pending_state, |wire| {
        wire["outbox"][0]["attempt_count"] = serde_json::json!(1);
    });
    assert!(attempt_without_error.validate().is_err());
    let error_without_attempt = mutate_state(&pending_state, |wire| {
        wire["outbox"][0]["last_error_detail"] = serde_json::json!({
            "kind": "delivery",
            "operation": "delivery_process",
            "message": "delivery process did not complete successfully",
            "delivery_process": {
                "kind": "failure",
                "attempt": {
                    "terminal": {"kind": "exited", "exit_code": 1},
                    "writer": {"kind": "completed"},
                    "stderr": {
                        "kind": "captured",
                        "retained_bytes_base64": "",
                        "original_len_bytes": "0",
                        "truncated": false
                    }
                },
                "primary": "unsuccessful_exit"
            }
        });
    });
    assert!(error_without_attempt.validate().is_err());

    let removed_route = mutate_target(&routed, |wire| {
        wire["routes"] = serde_json::json!([]);
    });
    removed_route
        .validate()
        .expect("valid route removal config");
    assert_eq!(
        removed_route
            .contract_digest_sha256()
            .expect("unchanged measurement digest"),
        pending_state.contract_digest_sha256()
    );
    assert!(pending_state.validate_for_target(&removed_route).is_err());

    let moved_route = mutate_target(&routed, |wire| {
        wire["routes"][0]["route_family"] = serde_json::json!("on_condition");
    });
    moved_route.validate().expect("valid family move config");
    assert!(pending_state.validate_for_target(&moved_route).is_err());
}

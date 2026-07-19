use super::support::*;

#[test]
fn persisted_process_payloads_are_canonical_and_bound_to_their_pending_records() {
    let routed = mutate_target(&target("integer", ""), |wire| {
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
    routed.validate().expect("routed target");
    let state = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        routed.contract_digest_sha256().expect("digest"),
    );
    let route_id = RouteId::new("run").expect("route id");
    let payload = ProcessStdinPayload::new(
        &route_id,
        RouteFamily::OnRun,
        &TargetId::new("demo").expect("target id"),
        "Demo",
        ProcessStdinEventKey::Reset {
            contract_digest_sha256: state.contract_digest_sha256().to_owned(),
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
    let payload = payload.immutable_bytes().expect("canonical payload");
    let valid = mutate_state(&state, |wire| {
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
    valid
        .validate_for_target(&routed)
        .expect("canonical matching payload");

    let retired_payload_schema = mutate_state(&valid, |wire| {
        let mut payload: serde_json::Value = serde_json::from_slice(&immutable_payload_bytes(
            &wire["outbox"][0]["immutable_payload"],
        ))
        .expect("payload JSON");
        payload["schema_version"] = serde_json::json!(2);
        wire["outbox"][0]["immutable_payload"] = serde_json::json!(
            crate::stable_json::stable_json(&payload)
                .expect("canonical retired payload")
                .into_bytes()
        );
    });
    assert!(retired_payload_schema.validate().is_err());

    let malformed = mutate_state(&valid, |wire| {
        wire["outbox"][0]["immutable_payload"] = serde_json::json!([123]);
    });
    assert!(malformed.validate().is_err());

    let mismatched = mutate_state(&valid, |wire| {
        let mut payload: serde_json::Value = serde_json::from_slice(&immutable_payload_bytes(
            &wire["outbox"][0]["immutable_payload"],
        ))
        .expect("payload JSON");
        payload["event_id"] = serde_json::json!("b".repeat(64));
        wire["outbox"][0]["immutable_payload"] = serde_json::json!(
            crate::stable_json::stable_json(&payload)
                .expect("canonical altered payload")
                .into_bytes()
        );
    });
    assert!(mismatched.validate().is_err());

    let forged_identity = mutate_state(&valid, |wire| {
        let forged_event_id = "b".repeat(64);
        let mut payload: serde_json::Value = serde_json::from_slice(&immutable_payload_bytes(
            &wire["outbox"][0]["immutable_payload"],
        ))
        .expect("payload JSON");
        payload["event_id"] = serde_json::json!(forged_event_id);
        wire["outbox"][0]["event_id"] = payload["event_id"].clone();
        wire["outbox"][0]["immutable_payload"] = serde_json::json!(
            crate::stable_json::stable_json(&payload)
                .expect("canonical forged payload")
                .into_bytes()
        );
    });
    assert!(forged_identity.validate().is_err());

    let noncanonical = mutate_state(&valid, |wire| {
        let bytes = immutable_payload_bytes(&wire["outbox"][0]["immutable_payload"])
            .into_iter()
            .chain(std::iter::once(b' '))
            .collect::<Vec<_>>();
        wire["outbox"][0]["immutable_payload"] = serde_json::json!(bytes);
    });
    assert!(noncanonical.validate().is_err());

    let inconsistent_summary = mutate_state(&valid, |wire| {
        let mut payload: serde_json::Value = serde_json::from_slice(&immutable_payload_bytes(
            &wire["outbox"][0]["immutable_payload"],
        ))
        .expect("payload JSON");
        payload["summary"] = serde_json::json!("incorrect summary");
        wire["outbox"][0]["immutable_payload"] = serde_json::json!(
            crate::stable_json::stable_json(&payload)
                .expect("canonical altered payload")
                .into_bytes()
        );
    });
    assert!(inconsistent_summary.validate().is_err());
}

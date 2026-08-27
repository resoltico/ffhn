use super::*;
use crate::{
    ConditionId,
    graph::{
        EventEnvelopeParts, EventKey, EventObservation, GraphId, MeasurementId,
        MeasurementInstanceId, SourceId, SourceInstanceId,
    },
};

fn policy() -> DeliveryPolicy {
    toml::from_str(
        r#"
max_pending = 1
max_attempts = 3
base_backoff_ms = 100
max_backoff_ms = 1000
jitter_ratio = "0.1"
"#,
    )
    .expect("policy")
}

fn route() -> GraphRoute {
    toml::from_str(&format!(
        "route_id = \"critical\"\nroute_family = \"on_condition\"\n[adapter]\n{}",
        crate::graph::test_support::process_adapter_toml(true, 1_000),
    ))
    .expect("route")
}

fn envelope(sequence: u64) -> EventEnvelope {
    let graph_id = GraphId::mint();
    let source_instance_id = SourceInstanceId::mint();
    let key = EventKey::ConditionSatisfied {
        graph_id: graph_id.clone(),
        source_id: SourceId::new("shop").expect("source id"),
        source_instance_id: source_instance_id.clone(),
        measurement_id: MeasurementId::new("price").expect("measurement id"),
        measurement_instance_id: MeasurementInstanceId::mint(),
        condition_id: ConditionId::new("changed").expect("condition id"),
        condition_defn_digest: "a".repeat(64),
        observation_seq: sequence,
    };
    EventEnvelope::new(EventEnvelopeParts {
        graph_id,
        source_instance_id,
        event_key: key,
        display_name: "Price".to_owned(),
        committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
        observation: Some(
            EventObservation::new(sequence.to_string(), sequence).expect("observation"),
        ),
        lifecycle_fact: None,
        policy_revision: Some(1),
    })
    .expect("envelope")
}

#[test]
fn admission_preserves_caller_priority_and_snapshots_policy_without_hash_reordering() {
    let policy = policy();
    let route = route();
    let first = OutboxAdmission::admit(
        &[],
        [envelope(1)],
        std::slice::from_ref(&route),
        Some(&policy),
        "2026-08-25T00:00:00Z",
    )
    .expect("first admission");
    assert_eq!(first.records().len(), 1);
    let second = OutboxAdmission::admit(
        first.records(),
        [envelope(2)],
        std::slice::from_ref(&route),
        Some(&policy),
        "2026-08-25T00:01:00Z",
    )
    .expect("bounded admission");
    assert!(second.records().is_empty());
    assert_eq!(second.overflow().len(), 1);
    assert_eq!(second.overflow()[0].route_id, *route.route_id());
    assert_eq!(
        second.overflow()[0].event_kind(),
        EventKind::ConditionSatisfied
    );
    assert_eq!(
        second.overflow()[0]
            .condition_id()
            .expect("condition overflow")
            .as_str(),
        "changed"
    );
}

#[test]
fn admission_skips_routes_that_do_not_accept_the_event_family() {
    let policy = policy();
    let source_route: GraphRoute = toml::from_str(&format!(
        "route_id = \"source\"\nroute_family = \"on_source\"\n[adapter]\n{}",
        crate::graph::test_support::process_adapter_toml(true, 1_000),
    ))
    .expect("route");
    let admission = OutboxAdmission::admit(
        &[],
        [envelope(1)],
        &[source_route],
        Some(&policy),
        "2026-08-25T00:00:00Z",
    )
    .expect("admission");
    assert!(admission.records().is_empty());
}

#[test]
fn dead_letter_requires_exact_snapshot_attempt_exhaustion_and_preserves_every_failure() {
    let policy = policy();
    let route = route();
    let admission = OutboxAdmission::admit(
        &[],
        [envelope(1)],
        std::slice::from_ref(&route),
        Some(&policy),
        "2026-08-25T00:00:00Z",
    )
    .expect("admission");
    let mut record = admission.records()[0].clone();
    for second in 1..=3 {
        let at = format!("2026-08-25T00:00:0{second}Z");
        record
            .record_failure(
                at.clone(),
                DeliveryAttemptFailure::Process {
                    message: "exit status 1".to_owned(),
                },
                at,
            )
            .expect("failure append");
    }
    let terminal_pending = serde_json::to_value(&record).expect("terminal pending wire");
    assert!(serde_json::from_value::<DeliveryRecord>(terminal_pending).is_err());
    let letter = record.into_dead_letter().expect("terminal letter");
    assert_eq!(letter.record().attempts().len(), 3);
    assert!(
        serde_json::from_value::<DeadLetter>(serde_json::to_value(letter).expect("wire")).is_ok()
    );
}

#[test]
fn retry_successor_requires_the_exact_snapshot_and_attempt_prefix() {
    let admission = OutboxAdmission::admit(
        &[],
        [envelope(1)],
        &[route()],
        Some(&policy()),
        "2026-08-25T00:00:00Z",
    )
    .expect("admission");
    let prior = admission.records()[0].clone();
    let mut retry = prior.clone();
    retry
        .record_failure(
            "2026-08-25T00:00:01Z".to_owned(),
            DeliveryAttemptFailure::Process {
                message: "failed".to_owned(),
            },
            "2026-08-25T00:00:02Z".to_owned(),
        )
        .expect("retry");
    assert!(retry.is_single_attempt_successor_of(&prior));

    let mut wire = serde_json::to_value(&retry).expect("retry wire");
    wire["delivery_policy"]["max_pending"] = serde_json::json!(2);
    let altered: DeliveryRecord = serde_json::from_value(wire).expect("valid altered record");
    assert!(!altered.is_single_attempt_successor_of(&prior));

    let mut changed = retry.clone();
    changed.envelope = envelope(2);
    assert!(!changed.is_single_attempt_successor_of(&prior));
    let mut changed = retry.clone();
    changed.route_id = GraphRouteId::new("other").expect("route");
    assert!(!changed.is_single_attempt_successor_of(&prior));
    let mut changed = retry.clone();
    changed.route_family = GraphRouteFamily::OnSource;
    assert!(!changed.is_single_attempt_successor_of(&prior));
    let mut changed = retry.clone();
    let fixture = crate::graph::test_support::failing_process();
    changed.adapter = GraphDeliveryAdapter::ProcessStdin {
        program: fixture.program,
        args: fixture.args,
        timeout_ms: 1_000,
    };
    assert!(!changed.is_single_attempt_successor_of(&prior));
    let mut changed = retry.clone();
    changed.attempts.clear();
    assert!(!changed.is_single_attempt_successor_of(&prior));
    let prior_with_attempt = retry.clone();
    let mut successor = prior_with_attempt.clone();
    successor
        .record_failure(
            "2026-08-25T00:00:03Z".to_owned(),
            DeliveryAttemptFailure::Process {
                message: "second".to_owned(),
            },
            "2026-08-25T00:00:04Z".to_owned(),
        )
        .expect("second retry");
    assert!(successor.is_single_attempt_successor_of(&prior_with_attempt));
    successor.attempts[0].attempt = 2;
    assert!(!successor.is_single_attempt_successor_of(&prior_with_attempt));
}

#[test]
fn durable_outbox_documents_reject_every_crossed_wire_and_expose_complete_snapshots() {
    let route = route();
    let policy = policy();
    let admission = OutboxAdmission::admit(
        &[],
        [envelope(1)],
        std::slice::from_ref(&route),
        Some(&policy),
        "2026-08-25T00:00:00Z",
    )
    .expect("admission");
    let record = &admission.records()[0];
    assert_eq!(record.route_family(), GraphRouteFamily::OnCondition);
    assert_eq!(record.envelope().event_id(), record.event_id());
    assert_eq!(record.adapter(), route.adapter());
    assert_eq!(record.delivery_policy(), &policy);
    assert!(record.attempts().is_empty());
    assert_eq!(record.next_retry_at_utc(), "2026-08-25T00:00:00Z");
    assert!(record.storage_file_name().ends_with("--critical.json"));
    assert!(record.clone().into_dead_letter().is_err());

    assert!(
        OutboxAdmission::admit(&[], [], &[], None, "2026-08-25T00:00:00Z")
            .expect("route-free admission")
            .records()
            .is_empty()
    );
    assert!(
        OutboxAdmission::admit(
            &[],
            [],
            std::slice::from_ref(&route),
            None,
            "2026-08-25T00:00:00Z",
        )
        .is_err()
    );
    assert!(OutboxAdmission::admit(&[], [], &[], Some(&policy), "not-a-time").is_err());

    let base = serde_json::to_value(record).expect("record wire");
    let mut invalid_wires = Vec::new();
    for (pointer, value) in [
        ("/schema_name", serde_json::json!("foreign.record")),
        ("/schema_version", serde_json::json!(99)),
        ("/route_family", serde_json::json!("on_source")),
        ("/next_retry_at_utc", serde_json::json!("not-a-time")),
    ] {
        let mut wire = base.clone();
        *wire.pointer_mut(pointer).expect("pointer") = value;
        invalid_wires.push(wire);
    }
    for wire in invalid_wires {
        assert!(serde_json::from_value::<DeliveryRecord>(wire).is_err());
    }

    let failures = [
        DeliveryAttemptFailure::SecretUnavailable {
            env: " ".to_owned(),
        },
        DeliveryAttemptFailure::Process {
            message: String::new(),
        },
        DeliveryAttemptFailure::Process {
            message: "line one\nline two".to_owned(),
        },
        DeliveryAttemptFailure::Process {
            message: "x".repeat(2_049),
        },
        DeliveryAttemptFailure::HttpWebhook {
            message: "success is not a failure".to_owned(),
            status: Some(204),
        },
        DeliveryAttemptFailure::HttpWebhook {
            message: String::new(),
            status: Some(500),
        },
    ];
    for failure in failures {
        let mut candidate = record.clone();
        assert!(
            candidate
                .record_failure(
                    "2026-08-25T00:00:01Z".to_owned(),
                    failure,
                    "2026-08-25T00:00:02Z".to_owned(),
                )
                .is_err()
        );
    }
    let mut candidate = record.clone();
    assert!(
        candidate
            .record_failure(
                "2026-08-25T00:00:02Z".to_owned(),
                DeliveryAttemptFailure::SecretUnavailable {
                    env: "TOKEN".to_owned(),
                },
                "2026-08-25T00:00:01Z".to_owned(),
            )
            .is_err()
    );
    assert!(
        candidate
            .record_failure(
                "bad".to_owned(),
                DeliveryAttemptFailure::Process {
                    message: "failed".to_owned(),
                },
                "2026-08-25T00:00:03Z".to_owned(),
            )
            .is_err()
    );

    let mut retried = record.clone();
    retried
        .record_failure(
            "2026-08-25T00:00:02Z".to_owned(),
            DeliveryAttemptFailure::HttpWebhook {
                message: "server error".to_owned(),
                status: Some(500),
            },
            "2026-08-25T00:00:03Z".to_owned(),
        )
        .expect("first retry");
    let retry_wire = serde_json::to_value(&retried).expect("retry wire");
    for (pointer, value) in [
        ("/attempts/0/attempt", serde_json::json!(2)),
        ("/attempts/0/attempted_at_utc", serde_json::json!("bad")),
        (
            "/next_retry_at_utc",
            serde_json::json!("2026-08-25T00:00:01Z"),
        ),
    ] {
        let mut wire = retry_wire.clone();
        *wire.pointer_mut(pointer).expect("pointer") = value;
        assert!(serde_json::from_value::<DeliveryRecord>(wire).is_err());
    }

    retried
        .record_failure(
            "2026-08-25T00:00:04Z".to_owned(),
            DeliveryAttemptFailure::Process {
                message: "again".to_owned(),
            },
            "2026-08-25T00:00:05Z".to_owned(),
        )
        .expect("second retry");
    let mut non_monotonic = serde_json::to_value(&retried).expect("retry wire");
    non_monotonic["attempts"][1]["attempted_at_utc"] = serde_json::json!("2026-08-25T00:00:01Z");
    assert!(serde_json::from_value::<DeliveryRecord>(non_monotonic).is_err());
    let mut simultaneous = serde_json::to_value(&retried).expect("retry wire");
    simultaneous["attempts"][1]["attempted_at_utc"] =
        simultaneous["attempts"][0]["attempted_at_utc"].clone();
    serde_json::from_value::<DeliveryRecord>(simultaneous)
        .expect("equal attempt timestamps are monotonic");

    retried
        .record_failure(
            "2026-08-25T00:00:06Z".to_owned(),
            DeliveryAttemptFailure::Process {
                message: "terminal".to_owned(),
            },
            "2026-08-25T00:00:07Z".to_owned(),
        )
        .expect("terminal retry");
    assert!(retried.validate().is_err());
    let letter = retried.into_dead_letter().expect("dead letter");
    assert_eq!(letter.record().attempts().len(), 3);
    let letter_wire = serde_json::to_value(&letter).expect("letter wire");
    for (pointer, value) in [
        ("/schema_name", serde_json::json!("foreign.dead_letter")),
        ("/schema_version", serde_json::json!(99)),
        ("/terminal_attempt", serde_json::json!(2)),
    ] {
        let mut wire = letter_wire.clone();
        *wire.pointer_mut(pointer).expect("pointer") = value;
        assert!(serde_json::from_value::<DeadLetter>(wire).is_err());
    }
    let mut below_policy = letter_wire.clone();
    below_policy["record"]["attempts"]
        .as_array_mut()
        .expect("attempts")
        .pop();
    below_policy["terminal_attempt"] = serde_json::json!(2);
    assert!(serde_json::from_value::<DeadLetter>(below_policy).is_err());

    let mut too_many = letter.record().clone();
    too_many.attempts.push(DeliveryAttempt {
        attempt: 4,
        attempted_at_utc: "2026-08-25T00:00:08Z".to_owned(),
        failure: DeliveryAttemptFailure::Process {
            message: "extra".to_owned(),
        },
    });
    assert!(too_many.validate_common().is_err());
}

#[test]
fn overflow_facts_bind_hex_identity_scope_and_route_family() {
    let first = OutboxAdmission::admit(
        &[],
        [envelope(1), envelope(2)],
        &[route()],
        Some(&policy()),
        "2026-08-25T00:00:00Z",
    )
    .expect("bounded admission");
    let fact = &first.overflow()[0];
    assert_eq!(fact.event_id().len(), 64);
    assert_eq!(fact.route_id().as_str(), "critical");
    fact.validate().expect("valid overflow");
    let mut digits = fact.clone();
    digits.event_id = "0".repeat(64);
    digits.validate().expect("digit digest");

    let base = serde_json::to_value(fact).expect("overflow wire");
    for (pointer, value) in [
        ("/event_id", serde_json::json!("A".repeat(64))),
        ("/event_id", serde_json::json!("a".repeat(63))),
        ("/route_family", serde_json::json!("on_source")),
        ("/condition_id", serde_json::Value::Null),
    ] {
        let mut wire = base.clone();
        *wire.pointer_mut(pointer).expect("pointer") = value;
        let candidate: OutboxOverflowFact = serde_json::from_value(wire).expect("wire shape");
        assert!(candidate.validate().is_err());
    }
}

#[test]
fn admission_deduplicates_existing_keys_and_rejects_cross_family_record_construction() {
    let route = route();
    let policy = policy();
    let event = envelope(1);
    let first = OutboxAdmission::admit(
        &[],
        [event.clone()],
        std::slice::from_ref(&route),
        Some(&policy),
        "2026-08-25T00:00:00Z",
    )
    .expect("first");
    let duplicate = OutboxAdmission::admit(
        first.records(),
        [event.clone()],
        std::slice::from_ref(&route),
        Some(&policy),
        "2026-08-25T00:00:01Z",
    )
    .expect("duplicate");
    assert!(duplicate.records().is_empty());
    assert!(duplicate.overflow().is_empty());

    let source_route: GraphRoute = toml::from_str(&format!(
        "route_id = \"source\"\nroute_family = \"on_source\"\n[adapter]\n{}",
        crate::graph::test_support::process_adapter_toml(true, 1_000),
    ))
    .expect("source route");
    assert!(DeliveryRecord::new(event, &source_route, &policy, "2026-08-25T00:00:00Z").is_err());
}

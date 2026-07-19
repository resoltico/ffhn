use super::support::*;

#[test]
fn durable_outbox_retries_stored_bytes_terminally_fails_and_never_evicts_pending_records() {
    let (_temporary, paths) = fixture_paths();
    let source = paths.target_dir().join("source.json");
    let log = paths.target_dir().join("deliveries.jsonl");
    let changed_condition = "[[conditions]]\ncondition_id = \"changed\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"";
    write_portable_delivery_target(&paths, "write", Some(&log), 2, 2, changed_condition);
    fs::write(&source, r#"{"value":10}"#).expect("baseline");
    let initialized = run_once(&paths).expect("initialized");
    assert!(initialized.delivery_outcomes().is_empty());

    fs::write(&source, r#"{"value":20}"#).expect("first condition event");
    let delivered = run_once(&paths).expect("delivered condition event");
    assert_eq!(delivered.delivery_outcomes().len(), 1);
    assert_eq!(
        delivered.delivery_outcomes()[0].status(),
        DeliveryStatus::Delivered
    );
    assert_eq!(
        delivered.delivery_outcomes()[0].event_kind(),
        DeliveryEventKind::ConditionSatisfied
    );
    assert_eq!(
        delivered.delivery_outcomes()[0].condition_id(),
        Some("changed")
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(paths.state_file()).expect("state"))
            .expect("state JSON")["outbox"],
        serde_json::json!([])
    );

    write_portable_delivery_target(&paths, "fail", None, 2, 2, changed_condition);
    fs::write(&source, r#"{"value":30}"#).expect("retryable event");
    let retried = run_once(&paths).expect("retry scheduled");
    assert_eq!(retried.delivery_outcomes().len(), 1);
    assert_eq!(
        retried.delivery_outcomes()[0].status(),
        DeliveryStatus::RetryScheduled
    );
    let retry_state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("retry state"))
            .expect("retry state JSON");
    let event_id = retry_state["outbox"][0]["event_id"]
        .as_str()
        .expect("event id")
        .to_owned();
    let stored_payload = retry_state["outbox"][0]["immutable_payload"]
        .as_array()
        .expect("payload bytes")
        .iter()
        .map(|value| {
            u8::try_from(value.as_u64().expect("byte")).expect("payload entry within byte range")
        })
        .collect::<Vec<_>>();

    make_pending_outbox_due(&paths);
    write_portable_delivery_target(&paths, "write", Some(&log), 2, 2, changed_condition);
    let retry_delivered = run_once(&paths).expect("retry delivery");
    assert_eq!(retry_delivered.delivery_outcomes().len(), 1);
    assert_eq!(
        retry_delivered.delivery_outcomes()[0].status(),
        DeliveryStatus::Delivered
    );
    assert_eq!(retry_delivered.delivery_outcomes()[0].event_id(), event_id);
    let log_lines = fs::read_to_string(&log)
        .expect("delivery log")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(log_lines.len(), 2);
    assert_eq!(log_lines[1].as_bytes(), stored_payload.as_slice());

    write_portable_delivery_target(&paths, "fail", None, 2, 1, changed_condition);
    fs::write(&source, r#"{"value":40}"#).expect("terminal event");
    let terminal = run_once(&paths).expect("terminal delivery");
    assert_eq!(terminal.delivery_outcomes().len(), 1);
    assert_eq!(
        terminal.delivery_outcomes()[0].status(),
        DeliveryStatus::DeadLettered
    );
    assert!(terminal.has_delivery_failure());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(paths.state_file()).expect("state"))
            .expect("state JSON")["outbox"],
        serde_json::json!([])
    );

    let two_conditions = "[[conditions]]\ncondition_id = \"critical\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"\n\n[[conditions]]\ncondition_id = \"trivial\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"";
    let (_temporary, overflow_paths) = fixture_paths();
    let overflow_source = overflow_paths.target_dir().join("source.json");
    write_portable_delivery_target(&overflow_paths, "fail", None, 1, 5, two_conditions);
    fs::write(&overflow_source, r#"{"value":1}"#).expect("overflow baseline");
    run_once(&overflow_paths).expect("overflow baseline run");
    fs::write(&overflow_source, r#"{"value":2}"#).expect("overflow event");
    let overflow = run_once(&overflow_paths).expect("overflow run");
    assert_eq!(overflow.outbox_overflow().len(), 1);
    assert_eq!(
        overflow.outbox_overflow()[0].event_kind(),
        DeliveryEventKind::ConditionSatisfied
    );
    assert_eq!(
        overflow.outbox_overflow()[0].condition_id(),
        Some("trivial")
    );
    let overflow_state: serde_json::Value =
        serde_json::from_slice(&fs::read(overflow_paths.state_file()).expect("overflow state"))
            .expect("overflow JSON");
    assert_eq!(
        overflow_state["outbox"].as_array().expect("outbox").len(),
        1
    );
    assert_eq!(overflow_state["outbox"][0]["condition_id"], "critical");
}

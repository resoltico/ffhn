use std::fs;

use super::*;
use crate::ConditionId;
use crate::graph::{
    DeliveryPolicy, EventEnvelope, EventEnvelopeParts, EventKey, EventObservation, GraphId,
    GraphRoute, MeasurementInstanceId, OutboxAdmission, SourceId, SourceIdentity, SourceInstanceId,
    TrustedGraphRoot,
};

fn record(source_instance_id: SourceInstanceId) -> DeliveryRecord {
    let policy: DeliveryPolicy = toml::from_str(
            "max_pending = 1\nmax_attempts = 2\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n",
        )
        .expect("policy");
    let route: GraphRoute = toml::from_str(&format!(
        "route_id = \"source\"\nroute_family = \"on_source\"\n[adapter]\n{}",
        crate::graph::test_support::process_adapter_toml(true, 1_000),
    ))
    .expect("route");
    let graph_id = GraphId::mint();
    let envelope = EventEnvelope::new(EventEnvelopeParts {
        graph_id: graph_id.clone(),
        source_instance_id: source_instance_id.clone(),
        event_key: EventKey::SourceLifecycle {
            graph_id,
            source_id: SourceId::new("demo").expect("source id"),
            source_instance_id,
            event_kind: super::super::EventKind::SourceInitialized,
            source_representation_digest: "a".repeat(64),
        },
        display_name: "Demo".to_owned(),
        committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
        observation: None,
        lifecycle_fact: Some("initialized".to_owned()),
        policy_revision: None,
    })
    .expect("envelope");
    OutboxAdmission::admit(
        &[],
        [envelope],
        &[route],
        Some(&policy),
        "2026-08-25T00:00:00Z",
    )
    .expect("admission")
    .records()[0]
        .clone()
}

fn measurement_record(
    source_instance_id: SourceInstanceId,
    measurement_id: MeasurementId,
    measurement_instance_id: MeasurementInstanceId,
) -> DeliveryRecord {
    let policy: DeliveryPolicy = toml::from_str(
            "max_pending = 1\nmax_attempts = 2\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n",
        )
        .expect("policy");
    let route: GraphRoute = toml::from_str(&format!(
        "route_id = \"condition\"\nroute_family = \"on_condition\"\n[adapter]\n{}",
        crate::graph::test_support::process_adapter_toml(true, 1_000),
    ))
    .expect("route");
    let graph_id = GraphId::mint();
    let envelope = EventEnvelope::new(EventEnvelopeParts {
        graph_id: graph_id.clone(),
        source_instance_id: source_instance_id.clone(),
        event_key: EventKey::ConditionSatisfied {
            graph_id,
            source_id: SourceId::new("demo").expect("source"),
            source_instance_id,
            measurement_id,
            measurement_instance_id,
            condition_id: ConditionId::new("changed").expect("condition"),
            condition_defn_digest: "a".repeat(64),
            observation_seq: 1,
        },
        display_name: "Measurement".to_owned(),
        committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
        observation: Some(EventObservation::new("1".to_owned(), 1).expect("observation")),
        lifecycle_fact: None,
        policy_revision: Some(1),
    })
    .expect("envelope");
    OutboxAdmission::admit(
        &[],
        [envelope],
        &[route],
        Some(&policy),
        "2026-08-25T00:00:00Z",
    )
    .expect("admission")
    .records()[0]
        .clone()
}

#[test]
fn delivered_source_record_is_removed_only_with_the_successor_source_generation() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn/source-outbox")).expect("outbox");
    let graph = TrustedGraphRoot::open(super::super::GraphPaths::new(root)).expect("graph");
    let source = graph
        .open_source(SourceId::new("demo").expect("source id"))
        .expect("source");
    let identity = SourceIdentity::fresh();
    source.write_identity(&identity).expect("identity");
    let storage = source.open_storage().expect("storage");
    let state = SourceState::fresh(identity.source_instance_id().clone());
    storage.write_source_state(&state).expect("state");
    let record = record(identity.source_instance_id().clone());
    fs::write(
        source
            .paths()
            .source_outbox_dir()
            .join(record.storage_file_name()),
        format!(
            "{}\n",
            crate::stable_json::stable_json(&record).expect("record")
        ),
    )
    .expect("record write");
    let next = commit_delivery_result(
        &source,
        &storage,
        &state,
        &OutboxOwner::Source,
        &record,
        DeliveryExecution::Delivered,
    )
    .expect("delivery commit");
    assert_eq!(next.generation(), 2);
    assert!(
        storage
            .read_source_delivery_records()
            .expect("records")
            .is_empty()
    );
    assert_eq!(
        storage
            .read_source_state()
            .expect("state read")
            .expect("state"),
        next
    );
}

#[test]
fn retry_and_dead_letter_results_replace_then_remove_the_exact_pending_record() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn/source-outbox")).expect("outbox");
    let graph = TrustedGraphRoot::open(super::super::GraphPaths::new(root)).expect("graph");
    let source = graph
        .open_source(SourceId::new("demo").expect("source id"))
        .expect("source");
    let identity = SourceIdentity::fresh();
    source.write_identity(&identity).expect("identity");
    let storage = source.open_storage().expect("storage");
    let state = SourceState::fresh(identity.source_instance_id().clone());
    storage.write_source_state(&state).expect("state");
    let prior = record(identity.source_instance_id().clone());
    fs::write(
        source
            .paths()
            .source_outbox_dir()
            .join(prior.storage_file_name()),
        format!(
            "{}\n",
            crate::stable_json::stable_json(&prior).expect("record")
        ),
    )
    .expect("pending");
    let mut retry = prior.clone();
    retry
        .record_failure(
            "2026-08-25T00:00:01Z".to_owned(),
            super::super::DeliveryAttemptFailure::Process {
                message: "failed".to_owned(),
            },
            "2026-08-25T00:00:02Z".to_owned(),
        )
        .expect("retry");
    let state = commit_delivery_result(
        &source,
        &storage,
        &state,
        &OutboxOwner::Source,
        &prior,
        DeliveryExecution::Retry(retry.clone()),
    )
    .expect("retry commit");
    assert_eq!(
        storage.read_source_delivery_records().expect("records"),
        [retry.clone()]
    );

    let mut terminal = retry.clone();
    terminal
        .record_failure(
            "2026-08-25T00:00:03Z".to_owned(),
            super::super::DeliveryAttemptFailure::Process {
                message: "failed again".to_owned(),
            },
            "2026-08-25T00:00:04Z".to_owned(),
        )
        .expect("terminal");
    let letter = terminal.into_dead_letter().expect("letter");
    let state = commit_delivery_result(
        &source,
        &storage,
        &state,
        &OutboxOwner::Source,
        &retry,
        DeliveryExecution::DeadLetter(letter.clone()),
    )
    .expect("dead letter commit");
    assert_eq!(state.generation(), 3);
    assert!(
        storage
            .read_source_delivery_records()
            .expect("records")
            .is_empty()
    );
    assert_eq!(
        storage.read_source_dead_letters().expect("letters"),
        [letter]
    );
}

#[test]
fn delivery_commit_rejects_missing_pending_foreign_lineage_owner_and_mutated_retry() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn/source-outbox")).expect("outbox");
    let graph = TrustedGraphRoot::open(super::super::GraphPaths::new(root)).expect("graph");
    let source = graph
        .open_source(SourceId::new("demo").expect("source id"))
        .expect("source");
    let identity = SourceIdentity::fresh();
    source.write_identity(&identity).expect("identity");
    let storage = source.open_storage().expect("storage");
    let state = SourceState::fresh(identity.source_instance_id().clone());
    storage.write_source_state(&state).expect("state");
    let prior = record(identity.source_instance_id().clone());
    assert!(
        commit_delivery_result(
            &source,
            &storage,
            &state,
            &OutboxOwner::Source,
            &prior,
            DeliveryExecution::Delivered,
        )
        .is_err()
    );
    assert!(
        commit_delivery_result(
            &source,
            &storage,
            &SourceState::fresh(SourceInstanceId::mint()),
            &OutboxOwner::Source,
            &prior,
            DeliveryExecution::Delivered,
        )
        .is_err()
    );
    assert!(
        validate_owner(
            &source,
            &OutboxOwner::Measurement(MeasurementId::new("price").expect("measurement")),
            &prior,
        )
        .is_err()
    );
    fs::write(
        source
            .paths()
            .source_outbox_dir()
            .join(prior.storage_file_name()),
        format!(
            "{}\n",
            crate::stable_json::stable_json(&prior).expect("record")
        ),
    )
    .expect("pending");
    assert!(
        commit_delivery_result(
            &source,
            &storage,
            &state,
            &OutboxOwner::Source,
            &prior,
            DeliveryExecution::Retry(prior.clone()),
        )
        .is_err()
    );
}

#[test]
fn source_and_measurement_owner_paths_and_dead_letter_identity_are_exact() {
    let instance = SourceInstanceId::mint();
    let prior = record(instance.clone());
    let measurement_id = MeasurementId::new("price").expect("measurement");
    assert!(
        pending_path(&OutboxOwner::Source, &prior)
            .expect("source pending")
            .as_str()
            .starts_with("source-outbox/")
    );
    assert!(
        pending_path(&OutboxOwner::Measurement(measurement_id.clone()), &prior)
            .expect("measurement pending")
            .as_str()
            .starts_with("measurements/price/outbox/")
    );
    assert!(
        dead_letter_path(&OutboxOwner::Source, &prior)
            .expect("source letter")
            .as_str()
            .starts_with("dead-letters/")
    );
    assert!(
        dead_letter_path(&OutboxOwner::Measurement(measurement_id), &prior)
            .expect("measurement letter")
            .as_str()
            .starts_with("measurements/price/dead-letters/")
    );
    assert!(stage("test").expect("stage").is_staged());

    let mut terminal = record(instance);
    for second in 1..=2 {
        terminal
            .record_failure(
                format!("2026-08-25T00:00:0{second}Z"),
                super::super::DeliveryAttemptFailure::Process {
                    message: "failed".to_owned(),
                },
                format!("2026-08-25T00:00:0{second}Z"),
            )
            .expect("failure");
    }
    let letter = terminal.into_dead_letter().expect("letter");
    assert!(validate_dead_letter_identity(&letter, &prior).is_err());

    let temporary = tempfile::tempdir().expect("temporary");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/other")).expect("source");
    fs::create_dir(root.join("sources/demo")).expect("demo source");
    let graph = TrustedGraphRoot::open(super::super::GraphPaths::new(root)).expect("graph");
    let other = graph
        .open_source(SourceId::new("other").expect("source"))
        .expect("source");
    assert!(validate_owner(&other, &OutboxOwner::Source, &prior).is_err());
    let measurement_record = measurement_record(
        SourceInstanceId::mint(),
        MeasurementId::new("price").expect("measurement"),
        MeasurementInstanceId::mint(),
    );
    let demo = graph
        .open_source(SourceId::new("demo").expect("source"))
        .expect("source");
    assert!(
        validate_owner(
            &demo,
            &OutboxOwner::Measurement(MeasurementId::new("other").expect("measurement")),
            &measurement_record,
        )
        .is_err()
    );
}

#[test]
fn measurement_delivery_commit_requires_and_touches_its_exact_identity_authority() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn/measurements/price/outbox")).expect("outbox");
    let graph = TrustedGraphRoot::open(super::super::GraphPaths::new(root)).expect("graph");
    let source = graph
        .open_source(SourceId::new("demo").expect("source"))
        .expect("source");
    let mut identity = SourceIdentity::fresh();
    source.write_identity(&identity).expect("identity");
    let storage = source.open_storage().expect("storage");
    let state = SourceState::fresh(identity.source_instance_id().clone());
    storage.write_source_state(&state).expect("state");
    let measurement_id = MeasurementId::new("price").expect("measurement");
    let measurement_instance = MeasurementInstanceId::mint();
    let record = measurement_record(
        identity.source_instance_id().clone(),
        measurement_id.clone(),
        measurement_instance.clone(),
    );
    fs::write(
        source
            .paths()
            .measurement_outbox_dir(&measurement_id)
            .join(record.storage_file_name()),
        format!(
            "{}\n",
            crate::stable_json::stable_json(&record).expect("record")
        ),
    )
    .expect("record");
    assert!(
        commit_delivery_result(
            &source,
            &storage,
            &state,
            &OutboxOwner::Measurement(measurement_id.clone()),
            &record,
            DeliveryExecution::Delivered,
        )
        .is_err()
    );

    identity
        .register_measurement_instance(measurement_id.clone(), measurement_instance)
        .expect("measurement identity");
    source.write_identity(&identity).expect("identity");
    let next = commit_delivery_result(
        &source,
        &storage,
        &state,
        &OutboxOwner::Measurement(measurement_id),
        &record,
        DeliveryExecution::Delivered,
    )
    .expect("measurement commit");
    assert_eq!(next.generation(), 2);
}

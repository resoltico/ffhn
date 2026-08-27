use std::fs;

use super::*;
use crate::graph::{
    DeliveryPolicy, EventEnvelope, EventEnvelopeParts, EventKey, EventKind, GraphId, GraphPaths,
    GraphRoute, OutboxAdmission, SourceIdentity, SourceState,
};

fn failing_source_record() -> super::super::DeliveryRecord {
    let policy: DeliveryPolicy = toml::from_str(
        "max_pending = 1\nmax_attempts = 2\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n",
    )
    .expect("policy");
    let route: GraphRoute = toml::from_str(&format!(
        "route_id = \"source\"\nroute_family = \"on_source\"\n[adapter]\n{}",
        crate::graph::test_support::process_adapter_toml(false, 1_000),
    ))
    .expect("route");
    let graph_id = GraphId::mint();
    let source_instance_id = super::super::SourceInstanceId::mint();
    let envelope = EventEnvelope::new(EventEnvelopeParts {
        graph_id: graph_id.clone(),
        source_instance_id: source_instance_id.clone(),
        event_key: EventKey::SourceLifecycle {
            graph_id,
            source_id: SourceId::new("demo").expect("source"),
            source_instance_id,
            event_kind: EventKind::SourceInitialized,
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

#[test]
fn empty_source_outbox_is_idle_after_source_lineage_is_ready() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(root.clone()),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn")).expect("source storage");
    let source = graph
        .open_source(SourceId::new("demo").expect("source id"))
        .expect("source");
    let identity = SourceIdentity::fresh();
    source.write_identity(&identity).expect("identity");
    source
        .open_storage()
        .expect("storage")
        .write_source_state(&SourceState::fresh(identity.source_instance_id().clone()))
        .expect("state");
    assert_eq!(
        drain_source_outbox_once(
            &graph,
            SourceId::new("demo").expect("source id"),
            "2026-08-25T00:00:00Z".to_owned()
        )
        .expect("drain"),
        DrainResult::Idle
    );
}

#[test]
fn drain_reports_skipped_locked_so_the_agent_can_defer_that_capability() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph = TrustedGraphRoot::initialize(
        super::super::GraphPaths::new(root.clone()),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    std::fs::create_dir_all(root.join("sources/demo")).expect("source directory");
    let source_id = SourceId::new("demo").expect("source id");
    let source = graph.open_source(source_id.clone()).expect("source");
    let _lease = source
        .try_acquire_write_lease()
        .expect("source lock")
        .expect("available source lock");
    assert_eq!(
        drain_source_outbox_once(&graph, source_id, "2026-08-25T00:00:00Z".to_owned())
            .expect("contention result"),
        DrainResult::Locked
    );
    assert_eq!(
        drain_measurement_outbox_once(
            &graph,
            SourceId::new("demo").expect("source"),
            super::super::MeasurementId::new("price").expect("measurement"),
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("measurement contention"),
        DrainResult::Locked
    );
}

#[test]
fn due_source_record_executes_and_is_removed_with_the_delivery_successor_generation() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(root.clone()),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn/source-outbox")).expect("outbox");
    let source = graph
        .open_source(SourceId::new("demo").expect("source id"))
        .expect("source");
    let identity = SourceIdentity::fresh();
    source.write_identity(&identity).expect("identity");
    let state = SourceState::fresh(identity.source_instance_id().clone());
    let storage = source.open_storage().expect("storage");
    storage.write_source_state(&state).expect("state");
    let policy: DeliveryPolicy = toml::from_str("max_pending = 1\nmax_attempts = 2\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n").expect("policy");
    let route: GraphRoute = toml::from_str(&format!(
        "route_id = \"source\"\nroute_family = \"on_source\"\n[adapter]\n{}",
        crate::graph::test_support::process_adapter_toml(true, 1_000),
    ))
    .expect("route");
    let graph_id = GraphId::mint();
    let envelope = EventEnvelope::new(EventEnvelopeParts {
        graph_id: graph_id.clone(),
        source_instance_id: identity.source_instance_id().clone(),
        event_key: EventKey::SourceLifecycle {
            graph_id,
            source_id: SourceId::new("demo").expect("source id"),
            source_instance_id: identity.source_instance_id().clone(),
            event_kind: EventKind::SourceInitialized,
            source_representation_digest: "a".repeat(64),
        },
        display_name: "Demo".to_owned(),
        committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
        observation: None,
        lifecycle_fact: Some("initialized".to_owned()),
        policy_revision: None,
    })
    .expect("envelope");
    let record = OutboxAdmission::admit(
        &[],
        [envelope],
        &[route],
        Some(&policy),
        "2026-08-25T00:00:00Z",
    )
    .expect("admission")
    .records()[0]
        .clone();
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
    assert_eq!(
        drain_source_outbox_once(
            &graph,
            SourceId::new("demo").expect("source id"),
            "2026-08-25T00:00:00Z".to_owned()
        )
        .expect("drain"),
        DrainResult::Delivered
    );
    assert!(
        storage
            .read_source_delivery_records()
            .expect("outbox")
            .is_empty()
    );
    assert_eq!(
        storage
            .read_source_state()
            .expect("state")
            .expect("state")
            .generation(),
        2
    );
}

#[test]
fn drain_result_vocabulary_and_due_selection_are_complete() {
    for (result, spelling) in [
        (DrainResult::Locked, "skipped_locked"),
        (DrainResult::Idle, "idle"),
        (DrainResult::Delivered, "delivered"),
        (DrainResult::Retrying, "retrying"),
        (DrainResult::DeadLettered, "dead_lettered"),
        (DrainResult::Unreachable, "unreachable"),
    ] {
        assert_eq!(result.as_str(), spelling);
    }
    assert_eq!(
        result_for(&DeliveryExecution::Delivered),
        DrainResult::Delivered
    );
    let now = time::OffsetDateTime::parse(
        "2026-08-25T00:00:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .expect("time");
    assert!(first_due(Vec::new(), now).expect("empty").is_none());
    let before = time::OffsetDateTime::parse(
        "2025-08-25T00:00:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .expect("time");
    assert!(
        first_due(vec![failing_source_record()], before)
            .expect("future record")
            .is_none()
    );
    let retry = super::super::execute_delivery_attempt(
        failing_source_record(),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("retry");
    assert_eq!(result_for(&retry), DrainResult::Retrying);
    let DeliveryExecution::Retry(record) = retry else {
        panic!("retry");
    };
    let terminal =
        super::super::execute_delivery_attempt(record, "2026-08-25T00:01:00Z".to_owned())
            .expect("terminal");
    assert_eq!(result_for(&terminal), DrainResult::DeadLettered);
}

#[test]
fn source_and_measurement_drains_classify_uninitialized_and_unresolvable_lineage() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(root.clone()),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    fs::create_dir_all(root.join("sources/demo")).expect("source");
    let source_id = SourceId::new("demo").expect("source id");
    assert_eq!(
        drain_source_outbox_once(&graph, source_id.clone(), "2026-08-25T00:00:00Z".to_owned())
            .expect("uninitialized"),
        DrainResult::Idle
    );
    assert_eq!(
        drain_measurement_outbox_once(
            &graph,
            source_id.clone(),
            super::super::MeasurementId::new("price").expect("measurement"),
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("uninitialized"),
        DrainResult::Idle
    );
    fs::create_dir(root.join("sources/demo/.ffhn")).expect("orphan storage");
    assert_eq!(
        drain_source_outbox_once(&graph, source_id.clone(), "2026-08-25T00:00:00Z".to_owned())
            .expect("refused source"),
        DrainResult::Unreachable
    );
    assert_eq!(
        drain_measurement_outbox_once(
            &graph,
            source_id.clone(),
            super::super::MeasurementId::new("price").expect("measurement"),
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("refused measurement"),
        DrainResult::Unreachable
    );
    fs::write(root.join("sources/demo/.ffhn-lineage.manifest"), "not-json").expect("manifest");
    assert_eq!(
        drain_source_outbox_once(&graph, source_id, "2026-08-25T00:00:00Z".to_owned())
            .expect("unreachable"),
        DrainResult::Unreachable
    );
    assert_eq!(
        drain_measurement_outbox_once(
            &graph,
            SourceId::new("demo").expect("source"),
            super::super::MeasurementId::new("price").expect("measurement"),
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("unreachable measurement"),
        DrainResult::Unreachable
    );
}

#[test]
fn measurement_drain_is_idle_for_never_initialized_and_ready_empty_outboxes() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(root.clone()),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn")).expect("storage");
    let source_id = SourceId::new("demo").expect("source");
    let source = graph.open_source(source_id.clone()).expect("source");
    let mut identity = SourceIdentity::fresh();
    source.write_identity(&identity).expect("identity");
    let storage = source.open_storage().expect("storage");
    storage
        .write_source_state(&SourceState::fresh(identity.source_instance_id().clone()))
        .expect("source state");
    let never = super::super::MeasurementId::new("never").expect("measurement");
    assert_eq!(
        drain_measurement_outbox_once(
            &graph,
            source_id.clone(),
            never,
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("never initialized"),
        DrainResult::Idle
    );
    let ready = super::super::MeasurementId::new("ready").expect("measurement");
    let measurement_identity = super::super::MeasurementIdentity::fresh();
    identity
        .register_measurement(ready.clone(), measurement_identity.clone())
        .expect("identity");
    source.write_identity(&identity).expect("identity");
    storage
        .write_measurement_state(
            &ready,
            &super::super::MeasurementState::fresh(
                identity.source_instance_id().clone(),
                measurement_identity.measurement_instance_id().clone(),
            ),
        )
        .expect("measurement state");
    assert_eq!(
        drain_measurement_outbox_once(
            &graph,
            source_id,
            ready.clone(),
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("ready empty"),
        DrainResult::Idle
    );
    storage
        .write_measurement_state(
            &ready,
            &super::super::MeasurementState::fresh(
                identity.source_instance_id().clone(),
                super::super::MeasurementInstanceId::mint(),
            ),
        )
        .expect("foreign measurement state");
    assert_eq!(
        drain_measurement_outbox_once(
            &graph,
            SourceId::new("demo").expect("source"),
            ready,
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("held measurement"),
        DrainResult::Unreachable
    );
}

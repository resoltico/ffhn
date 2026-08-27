use std::fs;

use time::Duration;

use super::*;
use crate::graph::{
    GraphPaths, MeasurementDocument, MeasurementId, SourceDocument, TrustedGraphRoot,
};

#[test]
fn empty_graph_tick_claims_and_releases_the_graph_lease_without_source_work() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(temporary.path().join("graph")),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph");
    assert!(
        agent_tick(&graph, "2026-08-25T00:00:00Z".to_owned())
            .expect("first tick")
            .sources()
            .is_empty()
    );
    assert!(
        agent_tick(&graph, "2026-08-25T00:00:01Z".to_owned())
            .expect("released tick")
            .sources()
            .is_empty()
    );
    let mut worker = AgentWorker::try_start(&graph)
        .expect("agent lease")
        .expect("available agent lease");
    assert!(
        worker
            .tick_with_jobs(&graph, "2026-08-25T00:00:02Z".to_owned(), 0)
            .expect_err("zero jobs")
            .to_string()
            .contains("positive")
    );
}

#[test]
fn contention_sets_and_honors_independent_in_memory_capability_deferrals() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph =
        TrustedGraphRoot::initialize(GraphPaths::new(root), "2026-08-25T00:00:00Z".to_owned())
            .expect("graph");
    let source_file = temporary.path().join("source.json");
    fs::write(&source_file, "{}").expect("source file");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n[outbox]\nmax_pending = 2\nmax_attempts = 2\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n[[routes]]\nroute_id = \"source\"\nroute_family = \"on_source\"\n[routes.adapter]\n{}",
        source_file.to_string_lossy(),
        crate::graph::test_support::process_adapter_toml(true, 1_000),
    ))
    .expect("source document");
    let source = graph
        .create_source_document(&source)
        .expect("source configuration");
    let _source_lease = source
        .try_acquire_write_lease()
        .expect("source lock")
        .expect("available source lock");
    let mut worker = AgentWorker::try_start(&graph)
        .expect("agent lease")
        .expect("available agent lease");
    let first = worker
        .tick(&graph, "2026-08-25T00:00:00Z".to_owned())
        .expect("first tick");
    assert_eq!(
        first.sources()[0].source_drain().expect("locked drain"),
        &DrainResult::Locked
    );
    assert!(first.sources()[0].acquisition_deferred_until().is_some());
    assert!(first.sources()[0].drain_deferred_until().is_some());
    assert_eq!(
        first.sources()[0].acquisition_deferred_reason(),
        Some("lock_contention")
    );
    assert_eq!(
        first.sources()[0].drain_deferred_reason(),
        Some("lock_contention")
    );
    let deferred = worker
        .tick(&graph, "2026-08-25T00:00:00.050Z".to_owned())
        .expect("paced tick");
    assert!(deferred.sources()[0].measurement().is_none());
    assert!(deferred.sources()[0].source_drain().is_none());
    assert_eq!(
        worker
            .next_wake_at(&graph, "2026-08-25T00:00:00.050Z".to_owned())
            .expect("wake time"),
        "2026-08-25T00:00:00.1Z"
    );
}

#[test]
fn measurement_drain_deferrals_are_route_independent_agent_report_facts() {
    let source_id = SourceId::new("shop").expect("source id");
    let measurement_id = MeasurementId::new("price").expect("measurement id");
    let mut deferrals = SourceDeferrals::default();
    deferrals.measurement_drain_until.insert(
        measurement_id.clone(),
        OffsetDateTime::parse("2026-08-25T00:00:01Z", &Rfc3339).expect("time"),
    );
    deferrals
        .measurement_drain_reason
        .insert(measurement_id, DeferralReason::DeliveryUnreachable);
    let turn = AgentWorker::turn(&deferrals, source_id, None, None, None, None);
    assert_eq!(
        turn.measurement_drain_deferrals(),
        &[(
            "price".to_owned(),
            "2026-08-25T00:00:01Z".to_owned(),
            "delivery_unreachable".to_owned(),
        )]
    );
    let report = crate::graph::AgentTickReport::from(&AgentTickResult {
        sources: vec![turn],
    });
    let wire = serde_json::to_value(report).expect("agent report");
    assert_eq!(
        wire["source_turns"][0]["measurement_drain_deferrals"][0]["measurement_id"],
        "price"
    );
    assert_eq!(
        wire["source_turns"][0]["measurement_drain_deferrals"][0]["reason"],
        "delivery_unreachable"
    );
}

#[test]
fn unreadable_source_is_deferred_without_aborting_wake_reconstruction() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let graph =
        TrustedGraphRoot::initialize(GraphPaths::new(root), "2026-08-25T00:00:00Z".to_owned())
            .expect("graph");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
        temporary.path().join("source.json").to_string_lossy(),
    ))
    .expect("source");
    let source = graph.create_source_document(&source).expect("source");
    fs::write(source.paths().source_file(), "not valid TOML").expect("invalid source");
    let mut worker = AgentWorker::try_start(&graph)
        .expect("agent lease")
        .expect("available agent lease");
    let tick = worker
        .tick(&graph, "2026-08-25T00:00:00Z".to_owned())
        .expect("isolated tick");
    assert!(tick.has_handled_failure());
    assert_eq!(
        tick.sources()[0].acquisition_deferred_reason(),
        Some("config_invalid")
    );
    assert_eq!(
        worker
            .next_wake_at(&graph, "2026-08-25T00:00:00Z".to_owned())
            .expect("fail-soft wake"),
        "2026-08-25T00:00:01Z"
    );
}

#[test]
fn one_agent_turn_can_deliver_an_event_committed_by_its_acquisition() {
    let temporary = tempfile::tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let source_file = temporary.path().join("source.json");
    fs::write(&source_file, r#"{"price":7}"#).expect("source body");
    let graph =
        TrustedGraphRoot::initialize(GraphPaths::new(root), "2026-08-25T00:00:00Z".to_owned())
            .expect("graph");
    let source: SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 2\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n[outbox]\nmax_pending = 2\nmax_attempts = 2\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n[[routes]]\nroute_id = \"source\"\nroute_family = \"on_source\"\n[routes.adapter]\n{}",
        source_file.to_string_lossy(),
        crate::graph::test_support::process_adapter_toml(true, 1_000),
    ))
    .expect("source");
    let source = graph.create_source_document(&source).expect("source");
    let measurement: MeasurementDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"price\"\ndisplay_name = \"Price\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = \"/price\"\n[outbox]\nmax_pending = 2\nmax_attempts = 2\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n[[routes]]\nroute_id = \"lifecycle\"\nroute_family = \"on_measurement\"\n[routes.adapter]\n{}",
        crate::graph::test_support::process_adapter_toml(true, 1_000),
    ))
    .expect("measurement");
    source
        .create_measurement_document(&measurement)
        .expect("measurement");
    let acquisition =
        super::super::measure_source_once(&graph, SourceId::new("shop").expect("source"))
            .expect("acquisition");
    assert_eq!(acquisition.status(), GraphSourceStatus::Document);
    let mut wake_worker = AgentWorker::try_start(&graph)
        .expect("wake agent")
        .expect("wake lease");
    let pending_wake = wake_worker
        .next_wake_at(&graph, "2026-08-25T00:00:00Z".to_owned())
        .expect("pending wake");
    assert!(OffsetDateTime::parse(&pending_wake, &Rfc3339).is_ok());
    let now = OffsetDateTime::parse("2026-08-25T00:00:00Z", &Rfc3339).expect("now");
    assert!(
        !super::wake::source_wake_candidates_with(
            &wake_worker,
            &graph,
            SourceId::new("shop").expect("source"),
            now,
            |source| source.inspect_lineage([]),
            |_| Err(CoreError::internal("storage failed")),
        )
        .is_empty()
    );
    let source_toml = fs::read_to_string(source.paths().source_file()).expect("source TOML");
    fs::write(
        source.paths().source_file(),
        source_toml.replace("enabled = true", "enabled = false"),
    )
    .expect("disable source");
    let without_acquisition = super::wake::source_wake_candidates(
        &wake_worker,
        &graph,
        SourceId::new("shop").expect("source"),
        now,
    );
    wake_worker
        .deferrals
        .entry(SourceId::new("shop").expect("source"))
        .or_default()
        .defer_acquisition(now, 100, DeferralReason::SourceDisabled);
    let with_acquisition = super::wake::source_wake_candidates(
        &wake_worker,
        &graph,
        SourceId::new("shop").expect("source"),
        now,
    );
    assert!(with_acquisition.len() > without_acquisition.len());
    fs::write(source.paths().source_file(), source_toml).expect("restore source");
    let tick = wake_worker
        .tick(&graph, "2026-08-25T00:00:00Z".to_owned())
        .expect("agent tick");
    assert_eq!(
        tick.sources()[0].measurement_drains(),
        &[("price".to_owned(), DrainResult::Delivered)]
    );
    let mut worker = wake_worker;
    let wake = worker
        .next_wake_at(&graph, "2026-08-25T00:00:00Z".to_owned())
        .expect("wake");
    assert!(OffsetDateTime::parse(&wake, &Rfc3339).is_ok());
    let shop_id = SourceId::new("shop").expect("source");
    let acquisition_deferred =
        OffsetDateTime::parse("2040-08-25T00:00:00Z", &Rfc3339).expect("deferred");
    let shop_deferrals = worker.deferrals.entry(shop_id).or_default();
    shop_deferrals.acquisition_until = Some(acquisition_deferred);
    shop_deferrals.acquisition_reason = Some(DeferralReason::LockContention);
    let source_lease = source
        .try_acquire_write_lease()
        .expect("source lock")
        .expect("lease");
    let contended = worker
        .tick(&graph, "2036-08-25T00:00:00Z".to_owned())
        .expect("contended tick");
    assert_eq!(
        contended.sources()[0].measurement_drains(),
        &[("price".to_owned(), DrainResult::Locked)]
    );
    drop(source_lease);
    let measurement_id = MeasurementId::new("price").expect("measurement");
    let source_id = SourceId::new("shop").expect("source");
    let deferred_until =
        OffsetDateTime::parse("2040-08-25T00:00:00Z", &Rfc3339).expect("deferred time");
    worker
        .deferrals
        .entry(source_id)
        .or_default()
        .measurement_drain_until
        .insert(measurement_id.clone(), deferred_until);
    worker
        .deferrals
        .get_mut(&SourceId::new("shop").expect("source"))
        .expect("deferrals")
        .measurement_drain_reason
        .insert(measurement_id, DeferralReason::DeliveryUnreachable);
    let deferred = worker
        .tick(&graph, "2037-08-25T00:00:00Z".to_owned())
        .expect("deferred tick");
    assert!(deferred.sources()[0].measurement_drains().is_empty());
    assert_eq!(deferred.sources()[0].measurement_drain_deferrals().len(), 1);
}

#[test]
fn deferral_vocabulary_expiry_wake_helpers_and_turn_accessors_are_complete() {
    for (reason, spelling) in [
        (DeferralReason::LockContention, "lock_contention"),
        (DeferralReason::Unreadable, "unreadable"),
        (DeferralReason::SourceDisabled, "source_disabled"),
        (
            DeferralReason::UnresolvableManifest,
            "unresolvable_manifest",
        ),
        (DeferralReason::ConfigInvalid, "config_invalid"),
        (DeferralReason::LineageRefused, "lineage_refused"),
        (DeferralReason::AcquisitionHold, "acquisition_hold"),
        (DeferralReason::DeliveryUnreachable, "delivery_unreachable"),
    ] {
        assert_eq!(reason.as_str(), spelling);
    }
    for (status, expected) in [
        (GraphSourceStatus::Disabled, DeferralReason::SourceDisabled),
        (
            GraphSourceStatus::UnresolvableManifest,
            DeferralReason::UnresolvableManifest,
        ),
        (
            GraphSourceStatus::ConfigInvalid,
            DeferralReason::ConfigInvalid,
        ),
        (
            GraphSourceStatus::LineageRefused,
            DeferralReason::LineageRefused,
        ),
        (
            GraphSourceStatus::AcquisitionHold,
            DeferralReason::AcquisitionHold,
        ),
    ] {
        assert_eq!(acquisition_deferral_reason(status), Some(expected));
    }

    let now = OffsetDateTime::parse("2026-08-25T00:00:00Z", &Rfc3339).expect("now");
    let later = now + Duration::milliseconds(100);
    let measurement_id = MeasurementId::new("price").expect("measurement");
    let mut deferrals = SourceDeferrals::default();
    assert!(deferrals.acquisition_permitted(now));
    assert!(deferrals.drain_permitted(now));
    assert!(deferrals.measurement_drain_permitted(&measurement_id, now));
    deferrals.defer_acquisition(now, 100, DeferralReason::LockContention);
    deferrals.defer_drain(now, 100, DeferralReason::Unreadable);
    deferrals
        .measurement_drain_until
        .insert(measurement_id.clone(), later);
    deferrals
        .measurement_drain_reason
        .insert(measurement_id.clone(), DeferralReason::DeliveryUnreachable);
    assert!(!deferrals.acquisition_permitted(now));
    assert!(!deferrals.drain_permitted(now));
    assert!(!deferrals.measurement_drain_permitted(&measurement_id, now));
    assert_eq!(super::wake::deferred_fallback(Some(&deferrals), now), later);
    assert_eq!(super::wake::max_with_source_defer(now, Some(later)), later);
    assert_eq!(super::wake::max_with_source_defer(later, None), later);
    assert!(super::wake::parse_utc("bad").is_none());
    assert_eq!(super::wake::parse_utc("2026-08-25T00:00:00Z"), Some(now));
    let mut candidates = Vec::new();
    assert_eq!(
        super::wake::or_fallback(Ok(7_u8), &mut candidates, later),
        Some(7)
    );
    assert!(
        super::wake::or_fallback::<u8>(Err(CoreError::internal("failed")), &mut candidates, later,)
            .is_none()
    );
    assert_eq!(candidates, [later]);
    assert!(
        super::wake::records_or_fallback::<u8>(
            Err(CoreError::internal("failed")),
            &mut candidates,
            later,
        )
        .is_empty()
    );
    let mut retry_candidates = Vec::new();
    super::wake::push_source_retry(&mut retry_candidates, None, None);
    super::wake::push_source_retry(&mut retry_candidates, Some(now), None);
    assert_eq!(retry_candidates, [now]);
    retry_candidates.clear();
    super::wake::push_measurement_retry(&mut retry_candidates, None, None, &measurement_id);
    super::wake::push_measurement_retry(&mut retry_candidates, Some(now), None, &measurement_id);
    assert_eq!(retry_candidates, [now]);
    deferrals.expire_elapsed(later);
    assert!(deferrals.acquisition_permitted(later));
    assert!(deferrals.drain_permitted(later));
    assert!(deferrals.measurement_drain_permitted(&measurement_id, later));
    assert!(deferrals.measurement_drain_reason.is_empty());

    let turn = AgentWorker::turn(
        &SourceDeferrals::default(),
        SourceId::new("shop").expect("source"),
        None,
        Some("acquisition".to_owned()),
        Some(DrainResult::Retrying),
        Some("drain".to_owned()),
    );
    assert_eq!(turn.source_id().as_str(), "shop");
    assert!(turn.measurement().is_none());
    assert_eq!(turn.acquisition_error(), Some("acquisition"));
    assert_eq!(turn.source_drain(), Some(&DrainResult::Retrying));
    assert_eq!(turn.drain_error(), Some("drain"));
    assert!(turn.measurement_drains().is_empty());
    assert!(turn.measurement_drain_deferrals().is_empty());
    assert!(turn.acquisition_deferred_until().is_none());
    assert!(turn.acquisition_deferred_reason().is_none());
    assert!(turn.drain_deferred_until().is_none());
    assert!(turn.drain_deferred_reason().is_none());
    let result = AgentTickResult {
        sources: vec![turn],
    };
    assert!(result.has_handled_failure());
    for drain in [
        DrainResult::Locked,
        DrainResult::Idle,
        DrainResult::Delivered,
        DrainResult::Retrying,
        DrainResult::DeadLettered,
        DrainResult::Unreachable,
    ] {
        assert_eq!(
            drain_is_failure(&drain),
            matches!(
                drain,
                DrainResult::Retrying | DrainResult::DeadLettered | DrainResult::Unreachable
            )
        );
    }
    for status in [
        GraphSourceStatus::Locked,
        GraphSourceStatus::Document,
        GraphSourceStatus::NotModified,
        GraphSourceStatus::FetchFailed,
        GraphSourceStatus::IntegrationFault,
    ] {
        assert_eq!(acquisition_deferral_reason(status), None);
    }

    let id = MeasurementId::new("drain").expect("measurement");
    let mut outcomes = Vec::new();
    let mut deferrals = SourceDeferrals::default();
    assert!(
        record_measurement_drain_result(
            &mut deferrals,
            &id,
            now,
            1_000,
            Ok(DrainResult::Locked),
            &mut outcomes,
        )
        .expect("locked control")
    );
    assert_eq!(deferrals.drain_reason, Some(DeferralReason::LockContention));
    outcomes.clear();
    assert!(
        !record_measurement_drain_result(
            &mut deferrals,
            &id,
            now,
            1_000,
            Ok(DrainResult::Unreachable),
            &mut outcomes,
        )
        .expect("unreachable control")
    );
    assert_eq!(
        deferrals.measurement_drain_reason.get(&id),
        Some(&DeferralReason::DeliveryUnreachable)
    );
    assert_eq!(
        deferrals.measurement_drain_until.get(&id),
        Some(&(now + Duration::milliseconds(1_000)))
    );
    outcomes.clear();
    assert!(
        !record_measurement_drain_result(
            &mut deferrals,
            &id,
            now,
            1_000,
            Ok(DrainResult::Delivered),
            &mut outcomes,
        )
        .expect("delivered control")
    );
    assert!(!deferrals.measurement_drain_reason.contains_key(&id));
    outcomes.clear();
    assert!(
        record_measurement_drain_result(
            &mut deferrals,
            &id,
            now,
            1_000,
            Err(CoreError::internal("drain failed")),
            &mut outcomes,
        )
        .is_err()
    );
    assert_eq!(
        deferrals.measurement_drain_reason.get(&id),
        Some(&DeferralReason::Unreadable)
    );
    assert_eq!(
        deferrals.measurement_drain_until.get(&id),
        Some(&(now + Duration::milliseconds(1_000)))
    );
    let mut deferrals = SourceDeferrals::default();
    let (locked, error) = drain_measurements(
        vec![
            id.clone(),
            MeasurementId::new("later").expect("measurement"),
        ],
        &mut deferrals,
        now,
        1_000,
        |_| Ok(DrainResult::Locked),
    );
    assert_eq!(locked, [("drain".to_owned(), DrainResult::Locked)]);
    assert!(error.is_none());
    let mut deferrals = SourceDeferrals::default();
    let (outcomes, error) =
        drain_measurements(vec![id.clone()], &mut deferrals, now, 1_000, |_| {
            Err(CoreError::internal("failed"))
        });
    assert!(outcomes.is_empty());
    assert!(error.is_some());
    let mut deferrals = SourceDeferrals::default();
    deferrals
        .measurement_drain_until
        .insert(id.clone(), now + Duration::seconds(1));
    let (outcomes, error) = drain_measurements(vec![id], &mut deferrals, now, 1_000, |_| {
        panic!("deferred drain must not execute")
    });
    assert!(outcomes.is_empty());
    assert!(error.is_none());
    let mut deferrals = SourceDeferrals::default();
    deferrals.defer_drain(now, 100, DeferralReason::DeliveryUnreachable);
    let (_, error, _) = finish_source_drain(
        &mut deferrals,
        DrainResult::Delivered,
        Vec::new(),
        Some("failed".to_owned()),
    );
    assert_eq!(error.as_deref(), Some("failed"));
    assert!(deferrals.drain_until.is_some());
    let (_, error, _) =
        finish_source_drain(&mut deferrals, DrainResult::Delivered, Vec::new(), None);
    assert!(error.is_none());
    assert!(deferrals.drain_until.is_none());
    deferrals.defer_drain(now, 100, DeferralReason::DeliveryUnreachable);
    finish_source_drain(&mut deferrals, DrainResult::Unreachable, Vec::new(), None);
    assert!(deferrals.drain_until.is_some());

    let source_id = SourceId::new("failure-axis").expect("source");
    let empty_turn = AgentWorker::turn(
        &SourceDeferrals::default(),
        source_id.clone(),
        None,
        None,
        None,
        None,
    );
    assert!(!empty_turn.has_handled_failure());
    let drain_error = AgentWorker::turn(
        &SourceDeferrals::default(),
        source_id.clone(),
        None,
        None,
        None,
        Some("drain failed".to_owned()),
    );
    assert!(drain_error.has_handled_failure());
    let source_retry = AgentWorker::turn(
        &SourceDeferrals::default(),
        source_id.clone(),
        None,
        None,
        Some(DrainResult::Retrying),
        None,
    );
    assert!(source_retry.has_handled_failure());
    let measurement_retry = AgentWorker::turn_with_measurements(
        &SourceDeferrals::default(),
        source_id,
        None,
        None,
        Some(DrainResult::Idle),
        None,
        vec![("measurement".to_owned(), DrainResult::DeadLettered)],
    );
    assert!(measurement_retry.has_handled_failure());
}

#[path = "tests/failure_isolation.rs"]
mod failure_isolation;

#[path = "tests/wake_mutation_contracts.rs"]
mod wake_mutation_contracts;

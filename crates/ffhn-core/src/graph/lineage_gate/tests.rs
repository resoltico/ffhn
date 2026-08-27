use std::fs;

use tempfile::tempdir;

use super::*;
use crate::{
    ConditionId,
    graph::{
        DeliveryPolicy, DeliveryRecord, EventEnvelope, EventEnvelopeParts, EventKey,
        EventObservation, GraphId, GraphPaths, GraphRoute, MeasurementIdentity,
        MeasurementInstanceId, OutboxAdmission, SourceId, SourceIdentity, SourceInstanceId,
        TrustedGraphRoot,
    },
};

fn source_with_storage() -> (tempfile::TempDir, TrustedSourceDir) {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    let source_id = SourceId::new("demo").expect("source id");
    fs::create_dir_all(root.join("sources").join(source_id.as_str()).join(".ffhn"))
        .expect("storage root");
    let graph = TrustedGraphRoot::open(GraphPaths::new(root)).expect("graph root");
    (
        temporary,
        graph.open_source(source_id).expect("source directory"),
    )
}

fn ready_source(source: &TrustedSourceDir) -> SourceIdentity {
    let identity = SourceIdentity::fresh();
    source.write_identity(&identity).expect("identity");
    source
        .open_storage()
        .expect("storage")
        .write_source_state(&SourceState::fresh(identity.source_instance_id().clone()))
        .expect("state");
    identity
}

fn measurement_record(
    source_instance_id: SourceInstanceId,
    measurement_id: MeasurementId,
    measurement_instance_id: MeasurementInstanceId,
) -> DeliveryRecord {
    measurement_record_for(
        SourceId::new("demo").expect("source id"),
        source_instance_id,
        measurement_id,
        measurement_instance_id,
    )
}

fn measurement_record_for(
    source_id: SourceId,
    source_instance_id: SourceInstanceId,
    measurement_id: MeasurementId,
    measurement_instance_id: MeasurementInstanceId,
) -> DeliveryRecord {
    let policy: DeliveryPolicy = toml::from_str(
        "max_pending = 1\nmax_attempts = 2\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n",
    )
    .expect("policy");
    let program = std::env::current_exe().expect("test executable");
    let route: GraphRoute = toml::from_str(&format!(
        "route_id = \"condition\"\nroute_family = \"on_condition\"\n[adapter]\nkind = \"process_stdin\"\nprogram = {:?}\ntimeout_ms = 1000\n",
        program.to_string_lossy(),
    ))
    .expect("route");
    let graph_id = GraphId::mint();
    let envelope = EventEnvelope::new(EventEnvelopeParts {
        graph_id: graph_id.clone(),
        source_instance_id: source_instance_id.clone(),
        event_key: EventKey::ConditionSatisfied {
            graph_id,
            source_id,
            source_instance_id,
            measurement_id,
            measurement_instance_id,
            condition_id: ConditionId::new("changed").expect("condition id"),
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

fn source_record(source_instance_id: SourceInstanceId) -> DeliveryRecord {
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
            source_id: SourceId::new("demo").expect("source"),
            source_instance_id,
            event_kind: super::super::EventKind::SourceInitialized,
            source_representation_digest: "a".repeat(64),
        },
        display_name: "Source".to_owned(),
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
fn source_rows_are_closed_before_any_measurement_can_be_used() {
    let (_temporary, source) = source_with_storage();
    assert_eq!(
        source.inspect_lineage([]).expect("inspection").source(),
        &SourceLineage::Refused(SourceLineageRefusal::StorageWithoutIdentity)
    );

    let identity = SourceIdentity::fresh();
    source.write_identity(&identity).expect("identity");
    assert_eq!(
        source.inspect_lineage([]).expect("inspection").source(),
        &SourceLineage::Refused(SourceLineageRefusal::SourceStateMissing)
    );

    source
        .open_storage()
        .expect("storage")
        .write_source_state(&SourceState::fresh(SourceInstanceId::mint()))
        .expect("foreign state");
    assert_eq!(
        source.inspect_lineage([]).expect("inspection").source(),
        &SourceLineage::Refused(SourceLineageRefusal::SourceInstanceMismatch)
    );

    source
        .open_storage()
        .expect("storage")
        .write_source_state(&SourceState::fresh(identity.source_instance_id().clone()))
        .expect("matching state");
    assert!(matches!(
        source.inspect_lineage([]).expect("inspection").source(),
        SourceLineage::Ready { .. }
    ));
}

#[test]
fn measurement_rows_hold_only_the_affected_measurement_and_never_hide_unconfigured_artifacts() {
    let (_temporary, source) = source_with_storage();
    let mut identity = ready_source(&source);
    let ready_id = MeasurementId::new("ready").expect("measurement id");
    let missing_id = MeasurementId::new("missing").expect("measurement id");
    let orphan_id = MeasurementId::new("orphan").expect("measurement id");
    let artifact_only_id = MeasurementId::new("artifact-only").expect("measurement id");
    let ready_identity = MeasurementIdentity::fresh();
    identity
        .register_measurement(ready_id.clone(), ready_identity.clone())
        .expect("ready entry");
    identity
        .register_measurement(missing_id.clone(), MeasurementIdentity::fresh())
        .expect("missing entry");
    source.write_identity(&identity).expect("updated identity");
    let storage = source.open_storage().expect("storage");
    storage
        .write_measurement_state(
            &ready_id,
            &MeasurementState::fresh(
                identity.source_instance_id().clone(),
                ready_identity.measurement_instance_id().clone(),
            ),
        )
        .expect("ready state");
    storage
        .write_measurement_state(
            &orphan_id,
            &MeasurementState::fresh(
                identity.source_instance_id().clone(),
                MeasurementInstanceId::mint(),
            ),
        )
        .expect("orphan state");
    fs::create_dir_all(source.paths().measurement_outbox_dir(&artifact_only_id))
        .expect("artifact-only subtree");

    let inspection = source.inspect_lineage([]).expect("inspection");
    assert!(matches!(
        inspection.measurement(&ready_id),
        Some(MeasurementLineage::Ready(_))
    ));
    assert_eq!(
        inspection.measurement(&missing_id),
        Some(&MeasurementLineage::Held(
            MeasurementLineageHold::StateMissing
        ))
    );
    assert_eq!(
        inspection.measurement(&orphan_id),
        Some(&MeasurementLineage::Held(
            MeasurementLineageHold::MissingIdentityEntry
        ))
    );
    assert_eq!(
        inspection.measurement(&artifact_only_id),
        Some(&MeasurementLineage::Held(
            MeasurementLineageHold::ArtifactUnreadable
        ))
    );
}

#[test]
fn declared_measurement_with_no_entry_or_artifact_is_the_only_never_initialized_case() {
    let (_temporary, source) = source_with_storage();
    ready_source(&source);
    let declared = MeasurementId::new("fresh").expect("measurement id");
    assert_eq!(
        source
            .inspect_lineage([declared.clone()])
            .expect("inspection")
            .measurement(&declared),
        Some(&MeasurementLineage::NeverInitialized)
    );
}

#[test]
fn every_pending_record_is_lineage_gated_at_its_owner_scope() {
    let (_temporary, source) = source_with_storage();
    let mut identity = ready_source(&source);
    let measurement_id = MeasurementId::new("price").expect("measurement id");
    let measurement_identity = MeasurementIdentity::fresh();
    identity
        .register_measurement(measurement_id.clone(), measurement_identity.clone())
        .expect("measurement identity");
    source.write_identity(&identity).expect("identity");
    let storage = source.open_storage().expect("storage");
    storage
        .write_measurement_state(
            &measurement_id,
            &MeasurementState::fresh(
                identity.source_instance_id().clone(),
                measurement_identity.measurement_instance_id().clone(),
            ),
        )
        .expect("measurement state");
    let foreign = measurement_record(
        identity.source_instance_id().clone(),
        measurement_id.clone(),
        MeasurementInstanceId::mint(),
    );
    fs::create_dir_all(source.paths().measurement_outbox_dir(&measurement_id))
        .expect("measurement outbox");
    fs::write(
        source
            .paths()
            .measurement_outbox_dir(&measurement_id)
            .join(foreign.storage_file_name()),
        format!(
            "{}\n",
            crate::stable_json::stable_json(&foreign).expect("record JSON")
        ),
    )
    .expect("foreign record");
    assert_eq!(
        source
            .inspect_lineage([])
            .expect("inspection")
            .measurement(&measurement_id),
        Some(&MeasurementLineage::Held(
            MeasurementLineageHold::MeasurementInstanceMismatch
        ))
    );

    fs::create_dir_all(source.paths().source_outbox_dir()).expect("source outbox");
    fs::write(
        source
            .paths()
            .source_outbox_dir()
            .join(foreign.storage_file_name()),
        format!(
            "{}\n",
            crate::stable_json::stable_json(&foreign).expect("record JSON")
        ),
    )
    .expect("misplaced source record");
    assert_eq!(
        source.inspect_lineage([]).expect("inspection").source(),
        &SourceLineage::Refused(SourceLineageRefusal::DeliveryArtifactUnreadable)
    );
}

#[test]
fn lineage_result_vocabularies_and_ready_accessors_are_complete() {
    for (reason, spelling) in [
        (
            SourceLineageRefusal::StorageWithoutIdentity,
            "storage_without_identity",
        ),
        (SourceLineageRefusal::StorageMissing, "storage_missing"),
        (
            SourceLineageRefusal::StorageUnavailable,
            "storage_unavailable",
        ),
        (
            SourceLineageRefusal::SourceStateMissing,
            "source_state_missing",
        ),
        (
            SourceLineageRefusal::SourceStateUnreadable,
            "source_state_unreadable",
        ),
        (
            SourceLineageRefusal::SourceInstanceMismatch,
            "source_instance_mismatch",
        ),
        (
            SourceLineageRefusal::MeasurementStorageUnreadable,
            "measurement_storage_unreadable",
        ),
        (
            SourceLineageRefusal::IdentityUnreadable,
            "identity_unreadable",
        ),
        (
            SourceLineageRefusal::DeliveryArtifactUnreadable,
            "delivery_artifact_unreadable",
        ),
    ] {
        assert_eq!(reason.as_str(), spelling);
    }
    for (hold, spelling) in [
        (
            MeasurementLineageHold::MissingIdentityEntry,
            "missing_identity_entry",
        ),
        (MeasurementLineageHold::StateMissing, "state_missing"),
        (
            MeasurementLineageHold::ArtifactUnreadable,
            "artifact_unreadable",
        ),
        (
            MeasurementLineageHold::SourceInstanceMismatch,
            "source_instance_mismatch",
        ),
        (
            MeasurementLineageHold::MeasurementInstanceMismatch,
            "measurement_instance_mismatch",
        ),
    ] {
        assert_eq!(hold.as_str(), spelling);
    }
    assert!(
        SourceLineage::NeedsInitialization
            .as_ready_state()
            .is_none()
    );
    assert!(
        SourceLineage::Refused(SourceLineageRefusal::StorageMissing)
            .as_ready_state()
            .is_none()
    );

    let (_temporary, source) = source_with_storage();
    let identity = ready_source(&source);
    let inspection = source.inspect_lineage([]).expect("inspection");
    let SourceLineage::Ready(ready) = inspection.source() else {
        panic!("ready source");
    };
    assert_eq!(ready.identity(), &identity);
    assert_eq!(
        ready.state().source_instance_id(),
        identity.source_instance_id()
    );
    assert_eq!(inspection.source().as_ready_state(), Some(ready.state()));
    assert!(inspection.measurements().is_empty());
    assert!(validate_measurement_entry_type(false, true).is_ok());
    assert!(validate_measurement_entry_type(true, true).is_err());
    assert!(validate_measurement_entry_type(false, false).is_err());
    assert!(
        measurement_entry_file_type::<()>(
            Err(std::io::Error::other("failed")),
            std::path::Path::new("measurement"),
        )
        .is_err()
    );
    assert!(
        measurement_dir_entry::<()>(
            Err(std::io::Error::other("failed")),
            std::path::Path::new("measurements"),
        )
        .is_err()
    );
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        assert!(measurement_storage_name(std::ffi::OsString::from_vec(vec![0xff])).is_err());
    }
}

#[test]
fn source_gate_classifies_missing_unreadable_and_unsafe_storage_components() {
    let temporary = tempdir().expect("temporary");
    let root = temporary.path().join("graph");
    let source_id = SourceId::new("demo").expect("source id");
    fs::create_dir_all(root.join("sources/demo")).expect("source dir");
    let graph = TrustedGraphRoot::open(GraphPaths::new(root.clone())).expect("graph");
    let source = graph.open_source(source_id.clone()).expect("source");
    source
        .write_identity(&SourceIdentity::fresh())
        .expect("identity");
    assert_eq!(
        source.inspect_lineage([]).expect("inspection").source(),
        &SourceLineage::Refused(SourceLineageRefusal::StorageMissing)
    );

    fs::remove_file(source.paths().identity_file()).expect("remove identity");
    fs::write(source.paths().identity_file(), "not-json").expect("bad identity");
    assert_eq!(
        source.inspect_lineage([]).expect("inspection").source(),
        &SourceLineage::Refused(SourceLineageRefusal::IdentityUnreadable)
    );

    fs::remove_file(source.paths().identity_file()).expect("remove bad identity");
    let identity = SourceIdentity::fresh();
    source.write_identity(&identity).expect("identity");
    fs::write(source.paths().storage_dir(), "not-a-directory").expect("bad storage");
    assert_eq!(
        source.inspect_lineage([]).expect("inspection").source(),
        &SourceLineage::Refused(SourceLineageRefusal::StorageUnavailable)
    );
    fs::remove_file(source.paths().storage_dir()).expect("remove bad storage");
    fs::create_dir_all(source.paths().storage_dir()).expect("storage");
    fs::write(source.paths().source_state_file(), "not-json").expect("bad state");
    assert_eq!(
        source.inspect_lineage([]).expect("inspection").source(),
        &SourceLineage::Refused(SourceLineageRefusal::SourceStateUnreadable)
    );
}

#[test]
fn measurement_gate_classifies_state_stamps_and_unreadable_artifact_subtrees() {
    let (_temporary, source) = source_with_storage();
    let mut identity = ready_source(&source);
    let id = MeasurementId::new("price").expect("measurement id");
    let entry = MeasurementIdentity::fresh();
    identity
        .register_measurement(id.clone(), entry.clone())
        .expect("entry");
    source.write_identity(&identity).expect("identity");
    let storage = source.open_storage().expect("storage");
    storage
        .write_measurement_state(
            &id,
            &MeasurementState::fresh(
                SourceInstanceId::mint(),
                entry.measurement_instance_id().clone(),
            ),
        )
        .expect("foreign source state");
    assert_eq!(
        source
            .inspect_lineage([])
            .expect("inspection")
            .measurement(&id),
        Some(&MeasurementLineage::Held(
            MeasurementLineageHold::SourceInstanceMismatch
        ))
    );
    storage
        .write_measurement_state(
            &id,
            &MeasurementState::fresh(
                identity.source_instance_id().clone(),
                MeasurementInstanceId::mint(),
            ),
        )
        .expect("foreign measurement state");
    assert_eq!(
        source
            .inspect_lineage([])
            .expect("inspection")
            .measurement(&id),
        Some(&MeasurementLineage::Held(
            MeasurementLineageHold::MeasurementInstanceMismatch
        ))
    );
    fs::write(
        source
            .paths()
            .measurement_storage_dir(&id)
            .join("state.json"),
        "not-json",
    )
    .expect("bad state");
    assert_eq!(
        source
            .inspect_lineage([])
            .expect("inspection")
            .measurement(&id),
        Some(&MeasurementLineage::Held(
            MeasurementLineageHold::ArtifactUnreadable
        ))
    );

    fs::remove_dir_all(source.paths().storage_dir().join("measurements"))
        .expect("remove measurement root");
    fs::write(
        source.paths().storage_dir().join("measurements"),
        "bad root",
    )
    .expect("bad measurement root");
    assert_eq!(
        source.inspect_lineage([]).expect("inspection").source(),
        &SourceLineage::Refused(SourceLineageRefusal::MeasurementStorageUnreadable)
    );
}

#[path = "tests/delivery.rs"]
mod delivery;

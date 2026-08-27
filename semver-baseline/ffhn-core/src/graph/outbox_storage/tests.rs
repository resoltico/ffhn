use std::fs;

use tempfile::tempdir;

use super::*;
use crate::{
    ConditionId,
    graph::{
        DeliveryPolicy, EventEnvelope, EventEnvelopeParts, EventKey, EventObservation, GraphId,
        GraphRoute, MeasurementId, MeasurementInstanceId, OutboxAdmission, SourceId,
        SourceInstanceId, TrustedGraphRoot,
    },
};

fn record() -> DeliveryRecord {
    let policy: DeliveryPolicy = toml::from_str(
        "max_pending = 2\nmax_attempts = 3\nbase_backoff_ms = 100\nmax_backoff_ms = 1000\njitter_ratio = \"0\"\n",
    )
    .expect("policy");
    let route: GraphRoute = toml::from_str(&format!(
        "route_id = \"critical\"\nroute_family = \"on_condition\"\n[adapter]\n{}",
        crate::graph::test_support::process_adapter_toml(true, 1_000),
    ))
    .expect("route");
    let graph_id = GraphId::mint();
    let source_instance_id = SourceInstanceId::mint();
    let envelope = EventEnvelope::new(EventEnvelopeParts {
        graph_id: graph_id.clone(),
        source_instance_id: source_instance_id.clone(),
        event_key: EventKey::ConditionSatisfied {
            graph_id,
            source_id: SourceId::new("shop").expect("source id"),
            source_instance_id,
            measurement_id: MeasurementId::new("price").expect("measurement id"),
            measurement_instance_id: MeasurementInstanceId::mint(),
            condition_id: ConditionId::new("changed").expect("condition id"),
            condition_defn_digest: "a".repeat(64),
            observation_seq: 1,
        },
        display_name: "Price".to_owned(),
        committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
        observation: Some(EventObservation::new("1".to_owned(), 1).expect("observation")),
        lifecycle_fact: None,
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
fn outbox_scan_is_no_follow_and_binds_each_filename_to_its_immutable_record_key() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn/source-outbox")).expect("outbox");
    let graph = TrustedGraphRoot::open(crate::graph::GraphPaths::new(root)).expect("graph root");
    let source = graph
        .open_source(SourceId::new("demo").expect("source id"))
        .expect("source");
    let record = record();
    fs::write(
        source
            .paths()
            .source_outbox_dir()
            .join(record.storage_file_name()),
        serde_json::to_string(&record).expect("record JSON"),
    )
    .expect("record write");
    let records = source
        .open_storage()
        .expect("storage")
        .read_source_delivery_records()
        .expect("records");
    assert_eq!(records, [record]);
    fs::write(
        source.paths().source_outbox_dir().join("foreign.json"),
        "{}",
    )
    .expect("foreign record");
    assert!(
        source
            .open_storage()
            .expect("storage")
            .read_source_delivery_records()
            .is_err()
    );
}

fn dead_letter() -> DeadLetter {
    let mut record = record();
    for second in 1..=3 {
        record
            .record_failure(
                format!("2026-08-25T00:00:0{second}Z"),
                super::super::DeliveryAttemptFailure::Process {
                    message: "failed".to_owned(),
                },
                format!("2026-08-25T00:00:0{second}Z"),
            )
            .expect("failure");
    }
    record.into_dead_letter().expect("dead letter")
}

#[test]
fn source_and_measurement_outbox_scans_cover_absence_dead_letters_and_filename_mismatch() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn")).expect("storage");
    let graph = TrustedGraphRoot::open(crate::graph::GraphPaths::new(root)).expect("graph");
    let source = graph
        .open_source(SourceId::new("demo").expect("source id"))
        .expect("source");
    let storage = source.open_storage().expect("storage");
    let measurement_id = MeasurementId::new("price").expect("measurement id");
    assert!(
        storage
            .read_source_delivery_records()
            .expect("records")
            .is_empty()
    );
    assert!(
        storage
            .read_source_dead_letters()
            .expect("letters")
            .is_empty()
    );
    assert!(
        storage
            .read_measurement_delivery_records(&measurement_id)
            .expect("records")
            .is_empty()
    );
    assert!(
        storage
            .read_measurement_dead_letters(&measurement_id)
            .expect("letters")
            .is_empty()
    );

    let record = record();
    let letter = dead_letter();
    for directory in [
        source.paths().measurement_outbox_dir(&measurement_id),
        source.paths().measurement_dead_letters_dir(&measurement_id),
    ] {
        fs::create_dir_all(directory).expect("directory");
    }
    fs::write(
        source
            .paths()
            .measurement_outbox_dir(&measurement_id)
            .join(record.storage_file_name()),
        serde_json::to_string(&record).expect("record JSON"),
    )
    .expect("record");
    fs::write(
        source
            .paths()
            .measurement_dead_letters_dir(&measurement_id)
            .join(letter.record().storage_file_name()),
        serde_json::to_string(&letter).expect("letter JSON"),
    )
    .expect("letter");
    assert_eq!(
        storage
            .read_measurement_delivery_records(&measurement_id)
            .expect("records"),
        std::slice::from_ref(&record)
    );
    assert_eq!(
        storage
            .read_measurement_dead_letters(&measurement_id)
            .expect("letters"),
        std::slice::from_ref(&letter)
    );

    fs::rename(
        source
            .paths()
            .measurement_outbox_dir(&measurement_id)
            .join(record.storage_file_name()),
        source
            .paths()
            .measurement_outbox_dir(&measurement_id)
            .join("foreign.json"),
    )
    .expect("rename record");
    assert!(
        storage
            .read_measurement_delivery_records(&measurement_id)
            .is_err()
    );
    fs::rename(
        source
            .paths()
            .measurement_dead_letters_dir(&measurement_id)
            .join(letter.record().storage_file_name()),
        source
            .paths()
            .measurement_dead_letters_dir(&measurement_id)
            .join("foreign.json"),
    )
    .expect("rename letter");
    assert!(
        storage
            .read_measurement_dead_letters(&measurement_id)
            .is_err()
    );
}

#[test]
fn outbox_directory_entries_must_be_regular_json_files() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn/source-outbox/entry.json"))
        .expect("directory entry");
    fs::create_dir_all(root.join("sources/demo/.ffhn/dead-letters/entry.json"))
        .expect("directory entry");
    let graph = TrustedGraphRoot::open(crate::graph::GraphPaths::new(root)).expect("graph");
    let storage = graph
        .open_source(SourceId::new("demo").expect("source"))
        .expect("source")
        .open_storage()
        .expect("storage");
    assert!(
        storage
            .read_source_delivery_records()
            .expect_err("invalid pending entry")
            .to_string()
            .contains("non-symlink .json regular files")
    );
    assert!(
        storage
            .read_source_dead_letters()
            .expect_err("invalid dead-letter entry")
            .to_string()
            .contains("non-symlink .json regular files")
    );
}

#[cfg(unix)]
#[test]
fn outbox_directory_entries_reject_symlinks_non_json_files_and_non_utf8_names() {
    use std::os::unix::ffi::OsStringExt;

    for (role, reader) in [("source-outbox", 0_u8), ("dead-letters", 1_u8)] {
        let temporary = tempdir().expect("temporary graph");
        let root = temporary.path().join("graph");
        let directory = root.join("sources/demo/.ffhn").join(role);
        fs::create_dir_all(&directory).expect("directory");
        fs::write(directory.join("record.txt"), "text").expect("non JSON");
        let graph =
            TrustedGraphRoot::open(crate::graph::GraphPaths::new(root.clone())).expect("graph");
        let storage = graph
            .open_source(SourceId::new("demo").expect("source"))
            .expect("source")
            .open_storage()
            .expect("storage");
        let result = if reader == 0 {
            storage.read_source_delivery_records().map(|_| ())
        } else {
            storage.read_source_dead_letters().map(|_| ())
        };
        assert!(
            result
                .expect_err("non-JSON entry")
                .to_string()
                .contains("non-symlink .json regular files")
        );
        fs::remove_file(directory.join("record.txt")).expect("remove");
        fs::write(directory.join("target.json"), "{}").expect("target");
        std::os::unix::fs::symlink(directory.join("target.json"), directory.join("link.json"))
            .expect("symlink");
        let result = if reader == 0 {
            storage.read_source_delivery_records().map(|_| ())
        } else {
            storage.read_source_dead_letters().map(|_| ())
        };
        assert!(
            result
                .expect_err("symlink entry")
                .to_string()
                .contains("non-symlink .json regular files")
        );
        fs::remove_file(directory.join("link.json")).expect("remove link");
        fs::remove_file(directory.join("target.json")).expect("remove target");
        assert!(
            utf8_file_name(
                std::ffi::OsString::from_vec(vec![0xff]),
                if reader == 0 {
                    "delivery record"
                } else {
                    "dead-letter"
                },
            )
            .is_err()
        );
    }
}

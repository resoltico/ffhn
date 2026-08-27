use std::{collections::BTreeMap, fs};

use tempfile::tempdir;

use super::*;
use crate::graph::{
    CommitCause, CommitManifestParts, GraphPaths, MeasurementId, MeasurementInstanceId, SourceId,
    SourceIdentity,
};

fn fixture() -> (tempfile::TempDir, TrustedSourceDir) {
    let temporary = tempdir().expect("temporary directory");
    let root = temporary.path().join("graph");
    let source_id = SourceId::new("demo").expect("source id");
    fs::create_dir_all(root.join("sources").join(source_id.as_str()).join(".ffhn"))
        .expect("storage root");
    let graph = crate::graph::TrustedGraphRoot::open(GraphPaths::new(root)).expect("graph root");
    (
        temporary,
        graph.open_source(source_id).expect("source root"),
    )
}

#[test]
fn normal_commit_installs_staged_state_and_first_measurement_identity_together() {
    let (_temporary, source) = fixture();
    let identity = SourceIdentity::fresh();
    source
        .apply_source_transition(
            &crate::graph::LineageManifest::new(crate::graph::LineageManifestParts {
                source_id: source.paths().source_id().clone(),
                scope: crate::graph::LineageScope::Init,
                from: None,
                target: identity,
            })
            .expect("init manifest"),
        )
        .expect("init transition");
    let storage = source.open_storage().expect("storage");
    let current = storage
        .read_source_state()
        .expect("source state")
        .expect("current source state");
    let next = current.next_generation().expect("next generation");
    let measurement_id = MeasurementId::new("price").expect("measurement id");
    let measurement_instance_id = MeasurementInstanceId::mint();
    let measurement_state = crate::graph::MeasurementState::fresh(
        next.source_instance_id().clone(),
        measurement_instance_id.clone(),
    );
    let source_bytes = format!(
        "{}\n",
        crate::stable_json::stable_json(&next).expect("source JSON")
    );
    let measurement_bytes = format!(
        "{}\n",
        crate::stable_json::stable_json(&measurement_state).expect("measurement JSON")
    );
    let staged_source = ManifestPath::new("staged/source-state.json").expect("staged source");
    let staged_measurement =
        ManifestPath::new("staged/price-state.json").expect("staged measurement");
    let source_result_hash = storage
        .stage_bytes(&staged_source, source_bytes.as_bytes())
        .expect("stage source state");
    let measurement_result_hash = storage
        .stage_bytes(&staged_measurement, measurement_bytes.as_bytes())
        .expect("stage measurement state");
    let prior_hash = crate::stable_json::sha256_hex(
        &fs::read(source.paths().source_state_file()).expect("prior source state"),
    );
    let manifest = CommitManifest::new(CommitManifestParts {
        source_id: source.paths().source_id().clone(),
        source_instance_id: next.source_instance_id().clone(),
        generation: next.generation(),
        touched_measurement_instances: BTreeMap::new(),
        identity_measurement_additions: BTreeMap::from([(
            measurement_id.clone(),
            measurement_instance_id.clone(),
        )]),
        cause: CommitCause::Acquisition,
        entries: vec![
            CommitOperation::Install {
                final_path: ManifestPath::new("source-state.json").expect("source final"),
                staged_path: staged_source,
                expected_prior_sha256: Some(prior_hash),
                result_sha256: source_result_hash,
            },
            CommitOperation::Install {
                final_path: ManifestPath::new("measurements/price/state.json")
                    .expect("measurement final"),
                staged_path: staged_measurement,
                expected_prior_sha256: None,
                result_sha256: measurement_result_hash,
            },
        ],
    })
    .expect("commit manifest");

    storage
        .write_commit_manifest(&manifest)
        .expect("durable commit point");
    source.recover_normal_commit().expect("crash recovery");
    assert!(storage.read_commit_manifest().expect("manifest").is_none());
    assert_eq!(
        source
            .read_identity()
            .expect("identity")
            .expect("committed identity")
            .measurements()
            .get(&measurement_id)
            .expect("measurement identity")
            .measurement_instance_id(),
        &measurement_instance_id
    );
    assert_eq!(
        source
            .open_storage()
            .expect("storage after commit")
            .read_source_state()
            .expect("source state")
            .expect("installed source state"),
        next
    );
    assert_eq!(
        source
            .open_storage()
            .expect("storage after commit")
            .read_measurement_state(&measurement_id)
            .expect("measurement state")
            .expect("installed measurement state"),
        measurement_state
    );
}

#[test]
fn staged_bytes_without_a_durable_commit_point_are_removed_before_normal_work() {
    let (_temporary, source) = fixture();
    let identity = SourceIdentity::fresh();
    source
        .apply_source_transition(
            &crate::graph::LineageManifest::new(crate::graph::LineageManifestParts {
                source_id: source.paths().source_id().clone(),
                scope: crate::graph::LineageScope::Init,
                from: None,
                target: identity,
            })
            .expect("init manifest"),
        )
        .expect("init transition");
    let storage = source.open_storage().expect("storage");
    let staged = ManifestPath::new("staged/orphan.json").expect("staged path");
    storage
        .stage_bytes(&staged, b"orphan")
        .expect("stage bytes");
    assert!(
        source
            .paths()
            .storage_dir()
            .join("staged/orphan.json")
            .exists()
    );
    source
        .recover_normal_commit()
        .expect("no-manifest recovery");
    assert!(!source.paths().storage_dir().join("staged").exists());
    assert!(
        storage
            .stage_bytes(
                &ManifestPath::new("not-staged.json").expect("path"),
                b"forbidden",
            )
            .is_err()
    );
    assert!(
        map_staging_removal(
            Err(std::io::Error::other("failed")),
            std::path::Path::new("staged"),
        )
        .is_err()
    );
    #[cfg(unix)]
    {
        let target = source.paths().storage_dir().join("staged-target");
        fs::create_dir(&target).expect("staged target");
        std::os::unix::fs::symlink(&target, source.paths().storage_dir().join("staged"))
            .expect("staged symlink");
        assert!(
            source
                .recover_normal_commit()
                .expect_err("symlink staging root")
                .to_string()
                .contains("reserved commit staging root must be a non-symlink directory")
        );
    }
}

fn ready_source() -> (tempfile::TempDir, TrustedSourceDir, SourceIdentity) {
    let (temporary, source) = fixture();
    let identity = SourceIdentity::fresh();
    source
        .apply_source_transition(
            &crate::graph::LineageManifest::new(crate::graph::LineageManifestParts {
                source_id: source.paths().source_id().clone(),
                scope: crate::graph::LineageScope::Init,
                from: None,
                target: identity.clone(),
            })
            .expect("init manifest"),
        )
        .expect("init transition");
    (temporary, source, identity)
}

#[test]
fn staging_existing_hash_and_replay_install_remove_guards_are_complete() {
    let (_temporary, source, _identity) = ready_source();
    let storage = source.open_storage().expect("storage");
    let staged = ManifestPath::new("staged/value.json").expect("staged");
    let hash = storage.stage_bytes(&staged, b"value").expect("stage");
    assert!(storage.stage_bytes(&staged, b"again").is_err());
    assert_eq!(
        storage
            .existing_file_sha256(&staged)
            .expect("existing hash"),
        Some(hash.clone())
    );
    assert!(
        storage
            .existing_file_sha256(&ManifestPath::new("staged/absent.json").expect("absent"))
            .expect("absent hash")
            .is_none()
    );
    let json_staged = ManifestPath::new("staged/json.json").expect("JSON staged");
    assert_eq!(
        storage
            .stage_json(&json_staged, &serde_json::json!({"b": 1, "a": 2}))
            .expect("stage JSON")
            .len(),
        64
    );

    let final_path = ManifestPath::new("source-state.json").expect("final");
    let current_hash = storage
        .existing_file_sha256(&final_path)
        .expect("hash")
        .expect("state hash");
    storage
        .replay_install(
            &final_path,
            &ManifestPath::new("staged/absent.json").expect("staged"),
            Some("0".repeat(64).as_str()),
            &current_hash,
        )
        .expect("already installed result");
    assert!(
        storage
            .replay_install(
                &final_path,
                &ManifestPath::new("staged/absent.json").expect("staged"),
                Some("0".repeat(64).as_str()),
                &"1".repeat(64),
            )
            .is_err()
    );
    assert!(
        storage
            .replay_install(
                &ManifestPath::new(format!("source-outbox/{}--route.json", "a".repeat(64)))
                    .expect("final"),
                &ManifestPath::new("staged/absent.json").expect("staged"),
                Some(&current_hash),
                &"1".repeat(64),
            )
            .is_err()
    );
    assert!(
        storage
            .replay_install(
                &ManifestPath::new(format!("source-outbox/{}--route.json", "a".repeat(64)))
                    .expect("final"),
                &ManifestPath::new("staged/absent.json").expect("staged"),
                None,
                &"1".repeat(64),
            )
            .is_err()
    );

    let bad_staged = ManifestPath::new("staged/bad.json").expect("staged");
    storage
        .stage_bytes(&bad_staged, b"bad")
        .expect("bad staged");
    assert!(
        storage
            .replay_install(
                &ManifestPath::new(format!("source-outbox/{}--route.json", "a".repeat(64)))
                    .expect("final"),
                &bad_staged,
                None,
                &"1".repeat(64),
            )
            .is_err()
    );

    let removable = ManifestPath::new(format!("source-outbox/{}--route.json", "b".repeat(64)))
        .expect("remove path");
    let full = source.paths().storage_dir().join(removable.as_str());
    fs::create_dir_all(full.parent().expect("parent")).expect("parent");
    fs::write(&full, b"record").expect("record");
    assert!(storage.replay_remove(&removable, &"0".repeat(64)).is_err());
    let record_hash = crate::stable_json::sha256_hex(b"record");
    storage
        .replay_remove(&removable, &record_hash)
        .expect("remove");
    assert!(!full.exists());
    storage
        .replay_remove(&removable, &record_hash)
        .expect("idempotent remove");
}

#[test]
fn normal_commit_refuses_foreign_source_missing_authority_state_and_bad_staging_root() {
    let (_temporary, source) = fixture();
    let identity = SourceIdentity::fresh();
    let manifest = CommitManifest::new(CommitManifestParts {
        source_id: SourceId::new("foreign").expect("source id"),
        source_instance_id: identity.source_instance_id().clone(),
        generation: 1,
        touched_measurement_instances: BTreeMap::new(),
        identity_measurement_additions: BTreeMap::new(),
        cause: CommitCause::Acquisition,
        entries: vec![CommitOperation::Install {
            final_path: ManifestPath::new("source-state.json").expect("final"),
            staged_path: ManifestPath::new("staged/source.json").expect("staged"),
            expected_prior_sha256: None,
            result_sha256: "a".repeat(64),
        }],
    })
    .expect("manifest");
    assert!(
        source
            .require_commit_source(&manifest)
            .expect_err("foreign source")
            .to_string()
            .contains("source_id does not match the source directory")
    );
    assert!(source.apply_normal_commit(&manifest).is_err());

    let storage = source.open_storage().expect("storage");
    fs::write(
        source.paths().storage_dir().join("staged"),
        "not a directory",
    )
    .expect("bad staging root");
    assert!(
        source
            .recover_normal_commit()
            .expect_err("file staging root")
            .to_string()
            .contains("reserved commit staging root must be a non-symlink directory")
    );
    fs::remove_file(source.paths().storage_dir().join("staged")).expect("remove bad staging");

    let local = CommitManifest::new(CommitManifestParts {
        source_id: source.paths().source_id().clone(),
        source_instance_id: identity.source_instance_id().clone(),
        generation: 1,
        touched_measurement_instances: BTreeMap::new(),
        identity_measurement_additions: BTreeMap::new(),
        cause: CommitCause::Acquisition,
        entries: vec![CommitOperation::Install {
            final_path: ManifestPath::new("source-state.json").expect("final"),
            staged_path: ManifestPath::new("staged/source.json").expect("staged"),
            expected_prior_sha256: None,
            result_sha256: "a".repeat(64),
        }],
    })
    .expect("local manifest");
    source
        .require_commit_source(&local)
        .expect("local commit source");
    storage.write_commit_manifest(&local).expect("manifest");
    assert!(source.recover_normal_commit().is_err());
    source.write_identity(&identity).expect("identity");
    assert!(source.recover_normal_commit().is_err());
}

#[test]
fn recovery_rejects_foreign_manifest_installed_generation_and_conflicting_identity_addition() {
    let (_temporary, source, identity) = ready_source();
    let storage = source.open_storage().expect("storage");
    let current = storage.read_source_state().expect("state").expect("state");
    let staged = ManifestPath::new("staged/current.json").expect("staged");
    let result_hash = storage.stage_json(&staged, &current).expect("stage");
    let manifest = CommitManifest::new(CommitManifestParts {
        source_id: source.paths().source_id().clone(),
        source_instance_id: current.source_instance_id().clone(),
        generation: 2,
        touched_measurement_instances: BTreeMap::new(),
        identity_measurement_additions: BTreeMap::new(),
        cause: CommitCause::Acquisition,
        entries: vec![CommitOperation::Install {
            final_path: ManifestPath::new("source-state.json").expect("final"),
            staged_path: staged,
            expected_prior_sha256: storage.source_state_sha256().expect("hash"),
            result_sha256: result_hash,
        }],
    })
    .expect("manifest");
    storage.write_commit_manifest(&manifest).expect("manifest");
    assert!(source.recover_normal_commit().is_err());
    fs::remove_file(source.paths().commit_manifest_file()).expect("remove manifest");

    let foreign_state = crate::graph::SourceState::fresh(crate::graph::SourceInstanceId::mint())
        .next_generation()
        .expect("generation");
    let staged = ManifestPath::new("staged/foreign-installed.json").expect("staged");
    let result_hash = storage.stage_json(&staged, &foreign_state).expect("stage");
    let foreign_installed = CommitManifest::new(CommitManifestParts {
        source_id: source.paths().source_id().clone(),
        source_instance_id: identity.source_instance_id().clone(),
        generation: 2,
        touched_measurement_instances: BTreeMap::new(),
        identity_measurement_additions: BTreeMap::new(),
        cause: CommitCause::Acquisition,
        entries: vec![CommitOperation::Install {
            final_path: ManifestPath::new("source-state.json").expect("final"),
            staged_path: staged,
            expected_prior_sha256: storage.source_state_sha256().expect("hash"),
            result_sha256: result_hash,
        }],
    })
    .expect("foreign installed manifest");
    storage
        .write_commit_manifest(&foreign_installed)
        .expect("manifest");
    assert!(source.recover_normal_commit().is_err());
    fs::remove_file(source.paths().commit_manifest_file()).expect("remove manifest");

    let foreign = CommitManifest::new(CommitManifestParts {
        source_id: source.paths().source_id().clone(),
        source_instance_id: crate::graph::SourceInstanceId::mint(),
        generation: 2,
        touched_measurement_instances: BTreeMap::new(),
        identity_measurement_additions: BTreeMap::new(),
        cause: CommitCause::Acquisition,
        entries: vec![CommitOperation::Install {
            final_path: ManifestPath::new("source-state.json").expect("final"),
            staged_path: ManifestPath::new("staged/foreign.json").expect("staged"),
            expected_prior_sha256: storage.source_state_sha256().expect("hash"),
            result_sha256: "a".repeat(64),
        }],
    })
    .expect("foreign manifest");
    storage.write_commit_manifest(&foreign).expect("manifest");
    assert!(source.recover_normal_commit().is_err());
    fs::remove_file(source.paths().commit_manifest_file()).expect("remove manifest");

    let measurement_id = MeasurementId::new("price").expect("measurement");
    let mut known = identity.clone();
    known
        .register_measurement_instance(measurement_id.clone(), MeasurementInstanceId::mint())
        .expect("known");
    let conflicting = CommitManifest::new(CommitManifestParts {
        source_id: source.paths().source_id().clone(),
        source_instance_id: identity.source_instance_id().clone(),
        generation: 2,
        touched_measurement_instances: BTreeMap::new(),
        identity_measurement_additions: BTreeMap::from([(
            measurement_id.clone(),
            MeasurementInstanceId::mint(),
        )]),
        cause: CommitCause::Acquisition,
        entries: vec![
            CommitOperation::Install {
                final_path: ManifestPath::new("source-state.json").expect("final"),
                staged_path: ManifestPath::new("staged/source.json").expect("staged"),
                expected_prior_sha256: storage.source_state_sha256().expect("hash"),
                result_sha256: "a".repeat(64),
            },
            CommitOperation::Install {
                final_path: ManifestPath::new("measurements/price/state.json").expect("final"),
                staged_path: ManifestPath::new("staged/price.json").expect("staged"),
                expected_prior_sha256: None,
                result_sha256: "b".repeat(64),
            },
        ],
    })
    .expect("conflicting manifest");
    assert!(apply_identity_additions(&mut known, &conflicting).is_err());
    let same_instance = known
        .measurements()
        .get(&measurement_id)
        .expect("known")
        .measurement_instance_id()
        .clone();
    let same = CommitManifest::new(CommitManifestParts {
        source_id: source.paths().source_id().clone(),
        source_instance_id: identity.source_instance_id().clone(),
        generation: 2,
        touched_measurement_instances: BTreeMap::new(),
        identity_measurement_additions: BTreeMap::from([(measurement_id, same_instance)]),
        cause: CommitCause::Acquisition,
        entries: conflicting.entries().to_vec(),
    })
    .expect("same manifest");
    apply_identity_additions(&mut known, &same).expect("idempotent addition");
}

#[test]
fn manifest_parent_and_regular_byte_helpers_reject_non_directory_and_non_file_nodes() {
    let (_temporary, source, _identity) = ready_source();
    let storage = source.open_storage().expect("storage");
    fs::write(source.paths().storage_dir().join("not-dir"), "file").expect("file");
    assert!(
        open_existing_child(
            &storage.dir,
            "not-dir",
            &source.paths().storage_dir().join("not-dir"),
        )
        .is_err()
    );
    fs::create_dir(source.paths().storage_dir().join("directory.json")).expect("directory");
    assert!(
        read_regular_bytes(
            &storage.dir,
            "directory.json",
            &source.paths().storage_dir().join("directory.json"),
        )
        .is_err()
    );
    #[cfg(unix)]
    {
        fs::create_dir(source.paths().storage_dir().join("target-dir")).expect("target dir");
        std::os::unix::fs::symlink(
            source.paths().storage_dir().join("target-dir"),
            source.paths().storage_dir().join("linked-dir"),
        )
        .expect("directory symlink");
        assert!(
            open_existing_child(
                &storage.dir,
                "linked-dir",
                &source.paths().storage_dir().join("linked-dir"),
            )
            .is_err()
        );
        fs::write(source.paths().storage_dir().join("target-file"), "file").expect("target file");
        std::os::unix::fs::symlink(
            source.paths().storage_dir().join("target-file"),
            source.paths().storage_dir().join("linked-file"),
        )
        .expect("file symlink");
        assert!(
            read_regular_bytes(
                &storage.dir,
                "linked-file",
                &source.paths().storage_dir().join("linked-file"),
            )
            .is_err()
        );
    }
}

#[path = "tests/mutation_contracts.rs"]
mod mutation_contracts;

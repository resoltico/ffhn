use std::fs;

use tempfile::tempdir;

use super::*;
use crate::graph::{
    GraphIdentity, LineageManifestParts, LineageScope, MeasurementIdentity, MeasurementInstanceId,
    SourceId,
};

fn fixture() -> (tempfile::TempDir, GraphPaths, SourceId) {
    let temporary = tempdir().expect("temporary directory");
    let root = temporary.path().join("graph");
    let source_id = SourceId::new("demo").expect("source id");
    fs::create_dir_all(root.join("sources").join(source_id.as_str()).join(".ffhn"))
        .expect("storage root");
    (temporary, GraphPaths::new(root), source_id)
}

#[test]
fn trusted_root_and_documents_preserve_the_fixed_hierarchy() {
    let (_temporary, paths, source_id) = fixture();
    let graph = TrustedGraphRoot::open(paths).expect("graph root");
    let graph_identity =
        GraphIdentity::new("2026-08-25T00:00:00Z".to_owned()).expect("graph identity");
    graph
        .write_graph_identity(&graph_identity)
        .expect("graph identity write");
    assert_eq!(
        graph.read_graph_identity().expect("graph identity read"),
        Some(graph_identity)
    );
    let source = graph.open_source(source_id).expect("source root");
    let identity = SourceIdentity::fresh();
    source.write_identity(&identity).expect("identity write");
    let storage = source.open_storage().expect("storage root");
    let state = SourceState::fresh(identity.source_instance_id().clone());
    storage.write_source_state(&state).expect("state write");
    let measurement_id = MeasurementId::new("price").expect("measurement id");
    let measurement_state = MeasurementState::fresh(
        identity.source_instance_id().clone(),
        MeasurementInstanceId::mint(),
    );
    storage
        .write_measurement_state(&measurement_id, &measurement_state)
        .expect("measurement state write");

    assert_eq!(
        source.read_identity().expect("identity read"),
        Some(identity)
    );
    assert_eq!(
        storage.read_source_state().expect("state read"),
        Some(state)
    );
    assert_eq!(
        storage
            .read_measurement_state(&measurement_id)
            .expect("measurement state read"),
        Some(measurement_state)
    );
}

#[test]
fn source_scope_transitions_replace_storage_and_recover_idempotently() {
    let (_temporary, paths, source_id) = fixture();
    let graph = TrustedGraphRoot::open(paths).expect("graph root");
    let source = graph.open_source(source_id).expect("source root");
    let initial = SourceIdentity::fresh();
    source
        .apply_source_transition(
            &LineageManifest::new(LineageManifestParts {
                source_id: source.paths().source_id().clone(),
                scope: LineageScope::Init,
                from: None,
                target: initial.clone(),
            })
            .expect("init manifest"),
        )
        .expect("init transition");
    let replacement = SourceIdentity::fresh();
    source
        .apply_source_transition(
            &LineageManifest::new(LineageManifestParts {
                source_id: source.paths().source_id().clone(),
                scope: LineageScope::SourceReset,
                from: Some(initial),
                target: replacement.clone(),
            })
            .expect("reset manifest"),
        )
        .expect("source reset");

    assert_eq!(
        source.read_identity().expect("identity"),
        Some(replacement.clone())
    );
    assert!(source.read_lineage_manifest().expect("manifest").is_none());
    assert!(!source.paths().tombstone_dir().exists());
    assert_eq!(
        source
            .open_storage()
            .expect("storage")
            .read_source_state()
            .expect("state")
            .expect("fresh state")
            .source_instance_id(),
        replacement.source_instance_id()
    );
    source
        .recover_lineage_transition()
        .expect("idempotent recovery");
}

#[test]
fn measurement_reset_replaces_only_one_measurement_subtree_and_lineage_entry() {
    let (_temporary, paths, source_id) = fixture();
    let graph = TrustedGraphRoot::open(paths).expect("graph root");
    let source = graph.open_source(source_id).expect("source root");
    let initial = SourceIdentity::fresh();
    source
        .apply_source_transition(
            &LineageManifest::new(LineageManifestParts {
                source_id: source.paths().source_id().clone(),
                scope: LineageScope::Init,
                from: None,
                target: initial,
            })
            .expect("init manifest"),
        )
        .expect("init transition");
    let measurement_id = MeasurementId::new("price").expect("measurement id");
    let mut before = source
        .read_identity()
        .expect("identity read")
        .expect("identity");
    let old_measurement = MeasurementIdentity::fresh();
    before
        .register_measurement(measurement_id.clone(), old_measurement.clone())
        .expect("first measurement registration");
    source.write_identity(&before).expect("identity update");
    let storage = source.open_storage().expect("storage");
    let source_state_bytes = fs::read(source.paths().source_state_file()).expect("source state");
    storage
        .write_measurement_state(
            &measurement_id,
            &MeasurementState::fresh(
                before.source_instance_id().clone(),
                old_measurement.measurement_instance_id().clone(),
            ),
        )
        .expect("old measurement state");
    drop(storage);
    let mut target = before.clone();
    let replacement = target
        .reset_measurement(measurement_id.clone())
        .expect("target measurement reset");
    source
        .apply_measurement_transition(
            &LineageManifest::new(LineageManifestParts {
                source_id: source.paths().source_id().clone(),
                scope: LineageScope::MeasurementReset {
                    measurement_id: measurement_id.clone(),
                },
                from: Some(before),
                target: target.clone(),
            })
            .expect("measurement manifest"),
        )
        .expect("measurement reset");

    assert_eq!(source.read_identity().expect("identity"), Some(target));
    assert_eq!(
        fs::read(source.paths().source_state_file()).expect("source state after"),
        source_state_bytes
    );
    let state = source
        .open_storage()
        .expect("storage after reset")
        .read_measurement_state(&measurement_id)
        .expect("measurement state")
        .expect("fresh measurement state");
    assert_eq!(
        state.measurement_instance_id(),
        replacement.measurement_instance_id()
    );
    assert!(!source.paths().tombstone_dir().exists());
}

#[cfg(unix)]
#[test]
fn trusted_root_rejects_symlinked_graph_sources_and_storage_components() {
    let (temporary, paths, source_id) = fixture();
    let root_link = temporary.path().join("graph-link");
    std::os::unix::fs::symlink(paths.root(), &root_link).expect("graph root symlink");
    assert!(TrustedGraphRoot::open(GraphPaths::new(root_link)).is_err());
    let root_file = temporary.path().join("graph-file");
    fs::write(&root_file, "file").expect("graph file");
    assert!(TrustedGraphRoot::open(GraphPaths::new(root_file)).is_err());

    let graph = TrustedGraphRoot::open(paths.clone()).expect("graph root");
    let source = graph.open_source(source_id.clone()).expect("source root");
    fs::write(paths.root().join("identity-target"), "{}").expect("identity target");
    std::os::unix::fs::symlink(paths.root().join("identity-target"), paths.identity_file())
        .expect("identity symlink");
    assert!(graph.read_graph_identity().is_err());
    fs::write(source.paths().storage_dir().join("state-target"), "{}").expect("state target");
    std::os::unix::fs::symlink(
        source.paths().storage_dir().join("state-target"),
        source.paths().source_state_file(),
    )
    .expect("state symlink");
    let opened = source.open_storage().expect("storage");
    assert!(opened.read_source_state().is_err());
    assert!(opened.source_state_sha256().is_err());
    fs::write(source.paths().source_dir().join("manifest-target"), "{}").expect("manifest target");
    std::os::unix::fs::symlink(
        source.paths().source_dir().join("manifest-target"),
        source.paths().lineage_manifest_file(),
    )
    .expect("manifest symlink");
    assert!(source.read_lineage_manifest_bytes().is_err());
    assert!(
        remove_regular_file(
            &source.dir,
            ".ffhn-lineage.manifest",
            &source.paths().lineage_manifest_file(),
            "lineage manifest",
        )
        .is_err()
    );

    let storage = paths.source(source_id.clone()).storage_dir();
    let replacement = storage.with_extension("replacement");
    fs::rename(&storage, &replacement).expect("move storage");
    std::os::unix::fs::symlink(&replacement, &storage).expect("storage symlink");
    let graph = TrustedGraphRoot::open(paths).expect("graph root");
    let source = graph.open_source(source_id).expect("source root");
    assert!(source.open_storage().is_err());
}

#[test]
fn trusted_document_reads_hashes_manifest_bytes_and_creation_boundaries_are_complete() {
    let temporary = tempdir().expect("temporary");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo")).expect("source directory");
    let paths = GraphPaths::new(root.clone());
    let graph = TrustedGraphRoot::open(paths.clone()).expect("graph");
    assert_eq!(graph.paths().root(), paths.root());
    assert!(graph.read_graph_identity().expect("identity").is_none());
    let source = graph
        .open_source(SourceId::new("demo").expect("source id"))
        .expect("source");
    assert_eq!(source.paths().source_id().as_str(), "demo");
    assert!(source.read_identity().expect("identity").is_none());
    assert!(source.read_lineage_manifest().expect("manifest").is_none());
    assert!(
        source
            .read_lineage_manifest_bytes()
            .expect("bytes")
            .is_none()
    );
    let storage = source.create_storage().expect("create storage");
    assert_eq!(storage.paths().source_id().as_str(), "demo");
    assert!(source.create_storage().is_err());
    assert!(storage.read_source_state().expect("state").is_none());
    assert!(storage.source_state_sha256().expect("hash").is_none());
    assert!(storage.read_commit_manifest().expect("manifest").is_none());
    assert!(
        storage
            .read_commit_manifest_bytes()
            .expect("bytes")
            .is_none()
    );
    let measurement_id = MeasurementId::new("price").expect("measurement id");
    assert!(
        storage
            .read_measurement_state(&measurement_id)
            .expect("measurement")
            .is_none()
    );
    assert!(
        storage
            .measurement_state_sha256(&measurement_id)
            .expect("hash")
            .is_none()
    );

    let identity = SourceIdentity::fresh();
    source.write_identity(&identity).expect("identity");
    let state = SourceState::fresh(identity.source_instance_id().clone());
    storage.write_source_state(&state).expect("state");
    assert_eq!(
        storage
            .source_state_sha256()
            .expect("hash")
            .expect("present")
            .len(),
        64
    );
    let measurement_state = MeasurementState::fresh(
        identity.source_instance_id().clone(),
        MeasurementInstanceId::mint(),
    );
    storage
        .write_measurement_state(&measurement_id, &measurement_state)
        .expect("measurement");
    assert_eq!(
        storage
            .measurement_state_sha256(&measurement_id)
            .expect("hash")
            .expect("present")
            .len(),
        64
    );

    let lineage = LineageManifest::new(LineageManifestParts {
        source_id: SourceId::new("demo").expect("source id"),
        scope: LineageScope::SourceReset,
        from: Some(identity.clone()),
        target: SourceIdentity::fresh(),
    })
    .expect("lineage manifest");
    source
        .write_lineage_manifest(&lineage)
        .expect("lineage write");
    assert_eq!(
        source.read_lineage_manifest().expect("lineage"),
        Some(lineage)
    );
    assert!(
        source
            .read_lineage_manifest_bytes()
            .expect("bytes")
            .is_some()
    );

    let manifest = CommitManifest::new(super::super::CommitManifestParts {
        source_id: SourceId::new("demo").expect("source id"),
        source_instance_id: identity.source_instance_id().clone(),
        generation: 2,
        touched_measurement_instances: std::collections::BTreeMap::new(),
        identity_measurement_additions: std::collections::BTreeMap::new(),
        cause: super::super::CommitCause::Acquisition,
        entries: vec![super::super::CommitOperation::Install {
            final_path: super::super::ManifestPath::new("source-state.json").expect("final"),
            staged_path: super::super::ManifestPath::new("staged/source-state.json")
                .expect("staged"),
            expected_prior_sha256: storage.source_state_sha256().expect("hash"),
            result_sha256: "a".repeat(64),
        }],
    })
    .expect("commit manifest");
    storage
        .write_commit_manifest(&manifest)
        .expect("manifest write");
    assert_eq!(
        storage.read_commit_manifest().expect("manifest"),
        Some(manifest)
    );
    assert!(
        storage
            .read_commit_manifest_bytes()
            .expect("bytes")
            .is_some()
    );
}

#[test]
fn regular_file_and_child_directory_helpers_reject_nonregular_nodes_and_remove_all_tombstone_shapes()
 {
    let (_temporary, paths, source_id) = fixture();
    let graph = TrustedGraphRoot::open(paths).expect("graph");
    let source = graph.open_source(source_id).expect("source");
    let storage = source.open_storage().expect("storage");

    fs::create_dir(source.paths().source_state_file()).expect("state directory");
    assert!(storage.read_source_state().is_err());
    assert!(storage.source_state_sha256().is_err());
    fs::remove_dir(source.paths().source_state_file()).expect("remove directory");
    fs::write(source.paths().source_state_file(), "bytes").expect("file");
    remove_regular_file(
        &storage.dir,
        "source-state.json",
        &source.paths().source_state_file(),
        "source state",
    )
    .expect("remove file");
    fs::create_dir(source.paths().source_state_file()).expect("directory");
    assert!(
        remove_regular_file(
            &storage.dir,
            "source-state.json",
            &source.paths().source_state_file(),
            "source state",
        )
        .is_err()
    );
    fs::remove_dir(source.paths().source_state_file()).expect("remove directory");

    assert!(
        open_optional_real_child(
            &storage.dir,
            "absent",
            &source.paths().storage_dir().join("absent"),
            "absent",
        )
        .expect("optional")
        .is_none()
    );
    let created = open_or_create_real_child(
        &storage.dir,
        "created",
        &source.paths().storage_dir().join("created"),
        "created",
    )
    .expect("created");
    drop(created);
    assert!(
        open_or_create_real_child(
            &storage.dir,
            "created",
            &source.paths().storage_dir().join("created"),
            "created",
        )
        .is_ok()
    );
    fs::write(source.paths().storage_dir().join("not-dir"), "file").expect("file");
    assert!(
        open_optional_real_child(
            &storage.dir,
            "not-dir",
            &source.paths().storage_dir().join("not-dir"),
            "not dir",
        )
        .is_err()
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            source.paths().storage_dir().join("created"),
            source.paths().storage_dir().join("optional-link"),
        )
        .expect("optional symlink");
        assert!(
            open_optional_real_child(
                &storage.dir,
                "optional-link",
                &source.paths().storage_dir().join("optional-link"),
                "optional link",
            )
            .is_err()
        );
    }

    remove_tombstone(&source.dir, &source.paths().tombstone_dir()).expect("absent tombstone");
    fs::write(source.paths().tombstone_dir(), "file").expect("file tombstone");
    remove_tombstone(&source.dir, &source.paths().tombstone_dir()).expect("file tombstone");
    fs::create_dir(source.paths().tombstone_dir()).expect("directory tombstone");
    remove_tombstone(&source.dir, &source.paths().tombstone_dir()).expect("dir tombstone");

    fs::create_dir(source.paths().lineage_manifest_file()).expect("manifest directory");
    assert!(source.read_lineage_manifest_bytes().is_err());
    fs::remove_dir(source.paths().lineage_manifest_file()).expect("remove manifest directory");

    fs::create_dir(source.paths().source_dir().join("blocked-output")).expect("blocked output");
    assert!(
        atomic_write_text(
            &source.dir,
            "blocked-output",
            "text",
            &source.paths().source_dir().join("blocked-output"),
        )
        .is_err()
    );
    assert!(
        fs::read_dir(source.paths().source_dir())
            .expect("source entries")
            .all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".ffhn-stage-"))
    );
    assert_eq!(
        optional_fs_entry::<()>(
            Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
            std::path::Path::new("entry"),
        )
        .expect("missing"),
        None
    );
    assert!(
        optional_fs_entry::<()>(
            Err(std::io::Error::other("failed")),
            std::path::Path::new("entry"),
        )
        .is_err()
    );
    assert!(require_missing_fs_entry(Ok(()), std::path::Path::new("entry"), "entry").is_err());
    assert!(
        require_missing_fs_entry::<()>(
            Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
            std::path::Path::new("entry"),
            "entry",
        )
        .is_ok()
    );
}

#[path = "tests/mutation_contracts.rs"]
mod mutation_contracts;

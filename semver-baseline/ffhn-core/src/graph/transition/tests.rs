use std::fs;

use tempfile::tempdir;

use crate::graph::{GraphPaths, LineageScope, SourceId, TrustedGraphRoot};

#[test]
fn blind_source_reset_replaces_unreadable_authority_without_reading_old_storage() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn")).expect("source storage");
    let graph = TrustedGraphRoot::open(GraphPaths::new(root)).expect("graph root");
    let source = graph
        .open_source(SourceId::new("demo").expect("source id"))
        .expect("source");
    fs::write(source.paths().identity_file(), b"not JSON").expect("corrupt identity");
    fs::write(source.paths().source_state_file(), b"also not JSON").expect("corrupt state");
    source
        .apply_blind_source_transition(LineageScope::SourceReset)
        .expect("blind reset");
    let identity = source
        .read_identity()
        .expect("identity read")
        .expect("fresh identity");
    let state = source
        .open_storage()
        .expect("storage")
        .read_source_state()
        .expect("state read")
        .expect("fresh state");
    assert_eq!(state.source_instance_id(), identity.source_instance_id());
    assert!(
        source
            .read_lineage_manifest()
            .expect("manifest read")
            .is_none()
    );
}

#[test]
fn transition_entrypoints_reject_wrong_scope_source_and_unresolvable_authority() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn")).expect("source storage");
    let graph = TrustedGraphRoot::open(GraphPaths::new(root)).expect("graph");
    let source = graph
        .open_source(SourceId::new("demo").expect("source"))
        .expect("source");
    assert!(
        source
            .apply_blind_source_transition(LineageScope::MeasurementReset {
                measurement_id: crate::graph::MeasurementId::new("price").expect("measurement"),
            })
            .is_err()
    );

    let target = crate::graph::SourceIdentity::fresh();
    let blind = crate::graph::LineageManifest::new(crate::graph::LineageManifestParts {
        source_id: SourceId::new("demo").expect("source"),
        scope: LineageScope::SourceReset,
        from: None,
        target,
    })
    .expect("blind manifest");
    assert!(source.apply_source_transition(&blind).is_err());

    let wrong = crate::graph::LineageManifest::new(crate::graph::LineageManifestParts {
        source_id: SourceId::new("other").expect("other"),
        scope: LineageScope::Init,
        from: None,
        target: crate::graph::SourceIdentity::fresh(),
    })
    .expect("wrong manifest");
    assert!(
        source
            .require_measurement_scope(&wrong)
            .expect_err("foreign measurement scope")
            .to_string()
            .contains("source_id does not match the source directory")
    );
    assert!(source.apply_source_transition(&wrong).is_err());
    assert!(source.apply_measurement_transition(&wrong).is_err());

    let init = crate::graph::LineageManifest::new(crate::graph::LineageManifestParts {
        source_id: SourceId::new("demo").expect("source"),
        scope: LineageScope::Init,
        from: None,
        target: crate::graph::SourceIdentity::fresh(),
    })
    .expect("init");
    assert!(
        source
            .require_measurement_scope(&init)
            .expect_err("non-measurement scope")
            .to_string()
            .contains("requires measurement_reset lineage scope")
    );
    assert!(source.apply_measurement_transition(&init).is_err());

    let from = crate::graph::SourceIdentity::fresh();
    let reset = crate::graph::LineageManifest::new(crate::graph::LineageManifestParts {
        source_id: SourceId::new("demo").expect("source"),
        scope: LineageScope::SourceReset,
        from: Some(from),
        target: crate::graph::SourceIdentity::fresh(),
    })
    .expect("reset");
    source.write_lineage_manifest(&reset).expect("manifest");
    source
        .write_identity(&crate::graph::SourceIdentity::fresh())
        .expect("foreign identity");
    assert!(source.recover_lineage_transition().is_err());
    assert!(
        super::map_transition_rename(
            Err(std::io::Error::other("failed")),
            std::path::Path::new("transition"),
        )
        .is_err()
    );
}

#[test]
fn blind_initialization_and_orphan_measurement_reset_create_fresh_authority_and_state() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo")).expect("source dir");
    let graph = TrustedGraphRoot::open(GraphPaths::new(root)).expect("graph");
    let source = graph
        .open_source(SourceId::new("demo").expect("source"))
        .expect("source");
    source
        .apply_blind_source_transition(LineageScope::Init)
        .expect("blind init");
    let before = source.read_identity().expect("identity").expect("identity");
    let measurement_id = crate::graph::MeasurementId::new("orphan").expect("measurement");
    let mut target = before.clone();
    target
        .reset_measurement(measurement_id.clone())
        .expect("orphan reset target");
    let manifest = crate::graph::LineageManifest::new(crate::graph::LineageManifestParts {
        source_id: SourceId::new("demo").expect("source"),
        scope: LineageScope::MeasurementReset {
            measurement_id: measurement_id.clone(),
        },
        from: Some(before),
        target: target.clone(),
    })
    .expect("measurement reset");
    source
        .require_measurement_scope(&manifest)
        .expect("local measurement scope");
    assert!(source.apply_source_transition(&manifest).is_err());
    source
        .apply_measurement_transition(&manifest)
        .expect("measurement transition");
    let state = source
        .open_storage()
        .expect("storage")
        .read_measurement_state(&measurement_id)
        .expect("state")
        .expect("state");
    assert_eq!(state.source_instance_id(), target.source_instance_id());
    assert!(
        source
            .swap_measurement_storage(
                &crate::graph::SourceIdentity::fresh(),
                &crate::graph::MeasurementId::new("missing").expect("measurement"),
            )
            .is_err()
    );
}

#[test]
fn recovery_continues_when_target_authority_was_installed_before_the_scope_swap() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn")).expect("storage");
    let graph = TrustedGraphRoot::open(GraphPaths::new(root)).expect("graph");
    let source = graph
        .open_source(SourceId::new("demo").expect("source"))
        .expect("source");
    let from = crate::graph::SourceIdentity::fresh();
    source.write_identity(&from).expect("identity");
    source
        .open_storage()
        .expect("storage")
        .write_source_state(&crate::graph::SourceState::fresh(
            from.source_instance_id().clone(),
        ))
        .expect("state");
    let target = crate::graph::SourceIdentity::fresh();
    let manifest = crate::graph::LineageManifest::new(crate::graph::LineageManifestParts {
        source_id: SourceId::new("demo").expect("source"),
        scope: LineageScope::SourceReset,
        from: Some(from),
        target: target.clone(),
    })
    .expect("manifest");
    source.write_lineage_manifest(&manifest).expect("manifest");
    source.write_identity(&target).expect("target identity");
    source.recover_lineage_transition().expect("continue swap");
    assert_eq!(source.read_identity().expect("identity"), Some(target));
}

use std::fs;

use tempfile::tempdir;

use super::*;
use crate::graph::{
    GraphPaths, LineageManifest, LineageManifestParts, LineageScope, SourceId, SourceIdentity,
    TrustedGraphRoot,
};

#[test]
fn turn_entry_refuses_a_foreign_lineage_manifest_before_normal_state_access() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn")).expect("source storage");
    let graph = TrustedGraphRoot::open(GraphPaths::new(root)).expect("graph root");
    let source = graph
        .open_source(SourceId::new("demo").expect("source id"))
        .expect("source");
    let identity = SourceIdentity::fresh();
    source.write_identity(&identity).expect("identity");
    source
        .open_storage()
        .expect("storage")
        .write_source_state(&crate::graph::SourceState::fresh(
            identity.source_instance_id().clone(),
        ))
        .expect("state");
    source
        .write_lineage_manifest(
            &LineageManifest::new(LineageManifestParts {
                source_id: source.paths().source_id().clone(),
                scope: LineageScope::SourceReset,
                from: Some(SourceIdentity::fresh()),
                target: SourceIdentity::fresh(),
            })
            .expect("foreign manifest"),
        )
        .expect("manifest write");
    assert_eq!(
        source.recover_turn_entry().expect("turn entry"),
        SourceTurnEntry::Unresolvable(UnresolvableManifest::Lineage)
    );
}

#[test]
fn turn_entry_vocabularies_ready_without_storage_and_unresolvable_commit_are_complete() {
    assert_eq!(UnresolvableManifest::Lineage.as_str(), "lineage_manifest");
    assert_eq!(UnresolvableManifest::Commit.as_str(), "commit_manifest");
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo")).expect("source");
    let graph = TrustedGraphRoot::open(GraphPaths::new(root)).expect("graph");
    let source = graph
        .open_source(SourceId::new("demo").expect("source"))
        .expect("source");
    assert_eq!(
        source.recover_turn_entry().expect("ready"),
        SourceTurnEntry::Ready
    );
    fs::create_dir(source.paths().storage_dir()).expect("storage");
    fs::write(source.paths().commit_manifest_file(), "not-json").expect("commit");
    assert_eq!(
        source.recover_turn_entry().expect("commit result"),
        SourceTurnEntry::Unresolvable(UnresolvableManifest::Commit)
    );
}

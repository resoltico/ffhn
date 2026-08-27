use std::fs;

use tempfile::tempdir;

use super::*;
use crate::graph::{GraphPaths, SourceId};

#[test]
fn graph_and_source_writer_locks_are_nonblocking_and_survive_only_as_handles() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo")).expect("source directory");
    let paths = GraphPaths::new(root);
    let first = TrustedGraphRoot::open(paths.clone()).expect("first graph");
    let second = TrustedGraphRoot::open(paths).expect("second graph");
    let graph_lease = first
        .try_acquire_agent_lease()
        .expect("first graph lock")
        .expect("available graph lock");
    assert!(
        second
            .try_acquire_agent_lease()
            .expect("second graph lock")
            .is_none()
    );
    drop(graph_lease);
    assert!(
        second
            .try_acquire_agent_lease()
            .expect("released graph lock")
            .is_some()
    );

    let source_id = SourceId::new("demo").expect("source id");
    let first_source = first.open_source(source_id.clone()).expect("first source");
    let second_source = second.open_source(source_id).expect("second source");
    let source_lease = first_source
        .try_acquire_write_lease()
        .expect("first source lock")
        .expect("available source lock");
    assert!(
        second_source
            .try_acquire_write_lease()
            .expect("second source lock")
            .is_none()
    );
    drop(source_lease);
    assert!(
        second_source
            .try_acquire_write_lease()
            .expect("released source lock")
            .is_some()
    );
}

#[test]
fn source_readers_share_a_generation_but_a_writer_excludes_them() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo")).expect("source directory");
    let paths = GraphPaths::new(root);
    let first = TrustedGraphRoot::open(paths.clone()).expect("first graph");
    let second = TrustedGraphRoot::open(paths).expect("second graph");
    let source_id = SourceId::new("demo").expect("source id");
    let first_source = first.open_source(source_id.clone()).expect("first source");
    let second_source = second.open_source(source_id).expect("second source");

    let first_reader = first_source
        .try_acquire_read_lease()
        .expect("first reader")
        .expect("available reader");
    let second_reader = second_source
        .try_acquire_read_lease()
        .expect("second reader")
        .expect("compatible reader");
    assert!(
        second_source
            .try_acquire_write_lease()
            .expect("writer contention")
            .is_none()
    );
    drop(first_reader);
    drop(second_reader);
    assert!(
        second_source
            .try_acquire_write_lease()
            .expect("released writer")
            .is_some()
    );
}

#[test]
fn lock_entries_must_be_regular_files() {
    let temporary = tempdir().expect("temporary graph");
    let root = temporary.path().join("graph");
    fs::create_dir_all(root.join("sources/demo/.ffhn.lock")).expect("source lock directory");
    fs::create_dir(root.join(".ffhn-agent.lock")).expect("agent lock directory");
    let graph = TrustedGraphRoot::open(GraphPaths::new(root)).expect("graph");
    assert!(
        graph
            .try_acquire_agent_lease()
            .err()
            .expect("directory is not an agent lock")
            .to_string()
            .contains("graph agent lock must be a non-symlink regular file")
    );
    let source = graph
        .open_source(SourceId::new("demo").expect("source"))
        .expect("source");
    for error in [
        source
            .try_acquire_write_lease()
            .err()
            .expect("directory is not a writer lock"),
        source
            .try_acquire_read_lease()
            .err()
            .expect("directory is not a reader lock"),
    ] {
        assert!(
            error
                .to_string()
                .contains("source lock must be a non-symlink regular file")
        );
    }
    #[cfg(unix)]
    {
        fs::remove_dir(graph.paths().agent_lock_file()).expect("remove agent lock directory");
        fs::write(graph.paths().root().join("agent-target"), "lock").expect("agent target");
        std::os::unix::fs::symlink(
            graph.paths().root().join("agent-target"),
            graph.paths().agent_lock_file(),
        )
        .expect("agent lock symlink");
        assert!(
            graph
                .try_acquire_agent_lease()
                .err()
                .expect("symlink is not an agent lock")
                .to_string()
                .contains("graph agent lock must be a non-symlink regular file")
        );

        fs::remove_dir(source.paths().lock_file()).expect("remove source lock directory");
        fs::write(source.paths().source_dir().join("lock-target"), "lock").expect("source target");
        std::os::unix::fs::symlink(
            source.paths().source_dir().join("lock-target"),
            source.paths().lock_file(),
        )
        .expect("source lock symlink");
        assert!(
            source
                .try_acquire_write_lease()
                .err()
                .expect("symlink is not a source lock")
                .to_string()
                .contains("source lock must be a non-symlink regular file")
        );
    }
    assert!(classify_lock_result(Ok(())).expect("acquired"));
    assert!(
        !classify_lock_result(Err(std::io::Error::from(std::io::ErrorKind::WouldBlock)))
            .expect("contended")
    );
    assert!(classify_lock_result(Err(std::io::Error::other("failed"))).is_err());
}

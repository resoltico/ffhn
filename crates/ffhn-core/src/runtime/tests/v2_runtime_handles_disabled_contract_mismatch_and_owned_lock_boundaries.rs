use super::support::*;

#[test]
fn v2_runtime_handles_disabled_contract_mismatch_and_owned_lock_boundaries() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(
        paths.target_dir().join("source.json"),
        r#"{"value":7,"other":8}"#,
    )
    .expect("source");
    run_once(&paths).expect("initial state");

    let disabled_target = fs::read_to_string(paths.target_file())
        .expect("target")
        .replace("enabled = true", "enabled = false");
    fs::write(paths.target_file(), disabled_target).expect("disabled target");
    let disabled_digest = load_target(&paths)
        .expect("disabled target loads")
        .contract_digest_sha256()
        .expect("disabled digest");
    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("state")).expect("JSON");
    state["contract_digest_sha256"] = serde_json::Value::String(disabled_digest);
    fs::write(
        paths.state_file(),
        serde_json::to_vec(&state).expect("state JSON"),
    )
    .expect("matching disabled state");
    let skipped = run_once(&paths).expect("disabled run");
    assert_eq!(skipped.outcome(), RunOutcome::SkippedDisabled);
    assert_eq!(
        serde_json::to_value(skipped).expect("skipped report")["previous_canonical_value"],
        "7"
    );

    write_target(&paths, "integer", "", "/other");
    assert_eq!(
        status(&paths).expect("contract mismatch status").kind(),
        StatusKind::InvalidState
    );

    let (_temporary, mismatched_target_paths) = fixture_paths();
    write_target(&mismatched_target_paths, "integer", "", "/value");
    fs::write(
        mismatched_target_paths.target_dir().join("source.json"),
        r#"{"value":1}"#,
    )
    .expect("source");
    let mismatched_target = fs::read_to_string(mismatched_target_paths.target_file())
        .expect("target")
        .replace("target_id = \"demo\"", "target_id = \"other\"");
    fs::write(mismatched_target_paths.target_file(), mismatched_target).expect("bad target id");
    assert_eq!(
        run_once(&mismatched_target_paths)
            .expect("target mismatch report")
            .outcome(),
        RunOutcome::ConfigInvalid
    );
    assert_eq!(
        status(&mismatched_target_paths)
            .expect("target mismatch status")
            .kind(),
        StatusKind::InvalidConfig
    );

    let (_temporary, mismatched_state_paths) = fixture_paths();
    write_target(&mismatched_state_paths, "integer", "", "/value");
    fs::write(
        mismatched_state_paths.target_dir().join("source.json"),
        r#"{"value":1}"#,
    )
    .expect("source");
    run_once(&mismatched_state_paths).expect("state");
    let mut wrong_state: serde_json::Value =
        serde_json::from_slice(&fs::read(mismatched_state_paths.state_file()).expect("state JSON"))
            .expect("state document");
    wrong_state["target_id"] = serde_json::Value::String("other".to_owned());
    fs::write(
        mismatched_state_paths.state_file(),
        serde_json::to_vec(&wrong_state).expect("wrong state JSON"),
    )
    .expect("wrong state");
    assert_eq!(
        run_once(&mismatched_state_paths)
            .expect("state mismatch report")
            .outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&mismatched_state_paths)
            .expect("state mismatch status")
            .kind(),
        StatusKind::InvalidState
    );

    let (_temporary, lock_paths) = fixture_paths();
    write_target(&lock_paths, "integer", "", "/value");
    fs::write(
        lock_paths.target_dir().join("source.json"),
        r#"{"value":1}"#,
    )
    .expect("source");
    fs::write(
        lock_paths.watch_root().join(".ffhn-locks"),
        "not a directory",
    )
    .expect("lock parent blocker");
    assert!(run_once(&lock_paths).is_err());
    let lock_status = status(&lock_paths).expect("lock failure status");
    assert_eq!(lock_status.kind(), StatusKind::InvalidState);
    assert_eq!(lock_status.lifecycle(), None);
    assert_eq!(
        lock_status.error_detail().map(DiagnosticDetail::operation),
        Some(DiagnosticOperation::LockAcquire)
    );
    assert!(reset(&lock_paths).is_err());

    let temporary = tempdir().expect("temporary directory");
    let parent_file = temporary.path().join("parent-file");
    fs::write(&parent_file, "not a directory").expect("parent file");
    assert!(blind_remove_storage_root(&parent_file.join("child")).is_err());
}

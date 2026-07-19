use super::support::*;

#[test]
fn predecessor_schemas_are_rejected_without_migration_and_reset_remains_blind() {
    let (_temporary, target_paths) = fixture_paths();
    write_target(&target_paths, "integer", "", "/value");
    let legacy_target = fs::read_to_string(target_paths.target_file())
        .expect("target")
        .replace("schema_version = 12", "schema_version = 9");
    fs::write(target_paths.target_file(), legacy_target).expect("legacy target");
    assert_eq!(
        run_once(&target_paths)
            .expect("legacy target report")
            .outcome(),
        RunOutcome::ConfigInvalid
    );
    assert!(!target_paths.storage_root().exists());

    let (_temporary, state_paths) = fixture_paths();
    write_target(&state_paths, "integer", "", "/value");
    fs::write(
        state_paths.target_dir().join("source.json"),
        r#"{"value":7}"#,
    )
    .expect("source");
    run_once(&state_paths).expect("current state");
    let mut legacy_state: serde_json::Value =
        serde_json::from_slice(&fs::read(state_paths.state_file()).expect("state"))
            .expect("state JSON");
    legacy_state["schema_version"] = serde_json::json!(6);
    fs::write(
        state_paths.state_file(),
        serde_json::to_vec(&legacy_state).expect("legacy state JSON"),
    )
    .expect("legacy state");
    assert_eq!(
        run_once(&state_paths)
            .expect("legacy state report")
            .outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&state_paths).expect("legacy state status").kind(),
        StatusKind::InvalidState
    );
    assert_eq!(
        serde_json::to_value(reset(&state_paths).expect("blind reset")).expect("reset JSON")["storage_cleared"],
        true
    );
    assert!(!state_paths.storage_root().exists());
}

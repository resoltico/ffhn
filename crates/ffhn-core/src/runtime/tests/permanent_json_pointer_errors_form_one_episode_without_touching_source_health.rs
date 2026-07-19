use super::support::*;

#[test]
fn permanent_json_pointer_errors_form_one_episode_without_touching_source_health() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "not-a-json-pointer");

    let first = run_once(&paths).expect("first permanent error");
    assert_eq!(first.outcome(), RunOutcome::ConfigInvalid);
    assert!(first.state_persisted());
    let first_state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("first state"))
            .expect("first state JSON");
    assert_eq!(
        first_state["permanent_error_episode"]["error_code"],
        "invalid_json_pointer"
    );
    assert_eq!(first_state["source_health"]["state"], "healthy");

    let second = run_once(&paths).expect("repeated permanent error");
    assert_eq!(second.outcome(), RunOutcome::ConfigInvalid);
    let second_state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("second state"))
            .expect("second state JSON");
    assert_eq!(
        second_state["permanent_error_episode"]["first_seen_at"],
        first_state["permanent_error_episode"]["first_seen_at"]
    );
    assert_eq!(second_state["source_health"]["state"], "healthy");
    assert!(second_state["accepted_observation"].is_null());
}

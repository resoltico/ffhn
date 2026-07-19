use super::support::*;

#[test]
fn persisted_parse_diagnostics_are_refused_without_state_mutation() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":7}"#).expect("source");
    run_once(&paths).expect("initial state");

    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("state")).expect("state JSON");
    state["accepted_observation"]["parse_diagnostics"] = serde_json::json!(["invented diagnostic"]);
    fs::write(
        paths.state_file(),
        serde_json::to_vec(&state).expect("state JSON"),
    )
    .expect("write malformed state");

    assert_eq!(
        run_once(&paths).expect("invalid state report").outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&paths).expect("invalid state status").kind(),
        StatusKind::InvalidState
    );
    assert_eq!(
        fs::read(paths.state_file()).expect("state remains untouched"),
        serde_json::to_vec(&state).expect("state JSON")
    );
}

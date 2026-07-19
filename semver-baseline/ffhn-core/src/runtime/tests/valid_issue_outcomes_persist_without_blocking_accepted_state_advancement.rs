use super::support::*;

#[test]
fn valid_issue_outcomes_persist_without_blocking_accepted_state_advancement() {
    let (_temporary, paths) = fixture_paths();
    let conditions = "[[conditions]]\ncondition_id = \"steady\"\n\n[conditions.predicate]\nkind = \"gt\"\nthreshold = \"-1\"\n\n[[conditions]]\ncondition_id = \"zero-reference\"\n\n[conditions.predicate]\nkind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"1\"";
    write_target_with_conditions(&paths, "integer", "", "/value", conditions);
    let source = paths.target_dir().join("source.json");

    fs::write(&source, r#"{"value":0}"#).expect("write zero baseline");
    assert_eq!(
        run_once(&paths).expect("first valid run").outcome(),
        RunOutcome::Initialized
    );
    let first: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("first state"))
            .expect("first state JSON");
    assert_eq!(first["observation_seq"], 1);
    assert_eq!(
        first["condition_state"]["zero-reference"]["result"],
        "unavailable"
    );
    assert_eq!(
        first["condition_state"]["steady"]["last_transition_value"]["canonical_value"],
        "0"
    );

    fs::write(&source, r#"{"value":1}"#).expect("write zero-reference value");
    assert_eq!(
        run_once(&paths).expect("zero-reference run").outcome(),
        RunOutcome::Changed
    );
    let second: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("second state"))
            .expect("second state JSON");
    assert_eq!(second["observation_seq"], 2);
    assert_eq!(second["accepted_observation"]["canonical_value"], "1");
    assert_eq!(
        second["condition_state"]["zero-reference"]["result"],
        "zero_reference"
    );
    assert_eq!(
        second["condition_state"]["zero-reference"]["last_transition_value"]["canonical_value"],
        "1"
    );
    assert_eq!(
        second["condition_state"]["steady"]["last_transition_value"],
        first["condition_state"]["steady"]["last_transition_value"]
    );
}

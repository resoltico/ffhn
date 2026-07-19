use super::support::*;

#[test]
fn valid_overflow_outcomes_persist_without_blocking_accepted_state_advancement() {
    let (_temporary, paths) = fixture_paths();
    let conditions = "[[conditions]]\ncondition_id = \"overflow\"\n\n[conditions.predicate]\nkind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"1\"";
    write_target_with_conditions(&paths, "integer", "", "/value", conditions);
    let source = paths.target_dir().join("source.json");

    fs::write(&source, format!(r#"{{"value":{}}}"#, i128::MIN)).expect("write minimum");
    assert_eq!(
        run_once(&paths).expect("first valid run").outcome(),
        RunOutcome::Initialized
    );
    fs::write(&source, format!(r#"{{"value":{}}}"#, i128::MAX)).expect("write maximum");
    assert_eq!(
        run_once(&paths).expect("overflow run").outcome(),
        RunOutcome::Changed
    );

    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("state")).expect("state JSON");
    assert_eq!(state["observation_seq"], 2);
    assert_eq!(
        state["accepted_observation"]["canonical_value"],
        i128::MAX.to_string()
    );
    assert_eq!(
        state["condition_state"]["overflow"]["result"],
        "arithmetic_overflow"
    );
}

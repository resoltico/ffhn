use super::support::*;

#[test]
fn named_conditions_keep_last_transition_references_independent() {
    let (_temporary, paths) = fixture_paths();
    let conditions = "[[conditions]]\ncondition_id = \"alpha\"\n\n[conditions.predicate]\nkind = \"gt\"\nthreshold = \"0\"\n\n[[conditions]]\ncondition_id = \"beta\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_condition_transition\"";
    write_target_with_conditions(&paths, "integer", "", "/value", conditions);
    let source = paths.target_dir().join("source.json");

    fs::write(&source, r#"{"value":10}"#).expect("write first source");
    run_once(&paths).expect("first run");
    fs::write(&source, r#"{"value":20}"#).expect("write second source");
    run_once(&paths).expect("second run");
    fs::write(&source, r#"{"value":20}"#).expect("write third source");
    run_once(&paths).expect("third run");

    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("state")).expect("state JSON");
    assert_eq!(state["condition_state"]["alpha"]["result"], "satisfied");
    assert_eq!(
        state["condition_state"]["alpha"]["last_transition_value"]["canonical_value"],
        "10"
    );
    assert_eq!(state["condition_state"]["beta"]["result"], "not_satisfied");
    assert_eq!(
        state["condition_state"]["beta"]["last_transition_value"]["canonical_value"],
        "20"
    );
}

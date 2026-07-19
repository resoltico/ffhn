use super::support::*;

#[test]
fn live_runtime_persists_level_hysteresis_between_runs() {
    let (_temporary, paths) = fixture_paths();
    let conditions = "[[conditions]]\ncondition_id = \"band\"\n\n[conditions.predicate]\nkind = \"band\"\nenter_threshold = \"10\"\nexit_threshold = \"8\"\ndirection = \"rising\"";
    write_target_with_conditions(&paths, "integer", "", "/value", conditions);
    let source = paths.target_dir().join("source.json");

    fs::write(&source, r#"{"value":10}"#).expect("write entering value");
    run_once(&paths).expect("enter band");
    let entered: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("entered state"))
            .expect("entered state JSON");
    assert_eq!(entered["condition_state"]["band"]["result"], "satisfied");
    assert_eq!(entered["condition_state"]["band"]["active"], true);

    fs::write(&source, r#"{"value":9}"#).expect("write retained value");
    run_once(&paths).expect("retain band");
    let retained: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("retained state"))
            .expect("retained state JSON");
    assert_eq!(retained["observation_seq"], 2);
    assert_eq!(retained["condition_state"]["band"]["result"], "satisfied");
    assert_eq!(retained["condition_state"]["band"]["active"], true);
    assert_eq!(
        retained["condition_state"]["band"]["last_transition_value"],
        entered["condition_state"]["band"]["last_transition_value"]
    );

    fs::write(&source, r#"{"value":7}"#).expect("write leaving value");
    run_once(&paths).expect("leave band");
    let left: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("left state"))
            .expect("left state JSON");
    assert_eq!(left["observation_seq"], 3);
    assert_eq!(left["condition_state"]["band"]["result"], "not_satisfied");
    assert_eq!(left["condition_state"]["band"]["active"], false);
}

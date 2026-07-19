use super::support::*;

#[test]
fn failure_branches_preserve_preexisting_measurement_and_condition_facts() {
    let (_temporary, paths) = fixture_paths();
    let conditions = "[[conditions]]\ncondition_id = \"band\"\n\n[conditions.predicate]\nkind = \"band\"\nenter_threshold = \"10\"\nexit_threshold = \"8\"\ndirection = \"rising\"";
    write_target_with_conditions(&paths, "integer", "", "/value", conditions);
    let source = paths.target_dir().join("source.json");
    fs::write(&source, r#"{"value":10}"#).expect("write baseline source");
    assert_eq!(
        run_once(&paths).expect("initialize baseline").outcome(),
        RunOutcome::Initialized
    );
    let baseline: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("baseline state"))
            .expect("baseline JSON");

    fs::write(&source, "not JSON").expect("write malformed source");
    assert_eq!(
        run_once(&paths).expect("source-suspect run").outcome(),
        RunOutcome::AcquisitionFailed
    );
    let after_source_suspect: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("source-suspect state"))
            .expect("source-suspect JSON");
    assert_eq!(
        after_source_suspect["accepted_observation"],
        baseline["accepted_observation"]
    );
    assert_eq!(
        after_source_suspect["fixed_initial_baseline"],
        baseline["fixed_initial_baseline"]
    );
    assert_eq!(
        after_source_suspect["observation_seq"],
        baseline["observation_seq"]
    );
    assert_eq!(
        after_source_suspect["condition_state"],
        baseline["condition_state"]
    );

    fs::write(&source, r#"{"value":"not-an-integer"}"#).expect("write unparseable source");
    assert_eq!(
        run_once(&paths).expect("parse-failure run").outcome(),
        RunOutcome::ValueUnparseable
    );
    let after_parse_failure: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("parse-failure state"))
            .expect("parse-failure JSON");
    assert_eq!(
        after_parse_failure["accepted_observation"],
        baseline["accepted_observation"]
    );
    assert_eq!(
        after_parse_failure["fixed_initial_baseline"],
        baseline["fixed_initial_baseline"]
    );
    assert_eq!(
        after_parse_failure["observation_seq"],
        baseline["observation_seq"]
    );
    assert_eq!(
        after_parse_failure["condition_state"],
        baseline["condition_state"]
    );

    write_target_with_conditions(&paths, "integer", "", "not-a-json-pointer", conditions);
    let invalid_digest = load_target(&paths)
        .expect("invalid-pointer target")
        .contract_digest_sha256()
        .expect("invalid-pointer digest");
    let mut permanent_input = after_parse_failure.clone();
    permanent_input["contract_digest_sha256"] = serde_json::Value::String(invalid_digest);
    fs::write(
        paths.state_file(),
        serde_json::to_vec(&permanent_input).expect("permanent input JSON"),
    )
    .expect("write matching permanent input");

    assert_eq!(
        run_once(&paths).expect("permanent-error run").outcome(),
        RunOutcome::ConfigInvalid
    );
    let after_permanent: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("permanent state"))
            .expect("permanent JSON");
    assert_eq!(
        after_permanent["accepted_observation"],
        after_parse_failure["accepted_observation"]
    );
    assert_eq!(
        after_permanent["fixed_initial_baseline"],
        after_parse_failure["fixed_initial_baseline"]
    );
    assert_eq!(
        after_permanent["observation_seq"],
        after_parse_failure["observation_seq"]
    );
    assert_eq!(
        after_permanent["condition_state"],
        after_parse_failure["condition_state"]
    );
    assert_eq!(
        after_permanent["source_health"],
        after_parse_failure["source_health"]
    );
    assert_eq!(
        after_permanent["permanent_error_episode"]["error_code"],
        "invalid_json_pointer"
    );
}

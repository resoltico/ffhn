use super::support::*;

#[test]
fn every_m3_source_failure_class_persists_its_closed_health_reason() {
    let cases = [
        (None, RunOutcome::FetchFailed, "fetch_failed"),
        (
            Some("not JSON"),
            RunOutcome::AcquisitionFailed,
            "json_malformed",
        ),
        (
            Some(r#"{"other":1}"#),
            RunOutcome::AcquisitionFailed,
            "json_missing_pointer_target",
        ),
        (
            Some(r#"{"value":[]}"#),
            RunOutcome::AcquisitionFailed,
            "json_non_scalar_pointer_target",
        ),
        (
            Some(r#"{"value":"not-an-integer"}"#),
            RunOutcome::ValueUnparseable,
            "value_unparseable",
        ),
    ];

    for (source_body, outcome, reason_class) in cases {
        let (_temporary, paths) = fixture_paths();
        write_target(&paths, "integer", "", "/value");
        if let Some(source_body) = source_body {
            fs::write(paths.target_dir().join("source.json"), source_body).expect("write source");
        }

        let report = run_once(&paths).expect("source failure report");
        assert_eq!(report.outcome(), outcome);
        assert!(report.state_persisted());
        let state: serde_json::Value =
            serde_json::from_slice(&fs::read(paths.state_file()).expect("source health state"))
                .expect("source health JSON");
        assert_eq!(state["accepted_observation"], serde_json::Value::Null);
        assert_eq!(state["fixed_initial_baseline"], serde_json::Value::Null);
        assert_eq!(state["observation_seq"], 0);
        assert_eq!(state["condition_state"], serde_json::json!({}));
        assert_eq!(state["source_health"]["state"], "suspect");
        assert_eq!(state["source_health"]["reason_class"], reason_class);
        assert_eq!(state["source_health"]["consecutive_unresolved"], 1);
    }
}

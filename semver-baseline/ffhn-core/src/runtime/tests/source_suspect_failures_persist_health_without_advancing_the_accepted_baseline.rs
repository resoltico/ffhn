use super::support::*;

#[test]
fn source_suspect_failures_persist_health_without_advancing_the_accepted_baseline() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/nested");
    fs::write(
        paths.target_dir().join("source.json"),
        r#"{"nested":{"value":1}}"#,
    )
    .expect("write source");
    assert_eq!(
        run_once(&paths).expect("structured leaf failure").outcome(),
        RunOutcome::AcquisitionFailed
    );
    let state = load_state(&paths)
        .expect("load source-health state")
        .expect("source-health state");
    assert!(state.accepted_observation().is_none());
    assert_eq!(state.observation_seq(), 0);
    let health: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("health state"))
            .expect("health JSON");
    assert_eq!(health["source_health"]["state"], "suspect");
    assert_eq!(
        health["source_health"]["reason_class"],
        "json_non_scalar_pointer_target"
    );
    assert_eq!(health["source_health"]["consecutive_unresolved"], 1);

    assert_eq!(
        run_once(&paths)
            .expect("repeated structured leaf failure")
            .outcome(),
        RunOutcome::AcquisitionFailed
    );
    let health: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("health state"))
            .expect("health JSON");
    assert_eq!(health["source_health"]["consecutive_unresolved"], 2);

    assert_eq!(
        run_once(&paths)
            .expect("escalation-boundary structured leaf failure")
            .outcome(),
        RunOutcome::AcquisitionFailed
    );
    let health: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("health state"))
            .expect("health JSON");
    assert_eq!(health["source_health"]["consecutive_unresolved"], 3);

    fs::write(
        paths.target_dir().join("source.json"),
        r#"{"nested":"not-an-integer"}"#,
    )
    .expect("write source");
    assert_eq!(
        run_once(&paths).expect("type failure").outcome(),
        RunOutcome::ValueUnparseable
    );
    let health: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("health state"))
            .expect("health JSON");
    assert_eq!(health["source_health"]["reason_class"], "value_unparseable");
    assert_eq!(health["source_health"]["consecutive_unresolved"], 1);
    assert!(health["accepted_observation"].is_null());

    fs::write(paths.target_dir().join("source.json"), r#"{"nested":7}"#)
        .expect("write valid source");
    assert_eq!(
        run_once(&paths).expect("valid recovery").outcome(),
        RunOutcome::Initialized
    );
    let recovered: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("recovered state"))
            .expect("recovered state JSON");
    assert_eq!(recovered["source_health"]["state"], "healthy");
    assert_eq!(recovered["observation_seq"], 1);
}

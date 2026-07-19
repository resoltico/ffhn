use super::support::*;

#[test]
fn contract_change_is_refused_without_state_mutation_until_blind_reset() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/price");
    fs::write(
        paths.target_dir().join("source.json"),
        r#"{"price":7,"other":9}"#,
    )
    .expect("write source");
    run_once(&paths).expect("initialize");
    let original_state = fs::read(paths.state_file()).expect("read state bytes");

    write_target(&paths, "integer", "", "/other");
    let refused = run_once(&paths).expect("refused run");
    assert_eq!(refused.outcome(), RunOutcome::RefusedContractDigest);
    assert_eq!(
        fs::read(paths.state_file()).expect("read retained state"),
        original_state
    );

    let reset_report = reset(&paths).expect("blind reset");
    let reset_json = serde_json::to_value(reset_report).expect("reset json");
    assert_eq!(reset_json["storage_cleared"], true);
    assert!(!paths.storage_root().exists());
    assert!(paths.target_file().is_file());
    assert_eq!(
        run_once(&paths).expect("fresh v2 run").outcome(),
        RunOutcome::Initialized
    );
}

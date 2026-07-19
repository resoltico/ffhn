use super::support::*;

#[test]
fn decimal_observations_preserve_evidence_and_normalize_comparison() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "decimal", "", "/price");
    fs::write(paths.target_dir().join("source.json"), r#"{"price":1.00}"#).expect("write source");

    let initialized = run_once(&paths).expect("initial run");
    assert_eq!(initialized.outcome(), RunOutcome::Initialized);
    assert_eq!(
        initialized
            .observation()
            .expect("observation")
            .raw_selected(),
        "1.00"
    );
    assert_eq!(
        initialized
            .observation()
            .expect("observation")
            .canonical_value(),
        "1"
    );

    fs::write(paths.target_dir().join("source.json"), r#"{"price":1.0}"#)
        .expect("write changed presentation");
    let unchanged = run_once(&paths).expect("second run");
    assert_eq!(unchanged.outcome(), RunOutcome::Unchanged);
    assert_eq!(
        unchanged
            .observation()
            .expect("second observation")
            .raw_selected(),
        "1.0"
    );
    assert_eq!(
        unchanged
            .observation()
            .expect("second observation")
            .canonical_value(),
        "1"
    );
    assert!(paths.state_file().is_file());
}

use super::support::*;

#[test]
fn root_json_pointer_selects_a_root_scalar_as_exact_evidence() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "decimal", "", "");
    fs::write(paths.target_dir().join("source.json"), "1.00").expect("write root scalar");

    let report = run_once(&paths).expect("root scalar run");
    assert_eq!(report.outcome(), RunOutcome::Initialized);
    assert_eq!(
        report.observation().expect("observation").raw_selected(),
        "1.00"
    );
    assert_eq!(
        report.observation().expect("observation").canonical_value(),
        "1"
    );
}

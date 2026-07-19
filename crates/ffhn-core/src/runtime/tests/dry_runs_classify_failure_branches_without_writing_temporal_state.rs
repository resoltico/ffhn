use super::support::*;

#[test]
fn dry_runs_classify_failure_branches_without_writing_temporal_state() {
    let (_temporary, source_failure_paths) = fixture_paths();
    write_target(&source_failure_paths, "integer", "", "/value");
    fs::write(
        source_failure_paths.target_dir().join("source.json"),
        "not JSON",
    )
    .expect("write malformed source");
    let source_failure =
        run_once_with_mode(&source_failure_paths, RunMode::DryRun).expect("dry source failure");
    assert_eq!(source_failure.outcome(), RunOutcome::AcquisitionFailed);
    assert!(!source_failure.state_persisted());
    assert!(!source_failure_paths.state_file().exists());

    let (_temporary, permanent_failure_paths) = fixture_paths();
    write_target(
        &permanent_failure_paths,
        "integer",
        "",
        "not-a-json-pointer",
    );
    let permanent_failure = run_once_with_mode(&permanent_failure_paths, RunMode::DryRun)
        .expect("dry permanent failure");
    assert_eq!(permanent_failure.outcome(), RunOutcome::ConfigInvalid);
    assert!(!permanent_failure.state_persisted());
    assert!(!permanent_failure_paths.state_file().exists());
}

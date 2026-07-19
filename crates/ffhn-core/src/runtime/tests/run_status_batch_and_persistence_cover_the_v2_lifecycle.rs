use super::support::*;

#[test]
fn run_status_batch_and_persistence_cover_the_v2_lifecycle() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":7}"#).expect("write source");
    assert_eq!(
        status(&paths).expect("pending status").kind(),
        StatusKind::Pending
    );
    let preview = run_once_with_mode(&paths, RunMode::DryRun).expect("dry run");
    assert_eq!(preview.outcome(), RunOutcome::Initialized);
    assert!(!preview.state_persisted());
    assert!(!paths.state_file().exists());
    assert_eq!(
        run_once(&paths).expect("initial run").outcome(),
        RunOutcome::Initialized
    );
    assert_eq!(
        status(&paths).expect("ready status").kind(),
        StatusKind::Ready
    );
    fs::write(paths.target_dir().join("source.json"), r#"{"value":8}"#).expect("change source");
    assert_eq!(
        run_once(&paths).expect("changed run").outcome(),
        RunOutcome::Changed
    );
    assert_eq!(
        run_once(&paths).expect("unchanged run").outcome(),
        RunOutcome::Unchanged
    );
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("unchanged state"))
            .expect("unchanged state JSON");
    assert_eq!(state["observation_seq"], 3);

    let second = TargetPaths::try_new(paths.watch_root(), "second").expect("second paths");
    write_target(&second, "integer", "", "/value");
    fs::write(second.target_dir().join("source.json"), r#"{"value":1}"#).expect("second source");
    let batch = run_batch_internal(vec![paths.clone(), second], RunMode::DryRun, 2).expect("batch");
    assert_eq!(batch.reports().len(), 2);
    assert!(run_batch_internal(Vec::new(), RunMode::Live, 0).is_err());

    fs::write(paths.state_file(), "not JSON").expect("invalid state");
    assert_eq!(
        run_once(&paths).expect("invalid state report").outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&paths).expect("invalid status").kind(),
        StatusKind::InvalidState
    );
    reset(&paths).expect("reset invalid state");
    fs::write(paths.target_dir().join("source.json"), "not JSON").expect("invalid JSON");
    assert_eq!(
        run_once(&paths).expect("JSON failure").outcome(),
        RunOutcome::AcquisitionFailed
    );
    fs::remove_file(paths.target_dir().join("source.json")).expect("remove source");
    assert_eq!(
        run_once(&paths).expect("missing source").outcome(),
        RunOutcome::FetchFailed
    );
}

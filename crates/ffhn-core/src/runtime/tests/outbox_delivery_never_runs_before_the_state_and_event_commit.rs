use super::support::*;

#[test]
fn outbox_delivery_never_runs_before_the_state_and_event_commit() {
    use std::os::unix::fs::PermissionsExt;

    let (_temporary, paths) = fixture_paths();
    let source = paths.target_dir().join("source.json");
    let log = paths.target_dir().join("commit-failure-deliveries.jsonl");
    let route_args = format!(
        "[\"-c\", \"cat >> \\\"$1\\\"\", \"--\", {:?}]",
        log.to_string_lossy()
    );
    let condition = "[[conditions]]\ncondition_id = \"changed\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"";
    write_delivery_target(&paths, &route_args, 4, 3, condition);
    fs::write(&source, r#"{"value":1}"#).expect("baseline source");
    run_once(&paths).expect("baseline state");
    let committed = fs::read(paths.state_file()).expect("committed state");
    fs::write(&source, r#"{"value":2}"#).expect("event source");

    let original = fs::metadata(paths.storage_root())
        .expect("storage metadata")
        .permissions();
    fs::set_permissions(paths.storage_root(), fs::Permissions::from_mode(0o500))
        .expect("make storage read-only");
    let report = run_once(&paths).expect("persist-failure report");
    fs::set_permissions(paths.storage_root(), original).expect("restore storage permissions");

    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
    assert!(!report.state_persisted());
    assert!(report.delivery_outcomes().is_empty());
    assert_eq!(
        fs::read(paths.state_file()).expect("retained state"),
        committed
    );
    assert!(!log.exists());
}

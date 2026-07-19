use super::support::*;

#[test]
fn state_io_reports_permission_failures_and_does_not_hide_persist_failure() {
    use std::os::unix::fs::PermissionsExt;

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":7}"#).expect("source");
    run_once(&paths).expect("initial committed state");
    let committed_before_failure = fs::read(paths.state_file()).expect("committed state bytes");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":8}"#).expect("changed source");
    fs::create_dir_all(paths.storage_root()).expect("storage root");
    let original_permissions = fs::metadata(paths.storage_root())
        .expect("storage metadata")
        .permissions();
    fs::set_permissions(paths.storage_root(), std::fs::Permissions::from_mode(0o500))
        .expect("make storage non-writable");
    let report = run_once(&paths).expect("persist failure report");
    fs::set_permissions(paths.storage_root(), original_permissions.clone())
        .expect("restore storage permissions");
    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
    assert_eq!(
        fs::read(paths.state_file()).expect("retained committed state"),
        committed_before_failure
    );

    fs::set_permissions(paths.storage_root(), std::fs::Permissions::from_mode(0o000))
        .expect("make storage inaccessible");
    let inaccessible = load_state(&paths);
    fs::set_permissions(paths.storage_root(), original_permissions)
        .expect("restore storage permissions");
    assert!(inaccessible.is_err());

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":7}"#).expect("source");
    let original_permissions = fs::metadata(paths.target_dir())
        .expect("target directory metadata")
        .permissions();
    fs::set_permissions(paths.target_dir(), std::fs::Permissions::from_mode(0o500))
        .expect("make target directory non-writable");
    let report = run_once(&paths).expect("storage-creation failure report");
    fs::set_permissions(paths.target_dir(), original_permissions)
        .expect("restore target directory permissions");
    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
}

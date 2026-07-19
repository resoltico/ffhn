use super::support::*;

#[test]
fn source_and_permanent_failures_preserve_persist_failure_reports() {
    fn refuse_state_writes(paths: &TargetPaths) {
        fs::create_dir_all(paths.storage_root()).expect("owned storage");
        fs::set_permissions(paths.storage_root(), fs::Permissions::from_mode(0o500))
            .expect("read-only storage");
    }

    fn restore_state_writes(paths: &TargetPaths) {
        fs::set_permissions(paths.storage_root(), fs::Permissions::from_mode(0o700))
            .expect("restore storage permissions");
    }

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    refuse_state_writes(&paths);
    let report = run_once(&paths).expect("source failure report");
    restore_state_writes(&paths);
    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
    assert!(!report.state_persisted());

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "not-an-rfc6901-pointer");
    refuse_state_writes(&paths);
    let report = run_once(&paths).expect("permanent-error report");
    restore_state_writes(&paths);
    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
    assert!(!report.state_persisted());
}

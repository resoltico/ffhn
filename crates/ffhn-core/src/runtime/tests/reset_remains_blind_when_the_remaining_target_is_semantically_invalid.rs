use super::support::*;

#[test]
fn reset_remains_blind_when_the_remaining_target_is_semantically_invalid() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    let invalid_target = fs::read_to_string(paths.target_file())
        .expect("target")
        .replacen("max_bytes = 1024", "max_bytes = 0", 1);
    fs::write(paths.target_file(), invalid_target).expect("invalid target");
    fs::create_dir_all(paths.storage_root()).expect("create storage root");
    fs::write(paths.state_file(), "malformed prior state").expect("write prior state");

    let report = reset(&paths).expect("blind reset report");

    assert!(report.storage_cleared());
    assert!(report.delivery_outcomes().is_empty());
    assert!(report.outbox_overflow().is_empty());
    assert!(report.outbox_error_detail().is_none());
    assert!(!paths.storage_root().exists());
}

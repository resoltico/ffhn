use super::support::*;

#[test]
fn reset_does_not_materialize_delivery_for_a_permanent_projection_contract_error() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "not-an-rfc-6901-pointer");
    fs::create_dir_all(paths.storage_root()).expect("create storage root");
    fs::write(paths.state_file(), "malformed prior state").expect("write prior state");

    let report = reset(&paths).expect("blind reset report");

    assert!(report.storage_cleared());
    assert!(report.delivery_outcomes().is_empty());
    assert!(report.outbox_overflow().is_empty());
    assert!(report.outbox_error_detail().is_none());
    assert!(!paths.storage_root().exists());
}

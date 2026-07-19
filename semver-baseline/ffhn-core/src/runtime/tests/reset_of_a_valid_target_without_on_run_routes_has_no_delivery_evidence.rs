use super::support::*;

#[test]
fn reset_of_a_valid_target_without_on_run_routes_has_no_delivery_evidence() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    let report = reset(&paths).expect("reset report");
    assert!(report.delivery_outcomes().is_empty());
    assert!(report.outbox_overflow().is_empty());
    assert!(report.outbox_error_detail().is_none());
}

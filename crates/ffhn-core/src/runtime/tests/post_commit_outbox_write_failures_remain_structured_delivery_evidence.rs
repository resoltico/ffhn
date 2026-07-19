use super::support::*;

#[test]
fn post_commit_outbox_write_failures_remain_structured_delivery_evidence() {
    use std::os::unix::fs::PermissionsExt;

    let (_temporary, paths) = fixture_paths();
    let source = paths.target_dir().join("source.json");
    let condition = "[[conditions]]\ncondition_id = \"changed\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"";
    let storage_root = paths.storage_root();
    let route_args = format!(
        "[\"-c\", \"cat >/dev/null; chmod 500 \\\"$1\\\"\", \"--\", {:?}]",
        storage_root.to_string_lossy()
    );
    write_delivery_target(&paths, &route_args, 4, 3, condition);
    fs::write(&source, r#"{"value":1}"#).expect("baseline source");
    run_once(&paths).expect("baseline state");
    let original = fs::metadata(&storage_root)
        .expect("storage metadata")
        .permissions();

    fs::write(&source, r#"{"value":2}"#).expect("event source");
    let report = run_once(&paths).expect("structured post-commit failure report");
    fs::set_permissions(&storage_root, original).expect("restore storage permissions");

    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
    assert!(report.state_persisted());
    assert_eq!(report.delivery_outcomes().len(), 1);
    assert_eq!(
        report.delivery_outcomes()[0].status(),
        DeliveryStatus::DeliveredUncommitted
    );
    assert!(
        report.delivery_outcomes()[0]
            .outbox_error_detail()
            .is_some()
    );
    assert!(report.outbox_error_detail().is_some());
    assert!(report.has_delivery_problem());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(paths.state_file()).expect("state"))
            .expect("state JSON")["outbox"]
            .as_array()
            .expect("outbox")
            .len(),
        1
    );

    let reset_root = paths.storage_root();
    let target = fs::read_to_string(paths.target_file()).expect("target");
    let reset_args = format!(
        "[\"-c\", \"cat >/dev/null; chmod 500 \\\"$1\\\"\", \"--\", {:?}]",
        reset_root.to_string_lossy()
    );
    fs::write(
        paths.target_file(),
        format!(
            "{target}\n[[routes]]\nroute_id = \"reset\"\nroute_family = \"on_run\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = \"/bin/sh\"\nargs = {reset_args}\ntimeout_ms = 1000\n"
        ),
    )
    .expect("add reset route");
    let reset = reset(&paths).expect("structured reset failure report");
    fs::set_permissions(&reset_root, fs::Permissions::from_mode(0o700))
        .expect("restore reset storage permissions");

    assert_eq!(reset.delivery_outcomes().len(), 1);
    assert_eq!(
        reset.delivery_outcomes()[0].status(),
        DeliveryStatus::DeliveredUncommitted
    );
    assert!(reset.outbox_error_detail().is_some());
    assert!(reset.has_delivery_problem());
}

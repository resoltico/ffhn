use super::support::*;

#[test]
fn reset_stages_and_delivers_one_durable_reset_event_after_the_blind_delete() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    let log = paths.target_dir().join("reset-deliveries.jsonl");
    let route_args = format!(
        "[\"-c\", \"cat >> \\\"$1\\\"\", \"--\", {:?}]",
        log.to_string_lossy()
    );
    let target = fs::read_to_string(paths.target_file()).expect("target");
    fs::write(
        paths.target_file(),
        format!(
            "{target}\n[[routes]]\nroute_id = \"run\"\nroute_family = \"on_run\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = \"/bin/sh\"\nargs = {route_args}\ntimeout_ms = 1000\n"
        ),
    )
    .expect("add reset route");

    let report = reset(&paths).expect("reset");
    assert_eq!(
        serde_json::to_value(&report).expect("reset JSON")["storage_cleared"],
        false
    );
    assert_eq!(report.delivery_outcomes().len(), 1);
    assert_eq!(
        report.delivery_outcomes()[0].status(),
        DeliveryStatus::Delivered
    );
    let events = fs::read_to_string(log).expect("reset event");
    let event: serde_json::Value = serde_json::from_str(events.trim()).expect("reset payload");
    assert_eq!(event["event_kind"], "reset");
    assert!(paths.state_file().is_file());
}

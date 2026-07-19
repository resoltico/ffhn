use super::support::*;

#[test]
fn lowering_max_attempts_dead_letters_an_exhausted_pending_record_without_another_process_run() {
    let (_temporary, paths) = fixture_paths();
    let source = paths.target_dir().join("source.json");
    let process_runs = paths.target_dir().join("process-runs.log");
    let condition = "[[conditions]]\ncondition_id = \"changed\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"";
    write_portable_delivery_target(&paths, "fail", None, 2, 2, condition);
    fs::write(&source, r#"{"value":1}"#).expect("baseline");
    run_once(&paths).expect("baseline");
    fs::write(&source, r#"{"value":2}"#).expect("event");
    let retry = run_once(&paths).expect("retry scheduled");
    assert_eq!(
        retry.delivery_outcomes()[0].status(),
        DeliveryStatus::RetryScheduled
    );
    assert!(!process_runs.exists());

    make_pending_outbox_due(&paths);
    write_portable_delivery_target(&paths, "fail", None, 2, 1, condition);
    let dead_letter = run_once(&paths).expect("lowered-attempt drain");
    assert_eq!(dead_letter.delivery_outcomes().len(), 1);
    assert_eq!(
        dead_letter.delivery_outcomes()[0].status(),
        DeliveryStatus::DeadLettered
    );
    assert_eq!(dead_letter.delivery_outcomes()[0].attempt_count(), 1);
    assert!(!process_runs.exists());
}

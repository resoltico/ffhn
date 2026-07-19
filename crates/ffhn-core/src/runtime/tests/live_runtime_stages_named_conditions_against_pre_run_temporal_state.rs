use super::support::*;

#[test]
fn live_runtime_stages_named_conditions_against_pre_run_temporal_state() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create target directory");
    let source = paths.target_dir().join("source.json");
    let source_path = format!("{:?}", source.to_string_lossy());
    fs::write(
        paths.target_file(),
        format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\n\n[[conditions]]\ncondition_id = \"changed\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"\n\n[target]\nkind = \"file\"\nfile_path = {source_path}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
        ),
    )
    .expect("write target");
    fs::write(&source, r#"{"value":10}"#).expect("write first source");

    assert_eq!(
        run_once(&paths).expect("first valid run").outcome(),
        RunOutcome::Initialized
    );
    let first: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("first state"))
            .expect("first state JSON");
    assert_eq!(first["observation_seq"], 1);
    assert_eq!(first["condition_state"]["changed"]["result"], "unavailable");
    assert_eq!(
        first["condition_state"]["changed"]["last_transition_value"]["canonical_value"],
        "10"
    );

    fs::write(&source, r#"{"value":20}"#).expect("write second source");
    assert_eq!(
        run_once(&paths).expect("second valid run").outcome(),
        RunOutcome::Changed
    );
    let second: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("second state"))
            .expect("second state JSON");
    assert_eq!(second["observation_seq"], 2);
    assert_eq!(second["condition_state"]["changed"]["result"], "satisfied");
    assert_eq!(
        second["condition_state"]["changed"]["last_transition_value"]["canonical_value"],
        "20"
    );
    assert_eq!(second["source_health"]["state"], "healthy");
}

#[test]
fn dry_run_without_routes_reports_named_policy_decisions_and_reference_evidence() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create target directory");
    let source = paths.target_dir().join("source.json");
    let source_path = format!("{:?}", source.to_string_lossy());
    fs::write(
        paths.target_file(),
        format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\n\n[[conditions]]\ncondition_id = \"price-rise\"\n\n[conditions.predicate]\nkind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"5\"\n\n[target]\nkind = \"file\"\nfile_path = {source_path}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
        ),
    )
    .expect("write target");
    fs::write(&source, r#"{"value":110}"#).expect("write initial source");
    assert_eq!(
        run_once(&paths).expect("initialize").outcome(),
        RunOutcome::Initialized
    );
    let persisted_before = fs::read(paths.state_file()).expect("initialized state");

    fs::write(&source, r#"{"value":200}"#).expect("write changed source");
    let report = run_once_with_mode(&paths, RunMode::DryRun).expect("dry-run report");
    assert_eq!(report.outcome(), RunOutcome::Changed);
    assert!(!report.state_persisted());
    assert!(report.delivery_outcomes().is_empty());
    let policy = report.policy_evaluation();
    assert!(policy.is_evaluated());
    let results = policy.condition_results().expect("evaluated conditions");
    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert_eq!(result.condition_id(), "price-rise");
    assert_eq!(result.outcome(), ConditionOutcome::Satisfied);
    assert!(result.triggered());
    assert!(!result.active_before());
    assert!(!result.active_after());
    let reference = result.reference().expect("reference evidence");
    assert_eq!(
        reference.reference(),
        ConditionReference::LastAcceptedObservation
    );
    assert_eq!(reference.canonical_value(), Some("110"));
    assert_eq!(policy.event_eligibilities().len(), 1);
    assert_eq!(
        policy.event_eligibilities()[0].event_kind(),
        DeliveryEventKind::ConditionSatisfied
    );
    assert_eq!(
        fs::read(paths.state_file()).expect("dry-run left state untouched"),
        persisted_before
    );
}

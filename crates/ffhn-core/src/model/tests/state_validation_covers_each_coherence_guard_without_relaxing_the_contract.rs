use super::support::*;

#[test]
fn state_validation_covers_each_coherence_guard_without_relaxing_the_contract() {
    let plain_target = target("integer", "");
    let empty = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        plain_target.contract_digest_sha256().expect("digest"),
    );

    let missing_baseline = mutate_state(&empty, |wire| {
        wire["condition_state"] = serde_json::json!({
            "condition": {"result": "not_satisfied", "active": false},
        });
    });
    assert!(missing_baseline.validate().is_err());

    let observation = plain_target
        .parse_json_scalar_token("10".to_owned())
        .expect("observation");
    let zero_sequence = mutate_state(&empty, |wire| {
        wire["accepted_observation"] =
            serde_json::to_value(&observation).expect("observation JSON");
        wire["fixed_initial_baseline"] = serde_json::to_value(&observation).expect("baseline JSON");
    });
    assert!(zero_sequence.validate().is_err());

    let conditioned = mutate_target(&plain_target, |wire| {
        wire["conditions"] = serde_json::json!([{
            "condition_id": "condition",
            "predicate": {"kind": "lt", "threshold": "20"},
        }]);
    });
    conditioned.validate().expect("conditioned target");
    let observation = conditioned
        .parse_json_scalar_token("10".to_owned())
        .expect("conditioned observation");
    let mut state = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        conditioned.contract_digest_sha256().expect("digest"),
    );
    let staged = conditioned
        .stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: &observation,
            },
            &state.condition_contexts(&conditioned),
        )
        .expect("policy stage");
    state
        .apply_valid_observation(
            &conditioned,
            observation,
            staged.condition_evaluations().expect("evaluations"),
            "2026-07-15T00:00:00Z",
        )
        .expect("state transition");

    let no_transition_value = mutate_state(&state, |wire| {
        wire["condition_state"]["condition"]["last_transition_at"] = serde_json::Value::Null;
        wire["condition_state"]["condition"]["last_transition_value"] = serde_json::Value::Null;
    });
    no_transition_value
        .validate()
        .expect("coherent no-transition state");
    no_transition_value
        .validate_for_target(&conditioned)
        .expect("target accepts no transition value");

    let missing_condition = mutate_state(&state, |wire| {
        wire["condition_state"] = serde_json::json!({});
    });
    assert!(missing_condition.validate_for_target(&conditioned).is_err());

    let wrong_condition = mutate_state(&state, |wire| {
        wire["condition_state"] = serde_json::json!({
            "other": wire["condition_state"]["condition"].clone(),
        });
    });
    assert!(wrong_condition.validate_for_target(&conditioned).is_err());

    for source_health in [
        serde_json::json!({"state": "healthy", "consecutive_unresolved": 1}),
        serde_json::json!({
            "state": "healthy",
            "consecutive_unresolved": 0,
            "first_unresolved_at": "2026-07-15T00:00:00Z",
        }),
        serde_json::json!({
            "state": "healthy",
            "consecutive_unresolved": 0,
            "last_details": {"kind": "io", "operation": "file_read", "message": "failed", "io_error_class": "not_found"},
        }),
    ] {
        let invalid_health = mutate_state(&empty, |wire| {
            wire["source_health"] = source_health;
        });
        assert!(invalid_health.validate().is_err());
    }
}

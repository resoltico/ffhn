use super::*;
use crate::graph::SourceState;
const INTEGER_MEASUREMENT: &str = r#"
schema_name = "ffhn.measurement"
schema_version = 1
measurement_id = "price"
display_name = "Price"
enabled = true
escalate_after = 2
declared_type = "integer"

[[conditions]]
condition_id = "below"
[conditions.predicate]
kind = "lt"
threshold = "10"

[projection]
kind = "json_pointer"
pointer = "/price"
"#;

fn measurement() -> MeasurementDocument {
    toml::from_str(INTEGER_MEASUREMENT).expect("measurement")
}
fn observation(value: &str) -> Observation {
    crate::model::parse_json_scalar_token_for_contract(
        crate::DeclaredType::Integer,
        &crate::TypeParams::default(),
        value.to_owned(),
    )
    .expect("observation")
}
#[test]
fn state_envelopes_reject_retired_schemas_and_nonpositive_generation() {
    let source = SourceInstanceId::mint();
    let mut source_wire =
        serde_json::to_value(SourceState::fresh(source.clone())).expect("source state wire");
    source_wire["generation"] = serde_json::json!(0);
    assert!(serde_json::from_value::<SourceState>(source_wire).is_err());

    let mut measurement_wire = serde_json::to_value(MeasurementState::fresh(
        source,
        MeasurementInstanceId::mint(),
    ))
    .expect("measurement state wire");
    measurement_wire["schema_version"] = serde_json::json!(2);
    assert!(serde_json::from_value::<MeasurementState>(measurement_wire).is_err());

    let state = SourceState::fresh(SourceInstanceId::mint());
    assert_eq!(state.next_generation().expect("successor").generation(), 2);
}

#[test]
fn accepted_observation_and_measurement_value_digest_are_one_persisted_fact() {
    let measurement = measurement();
    let mut accepted =
        MeasurementState::fresh(SourceInstanceId::mint(), MeasurementInstanceId::mint());
    accepted
        .apply_accepted_observation(&measurement, observation("1"), "a".repeat(64))
        .expect("accepted observation");
    let mut missing_digest = serde_json::to_value(accepted).expect("accepted state wire");
    missing_digest
        .as_object_mut()
        .expect("state object")
        .remove("measurement_value_digest");
    assert!(serde_json::from_value::<MeasurementState>(missing_digest).is_err());

    let mut invented_digest = serde_json::to_value(MeasurementState::fresh(
        SourceInstanceId::mint(),
        MeasurementInstanceId::mint(),
    ))
    .expect("fresh state wire");
    invented_digest["measurement_value_digest"] = serde_json::json!("a".repeat(64));
    assert!(serde_json::from_value::<MeasurementState>(invented_digest).is_err());

    let mut sequence_without_value = serde_json::to_value(MeasurementState::fresh(
        SourceInstanceId::mint(),
        MeasurementInstanceId::mint(),
    ))
    .expect("fresh state wire");
    sequence_without_value["observation_seq"] = serde_json::json!(1);
    assert!(serde_json::from_value::<MeasurementState>(sequence_without_value).is_err());

    let mut zero_with_value = serde_json::to_value({
        let mut state =
            MeasurementState::fresh(SourceInstanceId::mint(), MeasurementInstanceId::mint());
        state
            .apply_accepted_observation(&measurement, observation("1"), "a".repeat(64))
            .expect("accepted");
        state
    })
    .expect("accepted state wire");
    zero_with_value["observation_seq"] = serde_json::json!(0);
    assert!(serde_json::from_value::<MeasurementState>(zero_with_value).is_err());
}

#[test]
fn condition_policy_rebases_without_changing_observation_lineage_or_emitting_a_decision() {
    let measurement = measurement();
    let source = SourceInstanceId::mint();
    let mut state = MeasurementState::fresh(source, MeasurementInstanceId::mint());
    let mvd = "a".repeat(64);
    let first = state
        .apply_accepted_observation(&measurement, observation("5"), mvd.clone())
        .expect("first observation");
    assert_eq!(state.observation_seq(), 1);
    assert_eq!(
        state
            .accepted_observation()
            .expect("accepted")
            .canonical_value(),
        "5"
    );
    assert!(first[0].trigger());
    let revision_after_initial_definition = state.policy_revision();

    let tightened: MeasurementDocument =
        toml::from_str(&INTEGER_MEASUREMENT.replace("threshold = \"10\"", "threshold = \"4\""))
            .expect("tightened measurement");
    state.rebase_policy(&tightened).expect("policy rebase");
    assert_eq!(state.observation_seq(), 1);
    assert_eq!(
        state
            .accepted_observation()
            .expect("accepted")
            .canonical_value(),
        "5"
    );
    assert_eq!(
        state.policy_revision(),
        revision_after_initial_definition + 1
    );

    let after = state
        .apply_accepted_observation(&tightened, observation("4"), mvd)
        .expect("observation after rebase");
    assert_eq!(after[0].outcome(), ConditionOutcome::NotSatisfied);
    assert!(!after[0].trigger());
    assert_eq!(state.observation_seq(), 2);
}

#[test]
fn value_digest_refusal_cannot_be_bypassed_by_a_policy_rebase() {
    let measurement = measurement();
    let mut state =
        MeasurementState::fresh(SourceInstanceId::mint(), MeasurementInstanceId::mint());
    state
        .apply_accepted_observation(&measurement, observation("5"), "a".repeat(64))
        .expect("first observation");
    let changed_policy: MeasurementDocument =
        toml::from_str(&INTEGER_MEASUREMENT.replace("threshold = \"10\"", "threshold = \"4\""))
            .expect("changed policy");
    let revision = state.policy_revision();
    assert!(
        state
            .apply_accepted_observation(&changed_policy, observation("6"), "b".repeat(64))
            .is_err()
    );
    assert_eq!(state.observation_seq(), 1);
    assert_eq!(state.policy_revision(), revision);
}

#[test]
fn unchanged_condition_definition_retains_distinct_temporal_evidence() {
    let measurement = measurement();
    let mut state =
        MeasurementState::fresh(SourceInstanceId::mint(), MeasurementInstanceId::mint());
    state
        .apply_accepted_observation(&measurement, observation("5"), "a".repeat(64))
        .expect("first observation");
    state
        .apply_accepted_observation(&measurement, observation("6"), "a".repeat(64))
        .expect("second observation");
    let before = serde_json::to_value(&state).expect("state wire");
    let revision = state.policy_revision();

    state.rebase_policy(&measurement).expect("unchanged rebase");

    let after = serde_json::to_value(&state).expect("rebased state wire");
    assert_eq!(state.policy_revision(), revision);
    assert_eq!(after["condition_state"], before["condition_state"]);
    assert_ne!(
        after["accepted_observation"]["canonical_value"],
        after["condition_state"]["below"]["last_condition_transition"]["canonical_value"]
    );
}

#[test]
fn measurement_state_accessors_failures_faults_and_persisted_guards_are_complete() {
    let source = SourceInstanceId::mint();
    let instance = MeasurementInstanceId::mint();
    let mut state = MeasurementState::fresh(source.clone(), instance.clone());
    assert_eq!(state.source_instance_id(), &source);
    assert_eq!(state.measurement_instance_id(), &instance);
    assert_eq!(state.observation_seq(), 0);
    assert_eq!(state.measurement_episode_seq(), 0);
    assert_eq!(state.policy_revision(), 0);
    assert!(state.accepted_observation().is_none());
    assert!(state.measurement_value_digest().is_none());
    assert_eq!(
        state.extraction_health(),
        &MeasurementExtractionHealth::healthy()
    );
    assert!(state.integration_fault_episode().is_none());
    assert!(state.outbox_overflow().is_empty());
    state
        .set_outbox_overflow(Vec::new())
        .expect("empty overflow");

    assert!(
        !state
            .apply_extraction_failure(
                ExtractionFailureReason::JsonMalformed,
                "2026-08-25T00:00:00Z",
                2,
            )
            .expect("first failure")
    );
    assert_eq!(state.measurement_episode_seq(), 1);
    assert!(
        state
            .apply_extraction_failure(
                ExtractionFailureReason::JsonMalformed,
                "2026-08-25T00:01:00Z",
                2,
            )
            .expect("same episode")
    );
    assert_eq!(state.measurement_episode_seq(), 1);
    assert!(
        state
            .apply_measurement_integration_fault(
                GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation,
                "2026-08-25T00:02:00Z".to_owned(),
            )
            .expect("fault")
    );
    assert!(
        !state
            .apply_measurement_integration_fault(
                GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation,
                "2026-08-25T00:03:00Z".to_owned(),
            )
            .expect("same fault")
    );
    assert_eq!(
        state
            .integration_fault_episode()
            .expect("active integration fault")
            .code(),
        GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation
    );

    let measurement = measurement();
    assert!(
        state
            .apply_accepted_observation(&measurement, observation("1"), "invalid".to_owned())
            .is_err()
    );
    let decimal = crate::model::parse_json_scalar_token_for_contract(
        crate::DeclaredType::Decimal,
        &crate::TypeParams::default(),
        "1.0".to_owned(),
    )
    .expect("decimal observation");
    assert!(
        state
            .apply_accepted_observation(&measurement, decimal, "a".repeat(64))
            .is_err()
    );

    let mut schema = serde_json::to_value(&state).expect("state wire");
    schema["schema_name"] = serde_json::json!("foreign.measurement_state");
    assert!(serde_json::from_value::<MeasurementState>(schema).is_err());

    let mut source_fault = serde_json::to_value(&state).expect("state wire");
    source_fault["integration_fault_episode"] = serde_json::json!({
        "code": "secret_unavailable",
        "first_seen_at_utc": "2026-08-25T00:00:00Z"
    });
    assert!(serde_json::from_value::<MeasurementState>(source_fault).is_err());

    let invalid_overflow: OutboxOverflowFact = serde_json::from_value(serde_json::json!({
        "event_id": "invalid",
        "event_kind": "measurement_initialized",
        "route_id": "route",
        "route_family": "on_measurement"
    }))
    .expect("structural overflow");
    assert!(state.set_outbox_overflow(vec![invalid_overflow]).is_err());
}

#[test]
fn measurement_state_rejects_condition_and_contract_crossings_and_sequence_exhaustion() {
    let measurement = measurement();
    let mut state =
        MeasurementState::fresh(SourceInstanceId::mint(), MeasurementInstanceId::mint());
    state
        .apply_accepted_observation(&measurement, observation("5"), "a".repeat(64))
        .expect("accepted");
    let mut invalid_condition = serde_json::to_value(&state).expect("state wire");
    invalid_condition["condition_state"]["below"]["definition_digest"] =
        serde_json::json!("invalid");
    assert!(serde_json::from_value::<MeasurementState>(invalid_condition).is_err());

    let text_measurement: MeasurementDocument = toml::from_str(
        &INTEGER_MEASUREMENT
            .replace("declared_type = \"integer\"", "declared_type = \"text\"")
            .replace(
                "kind = \"lt\"\nthreshold = \"10\"",
                "kind = \"changed\"\nreference = \"last_accepted_observation\"",
            ),
    )
    .expect("text measurement");
    assert!(state.validate_for_measurement(&text_measurement).is_err());
    let revision = state.policy_revision();
    state.rebase_policy(&measurement).expect("unchanged rebase");
    assert_eq!(state.policy_revision(), revision);

    for field in ["fixed_initial_baseline", "last_condition_transition"] {
        let mut crossed = serde_json::to_value(&state).expect("state wire");
        crossed["condition_state"]["below"][field]["declared_type"] = serde_json::json!("text");
        crossed["condition_state"]["below"][field]["raw_selected"] = serde_json::json!("\"text\"");
        crossed["condition_state"]["below"][field]["comparison_projection"] =
            serde_json::json!("\"text\"");
        crossed["condition_state"]["below"][field]["canonical_value"] = serde_json::json!("text");
        let crossed: MeasurementState = serde_json::from_value(crossed).expect("crossed state");
        assert!(
            crossed.validate_for_measurement(&measurement).is_err(),
            "{field}"
        );
    }

    let mut max_observation = serde_json::to_value(&state).expect("state wire");
    max_observation["observation_seq"] = serde_json::json!(u64::MAX);
    let mut max_observation: MeasurementState =
        serde_json::from_value(max_observation).expect("max sequence state");
    assert!(
        max_observation
            .apply_accepted_observation(&measurement, observation("6"), "a".repeat(64))
            .is_err()
    );

    let mut fresh =
        MeasurementState::fresh(SourceInstanceId::mint(), MeasurementInstanceId::mint());
    let mut max_episode = serde_json::to_value(&fresh).expect("fresh wire");
    max_episode["measurement_episode_seq"] = serde_json::json!(u64::MAX);
    fresh = serde_json::from_value(max_episode).expect("max episode state");
    assert!(
        fresh
            .apply_extraction_failure(
                ExtractionFailureReason::JsonMalformed,
                "2026-08-25T00:00:00Z",
                1,
            )
            .is_err()
    );

    let id = ConditionId::new("missing").expect("condition id");
    assert!(required_definition_digest(&BTreeMap::new(), &id).is_err());
    assert!(required_condition_state(&BTreeMap::new(), &id).is_err());
    assert!(required_condition_state_mut(&mut BTreeMap::new(), &id).is_err());
    assert!(
        require_exact_evaluation_ids(&BTreeSet::from([id.clone()]), &BTreeSet::new(),).is_err()
    );
    assert!(
        require_exact_evaluation_ids(&BTreeSet::from([id.clone()]), &BTreeSet::from([id]),).is_ok()
    );
}

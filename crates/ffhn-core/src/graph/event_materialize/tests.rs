use super::*;
use crate::graph::{MeasurementInstanceId, SourceInstanceId};

fn measurement() -> MeasurementDocument {
    toml::from_str(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"price\"\ndisplay_name = \"Price\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\n[[conditions]]\ncondition_id = \"changed\"\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"\n[projection]\nkind = \"json_pointer\"\npointer = \"/price\"\n",
    )
    .expect("measurement")
}

fn priority_measurement() -> MeasurementDocument {
    toml::from_str(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"price\"\ndisplay_name = \"Price\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\n[[conditions]]\ncondition_id = \"changed\"\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"\n[[conditions]]\ncondition_id = \"issue\"\n[conditions.predicate]\nkind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"1\"\n[projection]\nkind = \"json_pointer\"\npointer = \"/price\"\n",
    )
    .expect("priority measurement")
}

fn observation(value: &str) -> crate::Observation {
    crate::model::parse_json_scalar_token_for_contract(
        crate::DeclaredType::Integer,
        &crate::TypeParams::default(),
        value.to_owned(),
    )
    .expect("observation")
}

fn measurement_event_parts<'a>(
    measurement: &'a MeasurementDocument,
    state: &'a MeasurementState,
    source_instance_id: &SourceInstanceId,
    event_kind: EventKind,
    code: Option<String>,
) -> MeasurementEventParts<'a> {
    MeasurementEventParts {
        graph_id: GraphId::mint(),
        source_id: SourceId::new("shop").expect("source"),
        source_instance_id: source_instance_id.clone(),
        measurement_id: MeasurementId::new("price").expect("measurement"),
        measurement,
        state,
        event_kind,
        code,
        measurement_value_digest: "a".repeat(64),
        committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
    }
}

#[test]
fn condition_events_are_route_independent_and_bind_the_definition_and_observation_sequence() {
    let measurement = measurement();
    let source_instance = SourceInstanceId::mint();
    let mut state = MeasurementState::fresh(source_instance.clone(), MeasurementInstanceId::mint());
    state
        .apply_accepted_observation(&measurement, observation("1"), "a".repeat(64))
        .expect("first");
    let evaluations = state
        .apply_accepted_observation(&measurement, observation("2"), "a".repeat(64))
        .expect("second");
    let events = materialize_condition_events(
        GraphId::mint(),
        SourceId::new("shop").expect("source"),
        source_instance,
        MeasurementId::new("price").expect("measurement id"),
        &measurement,
        &state,
        &evaluations,
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].event_kind(),
        super::super::EventKind::ConditionSatisfied
    );

    let not_satisfied: MeasurementDocument = toml::from_str(
        &toml::to_string(&measurement)
            .expect("measurement TOML")
            .replace(
                "kind = \"changed\"\nreference = \"last_accepted_observation\"",
                "kind = \"lt\"\nthreshold = \"0\"",
            ),
    )
    .expect("not-satisfied measurement");
    let source_instance = SourceInstanceId::mint();
    let mut state = MeasurementState::fresh(source_instance.clone(), MeasurementInstanceId::mint());
    let evaluations = state
        .apply_accepted_observation(&not_satisfied, observation("1"), "b".repeat(64))
        .expect("evaluation");
    assert!(
        materialize_condition_events(
            GraphId::mint(),
            SourceId::new("shop").expect("source"),
            source_instance,
            MeasurementId::new("price").expect("measurement"),
            &not_satisfied,
            &state,
            &evaluations,
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("events")
        .is_empty()
    );

    let satisfied: MeasurementDocument = toml::from_str(
        &toml::to_string(&not_satisfied)
            .expect("measurement TOML")
            .replace("threshold = \"0\"", "threshold = \"10\""),
    )
    .expect("satisfied measurement");
    let source_instance = SourceInstanceId::mint();
    let mut state = MeasurementState::fresh(source_instance.clone(), MeasurementInstanceId::mint());
    state
        .apply_accepted_observation(&satisfied, observation("5"), "c".repeat(64))
        .expect("first satisfied");
    let evaluations = state
        .apply_accepted_observation(&satisfied, observation("4"), "c".repeat(64))
        .expect("still satisfied");
    assert_eq!(evaluations[0].outcome(), ConditionOutcome::Satisfied);
    assert!(!evaluations[0].trigger());
    assert!(
        materialize_condition_events(
            GraphId::mint(),
            SourceId::new("shop").expect("source"),
            source_instance,
            MeasurementId::new("price").expect("measurement"),
            &satisfied,
            &state,
            &evaluations,
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("events")
        .is_empty()
    );
}

#[test]
fn condition_issues_precede_satisfactions_while_each_class_keeps_declaration_order() {
    let measurement = priority_measurement();
    let source_instance = SourceInstanceId::mint();
    let mut state = MeasurementState::fresh(source_instance.clone(), MeasurementInstanceId::mint());
    state
        .apply_accepted_observation(&measurement, observation("0"), "a".repeat(64))
        .expect("first observation");
    let evaluations = state
        .apply_accepted_observation(&measurement, observation("1"), "a".repeat(64))
        .expect("second observation");
    let events = materialize_condition_events(
        GraphId::mint(),
        SourceId::new("shop").expect("source"),
        source_instance,
        MeasurementId::new("price").expect("measurement id"),
        &measurement,
        &state,
        &evaluations,
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("events");
    assert_eq!(
        events
            .iter()
            .map(EventEnvelope::event_kind)
            .collect::<Vec<_>>(),
        vec![
            EventKind::ConditionEvaluationIssue,
            EventKind::ConditionSatisfied,
        ]
    );
}

#[test]
fn condition_materialization_rejects_missing_observations_missing_and_extra_evaluations() {
    assert!(condition_digest(&BTreeMap::new(), "missing").is_err());
    let measurement = measurement();
    let source_instance = SourceInstanceId::mint();
    let fresh = MeasurementState::fresh(source_instance.clone(), MeasurementInstanceId::mint());
    assert!(
        materialize_condition_events(
            GraphId::mint(),
            SourceId::new("shop").expect("source"),
            source_instance.clone(),
            MeasurementId::new("price").expect("measurement"),
            &measurement,
            &fresh,
            &[],
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .is_err()
    );

    let mut state = fresh;
    state
        .apply_accepted_observation(&measurement, observation("1"), "a".repeat(64))
        .expect("first");
    let evaluations = state
        .apply_accepted_observation(&measurement, observation("2"), "a".repeat(64))
        .expect("second");
    assert!(
        materialize_condition_events(
            GraphId::mint(),
            SourceId::new("shop").expect("source"),
            source_instance.clone(),
            MeasurementId::new("price").expect("measurement"),
            &measurement,
            &state,
            &[],
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .is_err()
    );

    let other: MeasurementDocument = toml::from_str(
        &toml::to_string(&measurement)
            .expect("measurement TOML")
            .replace("condition_id = \"changed\"", "condition_id = \"other\""),
    )
    .expect("other measurement");
    let mut other_state =
        MeasurementState::fresh(source_instance.clone(), MeasurementInstanceId::mint());
    other_state
        .apply_accepted_observation(&other, observation("1"), "b".repeat(64))
        .expect("first other");
    let other_evaluation = other_state
        .apply_accepted_observation(&other, observation("2"), "b".repeat(64))
        .expect("second other")
        .remove(0);
    let mut extra = evaluations;
    extra.push(other_evaluation);
    assert!(
        materialize_condition_events(
            GraphId::mint(),
            SourceId::new("shop").expect("source"),
            source_instance,
            MeasurementId::new("price").expect("measurement"),
            &measurement,
            &state,
            &extra,
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .is_err()
    );
}

#[test]
fn measurement_and_source_lifecycle_materializers_cover_every_owned_kind_and_code_guard() {
    let measurement = measurement();
    let source_instance = SourceInstanceId::mint();
    let mut state = MeasurementState::fresh(source_instance.clone(), MeasurementInstanceId::mint());
    assert_eq!(
        materialize_measurement_event(measurement_event_parts(
            &measurement,
            &state,
            &source_instance,
            EventKind::MeasurementInitialized,
            None,
        ))
        .expect("initialized")
        .event_kind(),
        EventKind::MeasurementInitialized
    );
    assert!(
        materialize_measurement_event(measurement_event_parts(
            &measurement,
            &state,
            &source_instance,
            EventKind::ExtractionEscalation,
            None,
        ))
        .is_err()
    );
    assert!(
        materialize_measurement_event(measurement_event_parts(
            &measurement,
            &state,
            &source_instance,
            EventKind::SourceInitialized,
            Some("code".to_owned()),
        ))
        .is_err()
    );
    state
        .apply_extraction_failure(
            super::super::ExtractionFailureReason::JsonMalformed,
            "2026-08-25T00:00:00Z",
            1,
        )
        .expect("extraction");
    assert_eq!(
        materialize_measurement_event(measurement_event_parts(
            &measurement,
            &state,
            &source_instance,
            EventKind::ExtractionEscalation,
            Some("json_malformed".to_owned()),
        ))
        .expect("escalation")
        .event_kind(),
        EventKind::ExtractionEscalation
    );
    state
        .apply_measurement_integration_fault(
            super::super::GraphIntegrationFaultCode::HtmlcutInternalError,
            "2026-08-25T00:01:00Z".to_owned(),
        )
        .expect("fault");
    assert_eq!(
        materialize_measurement_event(measurement_event_parts(
            &measurement,
            &state,
            &source_instance,
            EventKind::MeasurementIntegrationFault,
            Some("htmlcut_internal_error".to_owned()),
        ))
        .expect("integration")
        .event_kind(),
        EventKind::MeasurementIntegrationFault
    );

    let source: super::super::SourceDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = \"shop\"\ndisplay_name = \"Shop\"\nenabled = true\nescalate_after = 1\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 1024\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
        crate::graph::test_support::absolute_file_path("events.json"),
    ))
    .expect("source");
    let mut source_state = super::super::SourceState::fresh(source_instance.clone());
    source_state
        .apply_acquisition_failure(
            super::super::SourceFetchFailure {
                kind: super::super::SourceFetchFailureKind::FileNotFound,
                status: None,
                raw_platform_error: None,
            },
            "2026-08-25T00:00:00Z",
            1,
        )
        .expect("source failure");
    for (kind, code) in [
        (EventKind::SourceEscalation, "file_not_found"),
        (EventKind::SourceIntegrationFault, "secret_unavailable"),
    ] {
        if kind == EventKind::SourceIntegrationFault {
            source_state
                .apply_source_integration_fault(
                    super::super::GraphIntegrationFaultCode::SecretUnavailable,
                    "2026-08-25T00:01:00Z".to_owned(),
                )
                .expect("source fault");
        }
        assert_eq!(
            materialize_source_event(SourceEventParts {
                graph_id: GraphId::mint(),
                source_id: SourceId::new("shop").expect("source"),
                source_instance_id: source_instance.clone(),
                source: &source,
                state: &source_state,
                event_kind: kind,
                code: code.to_owned(),
                committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
            })
            .expect("source event")
            .event_kind(),
            kind
        );
    }
    assert!(
        materialize_source_event(SourceEventParts {
            graph_id: GraphId::mint(),
            source_id: SourceId::new("shop").expect("source"),
            source_instance_id: source_instance,
            source: &source,
            state: &source_state,
            event_kind: EventKind::SourceInitialized,
            code: "initialized".to_owned(),
            committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
        })
        .is_err()
    );
}

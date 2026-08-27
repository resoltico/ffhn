use super::*;
use crate::{
    ConditionId,
    graph::{
        GraphId, GraphIdentity, GraphRouteFamily, MeasurementId, MeasurementInstanceId, SourceId,
        SourceIdentity, SourceInstanceId,
    },
};

fn condition_key() -> (
    crate::graph::GraphId,
    crate::graph::SourceInstanceId,
    EventKey,
) {
    let graph = GraphIdentity::new("2026-08-25T00:00:00Z".to_owned()).expect("graph");
    let source = SourceIdentity::fresh();
    let key = EventKey::ConditionSatisfied {
        graph_id: graph.graph_id().clone(),
        source_id: SourceId::new("shop").expect("source id"),
        source_instance_id: source.source_instance_id().clone(),
        measurement_id: MeasurementId::new("price").expect("measurement id"),
        measurement_instance_id: MeasurementInstanceId::mint(),
        condition_id: ConditionId::new("price_changed").expect("condition id"),
        condition_defn_digest: "a".repeat(64),
        observation_seq: 3,
    };
    (
        graph.graph_id().clone(),
        source.source_instance_id().clone(),
        key,
    )
}

#[test]
fn event_identity_is_route_and_wall_clock_independent_but_lineage_nonreusing() {
    let (graph_id, source_instance_id, key) = condition_key();
    let first = EventEnvelope::new(EventEnvelopeParts {
        graph_id: graph_id.clone(),
        source_instance_id: source_instance_id.clone(),
        event_key: key.clone(),
        display_name: "Price".to_owned(),
        committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
        observation: Some(EventObservation::new("42".to_owned(), 3).expect("observation")),
        lifecycle_fact: None,
        policy_revision: Some(1),
    })
    .expect("first event");
    let later = EventEnvelope::new(EventEnvelopeParts {
        graph_id,
        source_instance_id,
        event_key: key,
        display_name: "Price renamed".to_owned(),
        committed_at_utc: "2036-08-25T00:00:00Z".to_owned(),
        observation: Some(EventObservation::new("42".to_owned(), 3).expect("observation")),
        lifecycle_fact: None,
        policy_revision: Some(999),
    })
    .expect("later event");
    assert_eq!(first.event_id(), later.event_id());

    let (_, _, replacement_key) = condition_key();
    assert_ne!(
        first.event_id(),
        replacement_key.event_id().expect("replacement identity")
    );
}

#[test]
fn envelope_rejects_any_detached_or_tampered_identity_field() {
    let (graph_id, source_instance_id, key) = condition_key();
    let envelope = EventEnvelope::new(EventEnvelopeParts {
        graph_id,
        source_instance_id,
        event_key: key,
        display_name: "Price".to_owned(),
        committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
        observation: Some(EventObservation::new("42".to_owned(), 3).expect("observation")),
        lifecycle_fact: None,
        policy_revision: None,
    })
    .expect("event");
    let mut wire = serde_json::to_value(envelope).expect("wire");
    wire["event_id"] = serde_json::json!("b".repeat(64));
    assert!(serde_json::from_value::<EventEnvelope>(wire).is_err());
}

#[test]
fn declared_codes_with_digits_are_valid_but_ambiguous_code_shapes_are_rejected() {
    assert!(super::key::require_code("source episode", "invalid_utf8").is_ok());
    for invalid in [
        "",
        "_invalid",
        "invalid_",
        "invalid__utf8",
        "InvalidUtf8",
        "invalid-utf8",
    ] {
        assert!(
            super::key::require_code("source episode", invalid).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn condition_events_preserve_valid_empty_text_values() {
    assert!(EventObservation::new(String::new(), 1).is_ok());
}

#[test]
fn event_codes_and_lifecycle_facts_are_closed_and_key_consistent() {
    let graph = GraphIdentity::new("2026-08-25T00:00:00Z".to_owned()).expect("graph");
    let source = SourceIdentity::fresh();
    let issue = EventKey::ConditionEvaluationIssue {
        graph_id: graph.graph_id().clone(),
        source_id: SourceId::new("shop").expect("source id"),
        source_instance_id: source.source_instance_id().clone(),
        measurement_id: MeasurementId::new("price").expect("measurement id"),
        measurement_instance_id: MeasurementInstanceId::mint(),
        issue_code: "invented_issue".to_owned(),
        condition_id: ConditionId::new("changed").expect("condition id"),
        condition_defn_digest: "a".repeat(64),
        observation_seq: 1,
    };
    assert!(issue.validate().is_err());

    let envelope = EventEnvelope::new(EventEnvelopeParts {
        graph_id: graph.graph_id().clone(),
        source_instance_id: source.source_instance_id().clone(),
        event_key: EventKey::SourceLifecycle {
            graph_id: graph.graph_id().clone(),
            source_id: SourceId::new("shop").expect("source id"),
            source_instance_id: source.source_instance_id().clone(),
            event_kind: EventKind::SourceInitialized,
            source_representation_digest: "b".repeat(64),
        },
        display_name: "Shop".to_owned(),
        committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
        observation: None,
        lifecycle_fact: Some("initialized".to_owned()),
        policy_revision: None,
    })
    .expect("source event");
    let mut wire = serde_json::to_value(envelope).expect("event wire");
    wire["lifecycle_fact"] = serde_json::json!("different");
    assert!(serde_json::from_value::<EventEnvelope>(wire).is_err());
}

type KeyCase = (
    EventKey,
    Option<EventObservation>,
    Option<String>,
    Option<u64>,
);

fn all_key_classes() -> Vec<KeyCase> {
    let graph = GraphId::mint();
    let source_instance = SourceInstanceId::mint();
    let source_id = SourceId::new("shop").expect("source id");
    let measurement_id = MeasurementId::new("price").expect("measurement id");
    let measurement_instance = MeasurementInstanceId::mint();
    let condition_id = ConditionId::new("changed").expect("condition id");
    vec![
        (
            EventKey::ConditionSatisfied {
                graph_id: graph.clone(),
                source_id: source_id.clone(),
                source_instance_id: source_instance.clone(),
                measurement_id: measurement_id.clone(),
                measurement_instance_id: measurement_instance.clone(),
                condition_id: condition_id.clone(),
                condition_defn_digest: "a".repeat(64),
                observation_seq: 1,
            },
            Some(EventObservation::new("42".to_owned(), 1).expect("observation")),
            None,
            Some(1),
        ),
        (
            EventKey::ConditionEvaluationIssue {
                graph_id: graph.clone(),
                source_id: source_id.clone(),
                source_instance_id: source_instance.clone(),
                measurement_id: measurement_id.clone(),
                measurement_instance_id: measurement_instance.clone(),
                issue_code: "zero_reference".to_owned(),
                condition_id,
                condition_defn_digest: "b".repeat(64),
                observation_seq: 2,
            },
            Some(EventObservation::new("0".to_owned(), 2).expect("observation")),
            None,
            Some(2),
        ),
        (
            EventKey::MeasurementLifecycle {
                graph_id: graph.clone(),
                source_id: source_id.clone(),
                source_instance_id: source_instance.clone(),
                measurement_id: measurement_id.clone(),
                measurement_instance_id: measurement_instance.clone(),
                event_kind: EventKind::MeasurementInitialized,
                measurement_value_digest: "c".repeat(64),
            },
            None,
            Some("initialized".to_owned()),
            Some(1),
        ),
        (
            EventKey::MeasurementEpisode {
                graph_id: graph.clone(),
                source_id: source_id.clone(),
                source_instance_id: source_instance.clone(),
                measurement_id: measurement_id.clone(),
                measurement_instance_id: measurement_instance.clone(),
                event_kind: EventKind::ExtractionEscalation,
                code: "json_malformed".to_owned(),
                measurement_episode_seq: 1,
                measurement_value_digest: "d".repeat(64),
            },
            None,
            Some("json_malformed".to_owned()),
            Some(1),
        ),
        (
            EventKey::MeasurementEpisode {
                graph_id: graph.clone(),
                source_id: source_id.clone(),
                source_instance_id: source_instance.clone(),
                measurement_id,
                measurement_instance_id: measurement_instance,
                event_kind: EventKind::MeasurementIntegrationFault,
                code: "ffhn_policy_invariant_violation".to_owned(),
                measurement_episode_seq: 2,
                measurement_value_digest: "e".repeat(64),
            },
            None,
            Some("ffhn_policy_invariant_violation".to_owned()),
            Some(2),
        ),
        (
            EventKey::SourceLifecycle {
                graph_id: graph.clone(),
                source_id: source_id.clone(),
                source_instance_id: source_instance.clone(),
                event_kind: EventKind::SourceInitialized,
                source_representation_digest: "f".repeat(64),
            },
            None,
            Some("initialized".to_owned()),
            None,
        ),
        (
            EventKey::SourceEpisode {
                graph_id: graph.clone(),
                source_id: source_id.clone(),
                source_instance_id: source_instance.clone(),
                event_kind: EventKind::SourceEscalation,
                code: "http_status".to_owned(),
                source_episode_seq: 1,
                source_representation_digest: "1".repeat(64),
            },
            None,
            Some("http_status".to_owned()),
            None,
        ),
        (
            EventKey::SourceEpisode {
                graph_id: graph,
                source_id,
                source_instance_id: source_instance,
                event_kind: EventKind::SourceIntegrationFault,
                code: "secret_unavailable".to_owned(),
                source_episode_seq: 2,
                source_representation_digest: "2".repeat(64),
            },
            None,
            Some("secret_unavailable".to_owned()),
            None,
        ),
    ]
}

#[test]
fn every_event_key_and_envelope_class_exposes_complete_scope_and_evidence() {
    for (key, observation, lifecycle_fact, policy_revision) in all_key_classes() {
        key.validate().expect("valid key");
        assert_eq!(key.event_id().expect("event id").len(), 64);
        assert_eq!(key.source_id().as_str(), "shop");
        key.graph_id().validate().expect("graph id");
        key.source_instance_id()
            .validate()
            .expect("source instance");
        if let Some((measurement_id, instance)) = key.measurement_lineage() {
            assert_eq!(measurement_id.as_str(), "price");
            instance.validate().expect("measurement instance");
        }
        let kind = key.event_kind();
        let envelope = EventEnvelope::new(EventEnvelopeParts {
            graph_id: key.graph_id().clone(),
            source_instance_id: key.source_instance_id().clone(),
            event_key: key,
            display_name: "Display".to_owned(),
            committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
            observation,
            lifecycle_fact,
            policy_revision,
        })
        .expect("envelope");
        assert_eq!(envelope.event_kind(), kind);
        assert_eq!(envelope.source_id().as_str(), "shop");
        assert_eq!(
            envelope.source_instance_id(),
            envelope.event_key().source_instance_id()
        );
        assert_eq!(
            envelope.measurement_lineage().is_some(),
            kind.route_family() != GraphRouteFamily::OnSource
        );
        assert_eq!(
            envelope.condition_id().is_some(),
            matches!(
                kind,
                EventKind::ConditionSatisfied | EventKind::ConditionEvaluationIssue
            )
        );
        envelope.validate().expect("valid envelope");
    }
    assert_eq!(
        EventKind::ConditionSatisfied.route_family(),
        GraphRouteFamily::OnCondition
    );
    for kind in [
        EventKind::ConditionEvaluationIssue,
        EventKind::MeasurementInitialized,
        EventKind::ExtractionEscalation,
        EventKind::MeasurementIntegrationFault,
    ] {
        assert_eq!(kind.route_family(), GraphRouteFamily::OnMeasurement);
    }
    for kind in [
        EventKind::SourceInitialized,
        EventKind::SourceEscalation,
        EventKind::SourceIntegrationFault,
    ] {
        assert_eq!(kind.route_family(), GraphRouteFamily::OnSource);
    }
    assert!(EventObservation::new("value".to_owned(), 0).is_err());
}

#[test]
fn event_keys_and_envelopes_reject_every_crossed_scope_and_identity_family() {
    let (key, observation, _, policy_revision) = all_key_classes().remove(0);
    let envelope = EventEnvelope::new(EventEnvelopeParts {
        graph_id: key.graph_id().clone(),
        source_instance_id: key.source_instance_id().clone(),
        event_key: key,
        display_name: "Display".to_owned(),
        committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
        observation,
        lifecycle_fact: None,
        policy_revision,
    })
    .expect("condition envelope");
    let base = serde_json::to_value(&envelope).expect("envelope wire");
    for (pointer, value) in [
        ("/schema_name", serde_json::json!("foreign.event")),
        ("/schema_version", serde_json::json!(2)),
        ("/display_name", serde_json::json!(" ")),
        ("/committed_at_utc", serde_json::json!("not-a-time")),
        ("/event_kind", serde_json::json!("source_initialized")),
        ("/event_id", serde_json::json!("0".repeat(64))),
        (
            "/graph_id",
            serde_json::to_value(GraphId::mint()).expect("graph id"),
        ),
        (
            "/source_instance_id",
            serde_json::to_value(SourceInstanceId::mint()).expect("source instance"),
        ),
        ("/observation/seq", serde_json::json!(2)),
        ("/lifecycle_fact", serde_json::json!("invented")),
        ("/emitter/kind", serde_json::json!("source")),
        ("/emitter/source_id", serde_json::json!("other")),
    ] {
        let mut wire = base.clone();
        if pointer == "/lifecycle_fact" {
            wire["lifecycle_fact"] = value;
        } else {
            *wire.pointer_mut(pointer).expect("pointer") = value;
        }
        assert!(
            serde_json::from_value::<EventEnvelope>(wire).is_err(),
            "{pointer}"
        );
    }

    let (source_key, _, source_fact, _) = all_key_classes().remove(5);
    let source = EventEnvelope::new(EventEnvelopeParts {
        graph_id: source_key.graph_id().clone(),
        source_instance_id: source_key.source_instance_id().clone(),
        event_key: source_key,
        display_name: "Source".to_owned(),
        committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
        observation: None,
        lifecycle_fact: source_fact,
        policy_revision: None,
    })
    .expect("source envelope");
    let mut wire = serde_json::to_value(source).expect("source wire");
    wire["policy_revision"] = serde_json::json!(1);
    assert!(serde_json::from_value::<EventEnvelope>(wire).is_err());

    let (episode_key, _, episode_fact, episode_policy) = all_key_classes().remove(3);
    let episode = EventEnvelope::new(EventEnvelopeParts {
        graph_id: episode_key.graph_id().clone(),
        source_instance_id: episode_key.source_instance_id().clone(),
        event_key: episode_key,
        display_name: "Measurement".to_owned(),
        committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
        observation: None,
        lifecycle_fact: episode_fact,
        policy_revision: episode_policy,
    })
    .expect("episode");
    let mut wire = serde_json::to_value(episode).expect("episode wire");
    wire["lifecycle_fact"] = serde_json::json!("different");
    assert!(serde_json::from_value::<EventEnvelope>(wire).is_err());

    let mut invalid_keys = Vec::new();
    for (index, (key, _, _, _)) in all_key_classes().into_iter().enumerate() {
        let mut wire = serde_json::to_value(key).expect("key wire");
        match index {
            0 => wire["observation_seq"] = serde_json::json!(0),
            1 => wire["issue_code"] = serde_json::json!("invented"),
            2 => wire["event_kind"] = serde_json::json!("source_initialized"),
            3 => wire["code"] = serde_json::json!("invented"),
            4 => wire["measurement_episode_seq"] = serde_json::json!(0),
            5 => wire["event_kind"] = serde_json::json!("measurement_initialized"),
            6 => wire["code"] = serde_json::json!("invented"),
            7 => wire["source_episode_seq"] = serde_json::json!(0),
            _ => unreachable!(),
        }
        invalid_keys.push(serde_json::from_value::<EventKey>(wire).expect("structural key"));
    }
    for key in invalid_keys {
        assert!(key.validate().is_err());
    }
    for index in [3_usize, 6] {
        let (key, _, _, _) = all_key_classes().remove(index);
        let mut wire = serde_json::to_value(key).expect("key wire");
        wire["event_kind"] = serde_json::json!("condition_satisfied");
        let invalid: EventKey = serde_json::from_value(wire).expect("structural key");
        assert!(invalid.validate().is_err());
    }

    for (index, replacement) in [
        (0, serde_json::json!("short")),
        (2, serde_json::json!("G".repeat(64))),
        (5, serde_json::json!("short")),
    ] {
        let (key, _, _, _) = all_key_classes().remove(index);
        let mut wire = serde_json::to_value(key).expect("key wire");
        let field = if index == 5 {
            "source_representation_digest"
        } else if index == 2 {
            "measurement_value_digest"
        } else {
            "condition_defn_digest"
        };
        wire[field] = replacement;
        let invalid: EventKey = serde_json::from_value(wire).expect("structural key");
        assert!(invalid.validate().is_err());
    }
}

#[test]
fn event_scope_helpers_and_emitters_validate_each_lineage_layer_directly() {
    let graph = GraphId::mint();
    let source = SourceInstanceId::mint();
    let measurement = MeasurementInstanceId::mint();
    let condition = ConditionId::new("condition").expect("condition");
    super::key::validate_source_ids(&graph, &source).expect("source lineage");
    super::key::validate_measurement_ids(&graph, &source, &measurement)
        .expect("measurement lineage");
    super::key::validate_measurement_condition(
        &graph,
        &source,
        &measurement,
        &condition,
        &"a".repeat(64),
        1,
    )
    .expect("condition lineage");
    assert!(super::key::validate_source_ids(&GraphId::non_v4_for_tests(), &source).is_err());
    assert!(
        super::key::validate_source_ids(&graph, &SourceInstanceId::non_v4_for_tests()).is_err()
    );
    assert!(
        super::key::validate_measurement_ids(
            &graph,
            &source,
            &MeasurementInstanceId::non_v4_for_tests(),
        )
        .is_err()
    );

    let measurement_for_wire = measurement.clone();
    let emitter = EventEmitter::measurement(
        SourceId::new("shop").expect("source id"),
        MeasurementId::new("price").expect("measurement id"),
        measurement,
    );
    emitter.validate().expect("measurement emitter");
    assert_eq!(
        serde_json::to_value(&emitter).expect("emitter wire"),
        serde_json::json!({
            "kind": "measurement",
            "source_id": "shop",
            "measurement_id": "price",
            "measurement_instance_id": serde_json::to_value(measurement_for_wire)
                .expect("instance wire")
        })
    );
    let crossed: EventEmitter = serde_json::from_value(serde_json::json!({
        "kind": "source",
        "source_id": "shop",
        "measurement_id": "price"
    }))
    .expect("syntactically valid crossed emitter");
    assert!(crossed.validate().is_err());
}

use super::super::payload::{
    require_canonical_timestamp, require_condition_fact, require_contract_digest,
    require_observation_fact, require_route_family,
};
use super::super::*;
use super::support::*;
use crate::{IntegrationFaultCode, PermanentErrorCode, SourceSuspectReason, TargetId};

#[test]
fn delivery_payload_guards_reject_every_incoherent_fact_before_drain() {
    let target_id = TargetId::new("demo").expect("target id");
    let reject_record =
        |payload: &ProcessStdinPayload, route_id: &RouteId, route_family: RouteFamily| {
            assert!(
                payload
                    .validate_for_record(
                        &target_id,
                        CONTRACT_DIGEST,
                        payload.event_id(),
                        route_id,
                        route_family,
                    )
                    .is_err()
            );
        };

    for (kind, spelling) in [
        (DeliveryEventKind::ConditionSatisfied, "condition_satisfied"),
        (DeliveryEventKind::Initialized, "initialized"),
        (DeliveryEventKind::Reset, "reset"),
        (DeliveryEventKind::ArithmeticOverflow, "arithmetic_overflow"),
        (DeliveryEventKind::ZeroReference, "zero_reference"),
        (
            DeliveryEventKind::SourceSuspectEscalated,
            "source_suspect_escalated",
        ),
        (
            DeliveryEventKind::PermanentContractError,
            "permanent_contract_error",
        ),
        (DeliveryEventKind::IntegrationFault, "integration_fault"),
    ] {
        assert_eq!(kind.as_str(), spelling);
    }

    assert!(
        ProcessStdinEventKey::ConditionEvent {
            condition_id: "changed".parse().expect("condition id"),
            observation_seq: 0,
        }
        .validate(None)
        .is_err()
    );
    assert!(
        ProcessStdinEventKey::ConditionLevel {
            condition_id: "changed".parse().expect("condition id"),
            entry_at: "2026-07-15T00:00:00+00:00".to_owned(),
        }
        .validate(None)
        .is_err()
    );
    assert!(
        ProcessStdinEventKey::Initialized {
            contract_digest_sha256: "not-a-digest".to_owned(),
        }
        .validate(None)
        .is_err()
    );
    assert!(
        ProcessStdinEventKey::IntegrationFault {
            contract_digest_sha256: "not-a-digest".to_owned(),
            integration_fault_code: IntegrationFaultCode::HtmlcutInternalError,
            first_seen_at: FIRST.to_owned(),
        }
        .validate(None)
        .is_err()
    );

    let condition_level = golden_event_keys().remove(1).event_key;
    let (_route, _family, mut payload) = payload_for_event_key(condition_level, &target_id);
    payload.condition_id = None;
    assert!(payload.event_key.validate_payload_facts(&payload).is_err());
    payload.condition_id = Some("other".parse().expect("condition id"));
    assert!(payload.event_key.validate_payload_facts(&payload).is_err());

    let source = golden_event_keys().remove(6).event_key;
    let (_route, _family, mut payload) = payload_for_event_key(source, &target_id);
    payload.reason_class = Some(SourceSuspectReason::JsonMalformed);
    assert!(payload.event_key.validate_payload_facts(&payload).is_err());
    payload.reason_class = Some(SourceSuspectReason::FetchFailed);
    payload.episode_started_at = Some(SECOND.to_owned());
    assert!(payload.event_key.validate_payload_facts(&payload).is_err());

    let permanent = golden_event_keys().remove(7).event_key;
    let (_route, _family, mut payload) = payload_for_event_key(permanent, &target_id);
    payload.error_code = Some(PermanentErrorCode::HtmlcutPlanInvalid);
    assert!(payload.event_key.validate_payload_facts(&payload).is_err());
    payload.error_code = Some(PermanentErrorCode::InvalidJsonPointer);
    payload.episode_started_at = Some(SECOND.to_owned());
    assert!(payload.event_key.validate_payload_facts(&payload).is_err());

    let integration = golden_event_keys().remove(8).event_key;
    let (_route, _family, mut payload) = payload_for_event_key(integration, &target_id);
    payload.integration_fault_code = Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation);
    assert!(payload.event_key.validate_payload_facts(&payload).is_err());
    payload.integration_fault_code = Some(IntegrationFaultCode::HtmlcutInternalError);
    payload.episode_started_at = Some(SECOND.to_owned());
    assert!(payload.event_key.validate_payload_facts(&payload).is_err());

    let integration = golden_event_keys().remove(8).event_key;
    let (_route, _family, mut payload) = payload_for_event_key(integration, &target_id);
    payload.integration_fault_code = None;
    assert!(payload.expected_summary().is_err());

    let source = golden_event_keys().remove(6).event_key;
    let (route, family, mut payload) = payload_for_event_key(source, &target_id);
    payload.integration_fault_code = Some(IntegrationFaultCode::HtmlcutInternalError);
    reject_record(&payload, &route, family);

    let permanent = golden_event_keys().remove(7).event_key;
    let (route, family, mut payload) = payload_for_event_key(permanent, &target_id);
    payload.integration_fault_code = Some(IntegrationFaultCode::HtmlcutInternalError);
    reject_record(&payload, &route, family);

    let integration = golden_event_keys().remove(8).event_key;
    let (route, family, mut payload) = payload_for_event_key(integration, &target_id);
    payload.integration_fault_code = None;
    reject_record(&payload, &route, family);

    let integration = golden_event_keys().remove(8).event_key;
    let (route, family, mut payload) = payload_for_event_key(integration, &target_id);
    payload.condition_id = Some("unexpected".parse().expect("condition id"));
    reject_record(&payload, &route, family);

    let condition = golden_event_keys().remove(0).event_key;
    let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
    payload.schema_version = 99;
    reject_record(&payload, &route, family);

    let condition = golden_event_keys().remove(0).event_key;
    let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
    payload.schema_name = "unexpected.schema".to_owned();
    reject_record(&payload, &route, family);

    let condition = golden_event_keys().remove(0).event_key;
    let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
    payload.target_id = TargetId::new("other").expect("target id");
    reject_record(&payload, &route, family);

    let condition = golden_event_keys().remove(0).event_key;
    let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
    payload.event_id = "other-event".to_owned();
    reject_record(&payload, &route, family);

    let condition = golden_event_keys().remove(0).event_key;
    let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
    payload.route_id = RouteId::new("other").expect("route id");
    reject_record(&payload, &route, family);

    let condition = golden_event_keys().remove(0).event_key;
    let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
    payload.route_family = RouteFamily::OnRun;
    reject_record(&payload, &route, family);

    let condition = golden_event_keys().remove(0).event_key;
    let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
    payload.event_kind = DeliveryEventKind::Reset;
    reject_record(&payload, &route, family);

    let condition = golden_event_keys().remove(0).event_key;
    let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
    payload.reason_class = Some(SourceSuspectReason::FetchFailed);
    reject_record(&payload, &route, family);

    let initialized = golden_event_keys().remove(2).event_key;
    let (route, family, mut payload) = payload_for_event_key(initialized, &target_id);
    payload.condition_id = Some("changed".parse().expect("condition id"));
    reject_record(&payload, &route, family);

    let initialized = golden_event_keys().remove(2).event_key;
    let (route, family, mut payload) = payload_for_event_key(initialized, &target_id);
    payload.reason_class = Some(SourceSuspectReason::FetchFailed);
    reject_record(&payload, &route, family);

    let reset = golden_event_keys().remove(3).event_key;
    let (route, family, mut payload) = payload_for_event_key(reset, &target_id);
    payload.canonical_value = Some("7".to_owned());
    reject_record(&payload, &route, family);

    let reset = golden_event_keys().remove(3).event_key;
    let (route, family, mut payload) = payload_for_event_key(reset, &target_id);
    payload.episode_started_at = Some(FIRST.to_owned());
    reject_record(&payload, &route, family);

    let arithmetic = golden_event_keys().remove(4).event_key;
    let (route, family, mut payload) = payload_for_event_key(arithmetic, &target_id);
    payload.episode_started_at = Some(FIRST.to_owned());
    reject_record(&payload, &route, family);

    let source = golden_event_keys().remove(6).event_key;
    let (route, family, mut payload) = payload_for_event_key(source, &target_id);
    payload.error_code = Some(PermanentErrorCode::InvalidJsonPointer);
    reject_record(&payload, &route, family);

    let source = golden_event_keys().remove(6).event_key;
    let (route, family, mut payload) = payload_for_event_key(source, &target_id);
    payload.condition_id = Some("changed".parse().expect("condition id"));
    reject_record(&payload, &route, family);

    let source = golden_event_keys().remove(6).event_key;
    let (route, family, mut payload) = payload_for_event_key(source, &target_id);
    payload.reason_class = None;
    reject_record(&payload, &route, family);

    let source = golden_event_keys().remove(6).event_key;
    let (route, family, mut payload) = payload_for_event_key(source, &target_id);
    payload.episode_started_at = None;
    reject_record(&payload, &route, family);

    let integration = golden_event_keys().remove(8).event_key;
    let (route, family, mut payload) = payload_for_event_key(integration, &target_id);
    payload.reason_class = Some(SourceSuspectReason::FetchFailed);
    reject_record(&payload, &route, family);

    let integration = golden_event_keys().remove(8).event_key;
    let (route, family, mut payload) = payload_for_event_key(integration, &target_id);
    payload.error_code = Some(PermanentErrorCode::InvalidJsonPointer);
    reject_record(&payload, &route, family);

    let integration = golden_event_keys().remove(8).event_key;
    let (route, family, mut payload) = payload_for_event_key(integration, &target_id);
    payload.integration_fault_code = None;
    reject_record(&payload, &route, family);

    let integration = golden_event_keys().remove(8).event_key;
    let (route, family, mut payload) = payload_for_event_key(integration, &target_id);
    payload.episode_started_at = None;
    reject_record(&payload, &route, family);

    let permanent = golden_event_keys().remove(7).event_key;
    let (route, family, mut payload) = payload_for_event_key(permanent, &target_id);
    payload.reason_class = Some(SourceSuspectReason::FetchFailed);
    reject_record(&payload, &route, family);

    let permanent = golden_event_keys().remove(7).event_key;
    let (route, family, mut payload) = payload_for_event_key(permanent, &target_id);
    payload.condition_id = Some("changed".parse().expect("condition id"));
    reject_record(&payload, &route, family);

    let permanent = golden_event_keys().remove(7).event_key;
    let (route, family, mut payload) = payload_for_event_key(permanent, &target_id);
    payload.error_code = None;
    reject_record(&payload, &route, family);

    let permanent = golden_event_keys().remove(7).event_key;
    let (route, family, mut payload) = payload_for_event_key(permanent, &target_id);
    payload.episode_started_at = None;
    reject_record(&payload, &route, family);

    let condition = golden_event_keys().remove(0).event_key;
    let (_route, _family, payload) = payload_for_event_key(condition, &target_id);
    assert!(require_route_family(&payload, RouteFamily::OnRun).is_err());
    let mut no_observation = payload.clone();
    no_observation.observation_seq = None;
    assert!(require_observation_fact(&no_observation, "condition").is_err());
    let mut no_canonical_value = payload.clone();
    no_canonical_value.canonical_value = Some(String::new());
    assert!(require_observation_fact(&no_canonical_value, "condition").is_err());
    let mut no_condition = payload.clone();
    no_condition.condition_id = None;
    assert!(require_condition_fact(&no_condition, "condition").is_err());
    assert!(require_canonical_timestamp("timestamp", "2026-07-15T00:00:00+00:00").is_err());
    assert!(require_canonical_timestamp("timestamp", "2026-07-15T02:00:00+02:00").is_err());
    assert!(require_canonical_timestamp("timestamp", "2026-07-15T00:00:00.000Z").is_err());
    assert!(require_contract_digest("not-a-digest", None).is_err());
    assert!(require_contract_digest(&"g".repeat(64), None).is_err());
    let mut mismatched = payload.clone();
    mismatched.observation_seq = Some(8);
    assert!(
        mismatched
            .event_key
            .validate_payload_facts(&mismatched)
            .is_err()
    );
    let mut different_condition = payload.clone();
    different_condition.condition_id = Some("other".parse().expect("condition id"));
    assert!(
        different_condition
            .event_key
            .validate_payload_facts(&different_condition)
            .is_err()
    );

    let mut missing_condition = payload.clone();
    missing_condition.condition_id = None;
    assert!(missing_condition.expected_summary().is_err());
    let mut missing_sequence = payload.clone();
    missing_sequence.observation_seq = None;
    assert!(missing_sequence.expected_summary().is_err());
    let mut missing_value = payload;
    missing_value.canonical_value = None;
    assert!(missing_value.expected_summary().is_err());

    let reset = golden_event_keys().remove(3).event_key;
    let (route, family, mut payload) = payload_for_event_key(reset, &target_id);
    payload.summary = "incorrect summary".to_owned();
    reject_record(&payload, &route, family);
}

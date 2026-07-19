use serde_json::{Value, json};

use super::super::*;
use crate::{CoreError, IntegrationFaultCode, PermanentErrorCode, SourceSuspectReason, TargetId};

pub(super) const CONTRACT_DIGEST: &str =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub(super) const FIRST: &str = "2026-07-15T12:00:00Z";
pub(super) const SECOND: &str = "2026-07-15T13:00:00Z";

pub(super) struct EventKeyGolden {
    pub(super) name: &'static str,
    pub(super) event_key: ProcessStdinEventKey,
    pub(super) route_family: RouteFamily,
    pub(super) expected_identity: Value,
    pub(super) expected_event_id: &'static str,
}

pub(super) struct EventKeyMutation {
    pub(super) name: &'static str,
    pub(super) golden_index: usize,
    pub(super) field: &'static str,
    pub(super) replacement: Value,
    pub(super) expected_rejection: ExpectedRejection,
}

#[derive(Clone, Copy)]
pub(super) enum ExpectedRejection {
    Contract(&'static str),
    Json,
}

pub(super) fn contract_message(error: CoreError) -> Result<String, CoreError> {
    match error {
        CoreError::Contract(message) => Ok(message),
        other => Err(other),
    }
}

pub(super) fn golden_event_keys() -> Vec<EventKeyGolden> {
    vec![
        EventKeyGolden {
            name: "condition event",
            event_key: ProcessStdinEventKey::ConditionEvent {
                condition_id: "changed".parse().expect("condition id"),
                observation_seq: 7,
            },
            route_family: RouteFamily::OnCondition,
            expected_identity: json!({
                "condition_id": "changed",
                "observation_seq": 7,
            }),
            expected_event_id: "e5a065277023e78d8547b8f6b4f5b688fe844441d64e000f6ef0023ce2bc1bd1",
        },
        EventKeyGolden {
            name: "condition level",
            event_key: ProcessStdinEventKey::ConditionLevel {
                condition_id: "low".parse().expect("condition id"),
                entry_at: FIRST.to_owned(),
            },
            route_family: RouteFamily::OnCondition,
            expected_identity: json!({
                "condition_id": "low",
                "entry_at": FIRST,
            }),
            expected_event_id: "76729aeaf7c2b9c510acb9ec7d66f288388911ec3dd62ad6fd53df7402ea62aa",
        },
        EventKeyGolden {
            name: "initialized",
            event_key: ProcessStdinEventKey::Initialized {
                contract_digest_sha256: CONTRACT_DIGEST.to_owned(),
            },
            route_family: RouteFamily::OnRun,
            expected_identity: json!({
                "kind": "initialized",
                "contract_digest_sha256": CONTRACT_DIGEST,
            }),
            expected_event_id: "a2b4994f06b8959d56e9b280dcef69989bf50b97c923f2d61cb069f1de3b578a",
        },
        EventKeyGolden {
            name: "reset",
            event_key: ProcessStdinEventKey::Reset {
                contract_digest_sha256: CONTRACT_DIGEST.to_owned(),
            },
            route_family: RouteFamily::OnRun,
            expected_identity: json!({
                "kind": "reset",
                "contract_digest_sha256": CONTRACT_DIGEST,
            }),
            expected_event_id: "79321abe28b96f48d93796ca62ac548025a6d305a3063e96d3f9a202e1cd3af7",
        },
        EventKeyGolden {
            name: "arithmetic overflow",
            event_key: ProcessStdinEventKey::ArithmeticOverflow {
                condition_id: "changed".parse().expect("condition id"),
                observation_seq: 7,
            },
            route_family: RouteFamily::OnRun,
            expected_identity: json!({
                "condition_id": "changed",
                "kind": "arithmetic_overflow",
                "observation_seq": 7,
            }),
            expected_event_id: "c47e14b2645cbc248cbf7076752bb3df5020dcbc803c44d0460e98878ba90b94",
        },
        EventKeyGolden {
            name: "zero reference",
            event_key: ProcessStdinEventKey::ZeroReference {
                condition_id: "low".parse().expect("condition id"),
                observation_seq: 7,
            },
            route_family: RouteFamily::OnRun,
            expected_identity: json!({
                "condition_id": "low",
                "kind": "zero_reference",
                "observation_seq": 7,
            }),
            expected_event_id: "0168719a74209a7bfd8feedec32a7f12cd398f488d5f592158c2716fdc138dce",
        },
        EventKeyGolden {
            name: "source suspect escalated",
            event_key: ProcessStdinEventKey::SourceSuspectEscalated {
                reason_class: SourceSuspectReason::FetchFailed,
                episode: FIRST.to_owned(),
            },
            route_family: RouteFamily::OnRun,
            expected_identity: json!({
                "reason_class": "fetch_failed",
                "episode": FIRST,
            }),
            expected_event_id: "7207d3b9286e32289e3f5b96697219660a028121b01db3baaef2a795f17e973d",
        },
        EventKeyGolden {
            name: "permanent contract error",
            event_key: ProcessStdinEventKey::PermanentContractError {
                contract_digest_sha256: CONTRACT_DIGEST.to_owned(),
                error_code: PermanentErrorCode::InvalidJsonPointer,
                first_seen_at: FIRST.to_owned(),
            },
            route_family: RouteFamily::OnRun,
            expected_identity: json!({
                "contract_digest_sha256": CONTRACT_DIGEST,
                "error_code": "invalid_json_pointer",
                "first_seen_at": FIRST,
            }),
            expected_event_id: "b0abfcb9a21fe6f12f1dbe3d7807387592f53e0f3953154da594ab133a3e50a9",
        },
        EventKeyGolden {
            name: "integration fault",
            event_key: ProcessStdinEventKey::IntegrationFault {
                contract_digest_sha256: CONTRACT_DIGEST.to_owned(),
                integration_fault_code: IntegrationFaultCode::HtmlcutInternalError,
                first_seen_at: FIRST.to_owned(),
            },
            route_family: RouteFamily::OnRun,
            expected_identity: json!({
                "contract_digest_sha256": CONTRACT_DIGEST,
                "integration_fault_code": "htmlcut_internal_error",
                "first_seen_at": FIRST,
            }),
            expected_event_id: "64e7fb4cfc56d21ad55fffeb2f44b3a3adab6182681edb310bdc7a7245523137",
        },
    ]
}

pub(super) fn payload_for_event_key(
    event_key: ProcessStdinEventKey,
    target_id: &TargetId,
) -> (RouteId, RouteFamily, ProcessStdinPayload) {
    let (
        route_id,
        route_family,
        summary,
        condition_id,
        observation_seq,
        canonical_value,
        reason_class,
        error_code,
        integration_fault_code,
        episode_started_at,
    ) = match &event_key {
        ProcessStdinEventKey::ConditionEvent {
            condition_id,
            observation_seq,
        } => (
            "condition",
            RouteFamily::OnCondition,
            format!(
                "Demo[condition={condition_id}]: satisfied at observation {observation_seq} (value={observation_seq})"
            ),
            Some(condition_id.clone()),
            Some(*observation_seq),
            Some(observation_seq.to_string()),
            None,
            None,
            None,
            None,
        ),
        ProcessStdinEventKey::ConditionLevel { condition_id, .. } => (
            "condition",
            RouteFamily::OnCondition,
            format!("Demo[condition={condition_id}]: satisfied at observation 7 (value=7)"),
            Some(condition_id.clone()),
            Some(7),
            Some("7".to_owned()),
            None,
            None,
            None,
            None,
        ),
        ProcessStdinEventKey::Initialized { .. } => (
            "run",
            RouteFamily::OnRun,
            "Demo: initialized".to_owned(),
            None,
            Some(7),
            Some("7".to_owned()),
            None,
            None,
            None,
            None,
        ),
        ProcessStdinEventKey::Reset { .. } => (
            "run",
            RouteFamily::OnRun,
            "Demo: reset".to_owned(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ),
        ProcessStdinEventKey::ArithmeticOverflow {
            condition_id,
            observation_seq,
        } => (
            "run",
            RouteFamily::OnRun,
            format!(
                "Demo[condition={condition_id}]: arithmetic_overflow at observation {observation_seq}"
            ),
            Some(condition_id.clone()),
            Some(*observation_seq),
            Some(observation_seq.to_string()),
            None,
            None,
            None,
            None,
        ),
        ProcessStdinEventKey::ZeroReference {
            condition_id,
            observation_seq,
        } => (
            "run",
            RouteFamily::OnRun,
            format!(
                "Demo[condition={condition_id}]: zero_reference at observation {observation_seq}"
            ),
            Some(condition_id.clone()),
            Some(*observation_seq),
            Some(observation_seq.to_string()),
            None,
            None,
            None,
            None,
        ),
        ProcessStdinEventKey::SourceSuspectEscalated {
            reason_class,
            episode,
        } => (
            "run",
            RouteFamily::OnRun,
            format!("Demo: source health escalated ({})", reason_class.as_str()),
            None,
            None,
            None,
            Some(*reason_class),
            None,
            None,
            Some(episode.clone()),
        ),
        ProcessStdinEventKey::PermanentContractError {
            error_code,
            first_seen_at,
            ..
        } => (
            "run",
            RouteFamily::OnRun,
            format!("Demo: permanent contract error ({})", error_code.as_str()),
            None,
            None,
            None,
            None,
            Some(*error_code),
            None,
            Some(first_seen_at.clone()),
        ),
        ProcessStdinEventKey::IntegrationFault {
            integration_fault_code,
            first_seen_at,
            ..
        } => (
            "run",
            RouteFamily::OnRun,
            format!(
                "Demo: integration fault ({})",
                integration_fault_code.as_str()
            ),
            None,
            None,
            None,
            None,
            None,
            Some(*integration_fault_code),
            Some(first_seen_at.clone()),
        ),
    };
    let route_id = RouteId::new(route_id).expect("route id");
    let payload = ProcessStdinPayload::new(
        &route_id,
        route_family,
        target_id,
        "Demo",
        event_key,
        &summary,
        condition_id,
        observation_seq,
        canonical_value,
        reason_class,
        error_code,
        integration_fault_code,
        episode_started_at,
    )
    .expect("valid payload");
    (route_id, route_family, payload)
}

use super::super::*;
use super::support::*;
use crate::{CoreError, TargetId};
use serde_json::{Value, json};

#[test]
fn persisted_payload_rejects_mutation_of_every_event_key_fact() {
    let target_id = TargetId::new("demo").expect("target id");
    let goldens = golden_event_keys();
    let mutations = vec![
        EventKeyMutation {
            name: "condition event condition id",
            golden_index: 0,
            field: "condition_id",
            replacement: json!("recovered"),
            expected_rejection: ExpectedRejection::Contract(
                "outbox event_id must be derived from its persisted event_key",
            ),
        },
        EventKeyMutation {
            name: "condition event observation sequence",
            golden_index: 0,
            field: "observation_seq",
            replacement: json!(8),
            expected_rejection: ExpectedRejection::Contract(
                "outbox event_id must be derived from its persisted event_key",
            ),
        },
        EventKeyMutation {
            name: "condition level condition id",
            golden_index: 1,
            field: "condition_id",
            replacement: json!("high"),
            expected_rejection: ExpectedRejection::Contract(
                "outbox event_id must be derived from its persisted event_key",
            ),
        },
        EventKeyMutation {
            name: "condition level entry instant",
            golden_index: 1,
            field: "entry_at",
            replacement: json!(SECOND),
            expected_rejection: ExpectedRejection::Contract(
                "outbox event_id must be derived from its persisted event_key",
            ),
        },
        EventKeyMutation {
            name: "initialized contract digest",
            golden_index: 2,
            field: "contract_digest_sha256",
            replacement: json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            expected_rejection: ExpectedRejection::Contract(
                "outbox immutable_payload event_key contract_digest_sha256 must match state",
            ),
        },
        EventKeyMutation {
            name: "reset contract digest",
            golden_index: 3,
            field: "contract_digest_sha256",
            replacement: json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            expected_rejection: ExpectedRejection::Contract(
                "outbox immutable_payload event_key contract_digest_sha256 must match state",
            ),
        },
        EventKeyMutation {
            name: "arithmetic-overflow condition id",
            golden_index: 4,
            field: "condition_id",
            replacement: json!("recovered"),
            expected_rejection: ExpectedRejection::Contract(
                "outbox event_id must be derived from its persisted event_key",
            ),
        },
        EventKeyMutation {
            name: "arithmetic-overflow observation sequence",
            golden_index: 4,
            field: "observation_seq",
            replacement: json!(8),
            expected_rejection: ExpectedRejection::Contract(
                "outbox event_id must be derived from its persisted event_key",
            ),
        },
        EventKeyMutation {
            name: "zero-reference condition id",
            golden_index: 5,
            field: "condition_id",
            replacement: json!("high"),
            expected_rejection: ExpectedRejection::Contract(
                "outbox event_id must be derived from its persisted event_key",
            ),
        },
        EventKeyMutation {
            name: "zero-reference observation sequence",
            golden_index: 5,
            field: "observation_seq",
            replacement: json!(8),
            expected_rejection: ExpectedRejection::Contract(
                "outbox event_id must be derived from its persisted event_key",
            ),
        },
        EventKeyMutation {
            name: "source-suspect reason class",
            golden_index: 6,
            field: "reason_class",
            replacement: json!("json_malformed"),
            expected_rejection: ExpectedRejection::Contract(
                "outbox event_id must be derived from its persisted event_key",
            ),
        },
        EventKeyMutation {
            name: "source-suspect episode",
            golden_index: 6,
            field: "episode",
            replacement: json!(SECOND),
            expected_rejection: ExpectedRejection::Contract(
                "outbox event_id must be derived from its persisted event_key",
            ),
        },
        EventKeyMutation {
            name: "permanent-error contract digest",
            golden_index: 7,
            field: "contract_digest_sha256",
            replacement: json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            expected_rejection: ExpectedRejection::Contract(
                "outbox immutable_payload event_key contract_digest_sha256 must match state",
            ),
        },
        EventKeyMutation {
            name: "permanent-error code",
            golden_index: 7,
            field: "error_code",
            replacement: json!("future_contract_error"),
            expected_rejection: ExpectedRejection::Json,
        },
        EventKeyMutation {
            name: "permanent-error first-seen instant",
            golden_index: 7,
            field: "first_seen_at",
            replacement: json!(SECOND),
            expected_rejection: ExpectedRejection::Contract(
                "outbox event_id must be derived from its persisted event_key",
            ),
        },
    ];

    for mutation in mutations {
        let golden = &goldens[mutation.golden_index];
        let (route_id, route_family, payload) =
            payload_for_event_key(golden.event_key.clone(), &target_id);
        let event_id = payload.event_id().to_owned();
        let payload_bytes = payload.immutable_bytes().expect("canonical payload");
        read_validated_process_stdin_payload_bytes(
            &payload_bytes,
            &target_id,
            CONTRACT_DIGEST,
            &event_id,
            &route_id,
            route_family,
        )
        .expect("baseline payload");

        let mut wire: Value = serde_json::from_slice(&payload_bytes).expect("payload JSON");
        wire["event_key"][mutation.field] = mutation.replacement;
        let mutated_bytes = crate::stable_json::stable_json(&wire)
            .expect("canonical mutated payload")
            .into_bytes();
        let result = read_validated_process_stdin_payload_bytes(
            &mutated_bytes,
            &target_id,
            CONTRACT_DIGEST,
            &event_id,
            &route_id,
            route_family,
        );

        let error = result.expect_err("every event-key mutation must be rejected");
        match mutation.expected_rejection {
            ExpectedRejection::Contract(expected) => assert_eq!(
                contract_message(error).expect("mutation must be a contract rejection"),
                expected,
                "{}",
                mutation.name
            ),
            ExpectedRejection::Json => assert!(matches!(
                contract_message(error).expect_err("mutation must be a JSON rejection"),
                CoreError::Json(_)
            )),
        }
    }
}

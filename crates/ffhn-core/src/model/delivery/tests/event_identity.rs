use super::support::*;
use crate::TargetId;

#[test]
fn process_stdin_event_keys_keep_the_complete_exact_identity_contract() {
    let target_id = TargetId::new("demo").expect("target id");

    for golden in golden_event_keys() {
        let identity = golden.event_key.identity_json();
        assert_eq!(
            identity, golden.expected_identity,
            "{} must preserve its exact identity-key shape",
            golden.name
        );
        assert_eq!(
            golden
                .event_key
                .event_id(&target_id, golden.route_family)
                .expect("event id"),
            golden.expected_event_id,
            "{} must preserve its exact deterministic event id",
            golden.name
        );
    }
}

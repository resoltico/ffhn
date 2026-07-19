use super::support::*;

#[test]
fn state_episodes_preserve_the_closed_taxonomy_boundaries() {
    let document = target("integer", "");
    let observation = document
        .parse_json_scalar_token("7".to_owned())
        .expect("observation");
    let digest = document.contract_digest_sha256().expect("digest");
    let mut state = StateDocument::new(TargetId::new("demo").expect("target"), digest);

    assert!(
        state
            .apply_permanent_error(
                PermanentErrorCode::InvalidJsonPointer,
                "2026-07-15T00:00:00Z",
            )
            .expect("first permanent episode")
    );
    assert!(
        !state
            .apply_permanent_error(
                PermanentErrorCode::InvalidJsonPointer,
                "2026-07-15T00:00:01Z",
            )
            .expect("continued permanent episode")
    );
    let continued_permanent = serde_json::to_value(&state).expect("permanent state JSON");
    assert_eq!(
        continued_permanent["permanent_error_episode"]["error_code"],
        "invalid_json_pointer"
    );
    assert_eq!(
        continued_permanent["permanent_error_episode"]["first_seen_at"],
        "2026-07-15T00:00:00Z"
    );

    state
        .apply_valid_observation(&document, observation, &[], "2026-07-15T00:00:03Z")
        .expect("valid observation clears permanent episode");
    assert!(
        serde_json::to_value(&state).expect("recovered state JSON")["permanent_error_episode"]
            .is_null()
    );
    assert!(
        state
            .apply_permanent_error(
                PermanentErrorCode::InvalidJsonPointer,
                "2026-07-15T00:00:04Z",
            )
            .expect("recurrent permanent episode")
    );
    assert_eq!(
        serde_json::to_value(&state).expect("recurrent state JSON")["permanent_error_episode"]["first_seen_at"],
        "2026-07-15T00:00:04Z"
    );

    let json_detail = plain_detail(
        DiagnosticKind::Json,
        DiagnosticOperation::JsonPointerSelection,
        "source detail",
        None,
    );
    let value_detail = plain_detail(
        DiagnosticKind::ValueUnparseable,
        DiagnosticOperation::ValueParse,
        "source detail",
        None,
    );
    assert!(
        !state
            .apply_source_suspect(
                SourceSuspectReason::JsonMalformed,
                json_detail.clone(),
                "2026-07-15T00:00:05Z",
                3,
            )
            .expect("first source-suspect failure")
    );
    assert!(
        !state
            .apply_source_suspect(
                SourceSuspectReason::JsonMalformed,
                json_detail,
                "2026-07-15T00:00:06Z",
                3,
            )
            .expect("continued source-suspect failure")
    );
    assert!(
        !state
            .apply_source_suspect(
                SourceSuspectReason::ValueUnparseable,
                value_detail.clone(),
                "2026-07-15T00:00:07Z",
                3,
            )
            .expect("changed source-suspect classification")
    );
    let changed_source = serde_json::to_value(&state).expect("source state JSON");
    assert_eq!(changed_source["source_health"]["consecutive_unresolved"], 1);
    assert_eq!(
        changed_source["source_health"]["first_unresolved_at"],
        "2026-07-15T00:00:07Z"
    );
    assert!(
        !state
            .apply_source_suspect(
                SourceSuspectReason::ValueUnparseable,
                value_detail.clone(),
                "2026-07-15T00:00:08Z",
                3,
            )
            .expect("second source-suspect failure")
    );
    assert!(
        state
            .apply_source_suspect(
                SourceSuspectReason::ValueUnparseable,
                value_detail.clone(),
                "2026-07-15T00:00:09Z",
                3,
            )
            .expect("source escalation boundary")
    );
    assert!(
        !state
            .apply_source_suspect(
                SourceSuspectReason::ValueUnparseable,
                value_detail,
                "2026-07-15T00:00:10Z",
                3,
            )
            .expect("post-escalation source-suspect failure")
    );
}

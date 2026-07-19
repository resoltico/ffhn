use super::support::*;

#[test]
fn integration_fault_episodes_are_immediate_code_keyed_and_independent_of_source_health() {
    let document = target("integer", "");
    let observation = document
        .parse_json_scalar_token("7".to_owned())
        .expect("observation");
    let digest = document.contract_digest_sha256().expect("digest");
    let mut state = StateDocument::new(TargetId::new("demo").expect("target"), digest);
    let detail = io_detail(
        IoErrorClass::ConnectionRefused,
        DiagnosticOperation::HttpFetch,
        "source detail",
        None,
    );
    state
        .apply_source_suspect(
            SourceSuspectReason::FetchFailed,
            detail,
            "2026-07-15T00:00:00Z",
            3,
        )
        .expect("source health");
    let source_health = serde_json::to_value(&state).expect("state JSON")["source_health"].clone();

    assert!(
        state
            .apply_integration_fault(
                IntegrationFaultCode::HtmlcutInternalError,
                "2026-07-15T00:00:01Z",
            )
            .expect("integration episode begins")
    );
    assert!(
        !state
            .apply_integration_fault(
                IntegrationFaultCode::HtmlcutInternalError,
                "2026-07-15T00:00:02Z",
            )
            .expect("same integration episode continues")
    );
    let continued = serde_json::to_value(&state).expect("state JSON");
    assert_eq!(continued["source_health"], source_health);
    assert_eq!(continued["observation_seq"], 0);
    assert_eq!(
        continued["integration_fault_episode"]["integration_fault_code"],
        "htmlcut_internal_error"
    );
    assert_eq!(
        continued["integration_fault_episode"]["first_seen_at"],
        "2026-07-15T00:00:01Z"
    );

    assert!(
        state
            .apply_integration_fault(
                IntegrationFaultCode::FfhnBoundaryInvariantViolation,
                "2026-07-15T00:00:03Z",
            )
            .expect("changed integration code begins a new episode")
    );
    assert_eq!(
        serde_json::to_value(&state).expect("state JSON")["integration_fault_episode"]["first_seen_at"],
        "2026-07-15T00:00:03Z"
    );

    state
        .apply_valid_observation(&document, observation, &[], "2026-07-15T00:00:04Z")
        .expect("valid observation clears integration episode");
    assert!(
        serde_json::to_value(&state).expect("state JSON")["integration_fault_episode"].is_null()
    );
}

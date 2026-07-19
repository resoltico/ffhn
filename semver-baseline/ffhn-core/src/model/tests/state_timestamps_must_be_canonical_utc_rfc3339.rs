use super::support::*;

#[test]
fn state_timestamps_must_be_canonical_utc_rfc3339() {
    let document = target("integer", "");
    let digest = document.contract_digest_sha256().expect("digest");
    let mut state = StateDocument::new(TargetId::new("demo").expect("target"), digest);
    let detail = plain_detail(
        DiagnosticKind::Json,
        DiagnosticOperation::JsonPointerSelection,
        "source detail",
        None,
    );

    assert!(
        state
            .apply_source_suspect(
                SourceSuspectReason::JsonMalformed,
                detail.clone(),
                "2026-07-15T00:00:00+00:00",
                3
            )
            .is_err()
    );
    assert!(
        state
            .apply_integration_fault(
                IntegrationFaultCode::HtmlcutInternalError,
                "2026-07-15T02:00:00+02:00",
            )
            .is_err()
    );
    assert!(
        state
            .apply_permanent_error(
                PermanentErrorCode::InvalidJsonPointer,
                "2026-07-15T02:00:00+02:00",
            )
            .is_err()
    );

    state
        .apply_source_suspect(
            SourceSuspectReason::JsonMalformed,
            detail,
            "2026-07-15T00:00:00Z",
            3,
        )
        .expect("canonical source-health timestamp");
    let invalid_state = mutate_state(&state, |wire| {
        wire["source_health"]["first_unresolved_at"] =
            serde_json::json!("2026-07-15T00:00:00+00:00");
    });
    assert!(invalid_state.validate().is_err());

    state
        .apply_integration_fault(
            IntegrationFaultCode::HtmlcutInternalError,
            "2026-07-15T00:00:01Z",
        )
        .expect("canonical integration timestamp");
    let invalid_state = mutate_state(&state, |wire| {
        wire["integration_fault_episode"]["first_seen_at"] =
            serde_json::json!("2026-07-15T00:00:00+00:00");
    });
    assert!(invalid_state.validate().is_err());
}

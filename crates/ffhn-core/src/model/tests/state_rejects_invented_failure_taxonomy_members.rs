use super::support::*;

#[test]
fn state_rejects_invented_failure_taxonomy_members() {
    let document = target("integer", "");
    let digest = document.contract_digest_sha256().expect("digest");
    let mut state = StateDocument::new(TargetId::new("demo").expect("target"), digest);
    state
        .apply_source_suspect(
            SourceSuspectReason::JsonMalformed,
            plain_detail(
                DiagnosticKind::Json,
                DiagnosticOperation::JsonPointerSelection,
                "detail",
                None,
            ),
            "2026-07-15T00:00:00Z",
            3,
        )
        .expect("source health");
    let mut wire = serde_json::to_value(&state).expect("state JSON");
    wire["source_health"]["reason_class"] = serde_json::json!("invented_reason");
    assert!(serde_json::from_value::<StateDocument>(wire).is_err());

    let mut permanent_state = StateDocument::new(
        TargetId::new("demo").expect("target"),
        document.contract_digest_sha256().expect("digest"),
    );
    permanent_state
        .apply_permanent_error(
            PermanentErrorCode::InvalidJsonPointer,
            "2026-07-15T00:00:00Z",
        )
        .expect("permanent error");
    let mut wire = serde_json::to_value(permanent_state).expect("state JSON");
    wire["permanent_error_episode"]["error_code"] = serde_json::json!("invented_permanent_error");
    assert!(serde_json::from_value::<StateDocument>(wire).is_err());

    let mut integration_state = StateDocument::new(
        TargetId::new("demo").expect("target"),
        document.contract_digest_sha256().expect("digest"),
    );
    integration_state
        .apply_integration_fault(
            IntegrationFaultCode::HtmlcutInternalError,
            "2026-07-15T00:00:00Z",
        )
        .expect("integration fault");
    let mut wire = serde_json::to_value(integration_state).expect("state JSON");
    wire["integration_fault_episode"]["integration_fault_code"] =
        serde_json::json!("invented_integration_fault");
    assert!(serde_json::from_value::<StateDocument>(wire).is_err());
}

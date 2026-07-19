use super::support::*;
use crate::{HtmlcutDiagnosticCode, HtmlcutErrorClass};

#[test]
fn persisted_source_health_rejects_evidence_free_io_diagnostics() {
    let target = target("integer", "");
    let empty = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        target.contract_digest_sha256().expect("digest"),
    );
    let mut wire = serde_json::to_value(empty).expect("state JSON");
    wire["source_health"] = serde_json::json!({
        "state": "suspect",
        "reason_class": "fetch_failed",
        "consecutive_unresolved": 1,
        "first_unresolved_at": "2026-07-15T00:00:00Z",
        "last_details": {
            "kind": "io",
            "operation": "http_fetch",
            "message": "the HTTP request could not be completed"
        }
    });

    assert!(serde_json::from_value::<StateDocument>(wire).is_err());
}

#[test]
fn state_validation_rejects_each_incoherent_temporal_shape_before_runtime_use() {
    let plain_target = target("integer", "");
    let plain_digest = plain_target.contract_digest_sha256().expect("digest");
    let empty = StateDocument::new(TargetId::new("demo").expect("target id"), plain_digest);
    empty.validate().expect("empty state");
    let incoherent_baseline = mutate_state(&empty, |wire| {
        wire["observation_seq"] = serde_json::json!(1);
    });
    assert!(incoherent_baseline.validate().is_err());

    let one_condition = mutate_target(&plain_target, |wire| {
        wire["conditions"] = serde_json::json!([{
            "condition_id": "condition",
            "predicate": {"kind": "lt", "threshold": "20"},
        }]);
    });
    one_condition.validate().expect("condition target");
    let mut state = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        one_condition.contract_digest_sha256().expect("digest"),
    );
    let observation = one_condition
        .parse_json_scalar_token("10".to_owned())
        .expect("observation");
    let staged = one_condition
        .stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: &observation,
            },
            &state.condition_contexts(&one_condition),
        )
        .expect("policy stage");
    state
        .apply_valid_observation(
            &one_condition,
            observation,
            staged.condition_evaluations().expect("evaluations"),
            "2026-07-15T00:00:00Z",
        )
        .expect("valid temporal state");

    let type_mismatched_observations = mutate_state(&state, |wire| {
        wire["accepted_observation"]["declared_type"] = serde_json::json!("decimal");
        wire["fixed_initial_baseline"]["declared_type"] = serde_json::json!("decimal");
    });
    type_mismatched_observations
        .validate()
        .expect("individually valid typed observations");
    assert!(
        type_mismatched_observations
            .validate_for_target(&one_condition)
            .is_err()
    );

    let transition_mismatch = mutate_state(&state, |wire| {
        wire["condition_state"]["condition"]["last_transition_value"]["declared_type"] =
            serde_json::json!("decimal");
    });
    transition_mismatch
        .validate()
        .expect("individually valid transition value");
    assert!(
        transition_mismatch
            .validate_for_target(&one_condition)
            .is_err()
    );
    assert!(state.validate_for_target(&plain_target).is_err());

    let half_transition = mutate_state(&state, |wire| {
        wire["condition_state"]["condition"]["last_transition_at"] = serde_json::Value::Null;
    });
    assert!(half_transition.validate().is_err());

    let invalid_source_shapes = [
        mutate_state(&empty, |wire| {
            wire["source_health"] = serde_json::json!({
                "state": "healthy",
                "reason_class": "fetch_failed",
                "consecutive_unresolved": 0,
            });
        }),
        mutate_state(&empty, |wire| {
            wire["source_health"] = serde_json::json!({
                "state": "suspect",
                "consecutive_unresolved": 1,
            });
        }),
        mutate_state(&empty, |wire| {
            wire["source_health"] = serde_json::json!({
                "state": "suspect",
                "reason_class": "fetch_failed",
                "consecutive_unresolved": 0,
                "first_unresolved_at": "2026-07-15T00:00:00Z",
                "last_details": {"kind": "io", "operation": "file_read", "message": "failed", "io_error_class": "not_found"},
            });
        }),
        mutate_state(&empty, |wire| {
            wire["source_health"] = serde_json::json!({
                "state": "suspect",
                "reason_class": "fetch_failed",
                "consecutive_unresolved": 1,
                "first_unresolved_at": "2026-07-15T00:00:00Z",
                "last_details": {
                    "kind": "delivery",
                    "operation": "delivery_process",
                    "message": "delivery process did not complete successfully",
                    "delivery_process": {
                        "kind": "failure",
                        "attempt": {
                            "terminal": {"kind": "exited", "exit_code": 1},
                            "writer": {"kind": "completed"},
                            "stderr": {"kind": "captured", "retained_bytes_base64": "", "original_len_bytes": "0", "truncated": false}
                        },
                        "primary": "unsuccessful_exit"
                    }
                }
            });
        }),
        mutate_state(&empty, |wire| {
            wire["source_health"] = serde_json::json!({
                "state": "suspect",
                "reason_class": "fetch_failed",
                "consecutive_unresolved": 1,
                "first_unresolved_at": "2026-07-15T00:00:00Z",
                "last_details": {
                    "kind": "htmlcut",
                    "operation": "html_extraction",
                    "message": "HTMLCut boundary invariant failed",
                    "htmlcut_failure": {
                        "error_class": "ffhn_boundary_invariant_violation",
                        "plan_digest_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    },
                    "integration_fault_code": "ffhn_boundary_invariant_violation",
                }
            });
        }),
        mutate_state(&empty, |wire| {
            wire["source_health"] = serde_json::json!({
                "state": "suspect",
                "reason_class": "json_malformed",
                "consecutive_unresolved": 1,
                "first_unresolved_at": "2026-07-15T00:00:00Z",
                "last_details": {
                    "kind": "io",
                    "operation": "http_fetch",
                    "message": "the HTTP request could not be completed",
                    "io_error_class": "connection_refused"
                }
            });
        }),
    ];
    assert!(
        invalid_source_shapes
            .iter()
            .all(|state| state.validate().is_err())
    );

    let decimal_observation = target("decimal", "")
        .parse_json_scalar_token("10".to_owned())
        .expect("decimal observation");
    assert!(
        StateDocument::new(
            TargetId::new("demo").expect("target id"),
            one_condition.contract_digest_sha256().expect("digest"),
        )
        .apply_valid_observation(
            &one_condition,
            decimal_observation,
            &[],
            "2026-07-15T00:00:00Z"
        )
        .is_err()
    );
    assert!(
        StateDocument::new(
            TargetId::new("demo").expect("target id"),
            one_condition.contract_digest_sha256().expect("digest"),
        )
        .apply_valid_observation(
            &one_condition,
            one_condition
                .parse_json_scalar_token("10".to_owned())
                .expect("integer observation"),
            &[],
            "2026-07-15T00:00:00Z",
        )
        .is_err()
    );
    let other_condition = mutate_target(&one_condition, |wire| {
        wire["conditions"][0]["condition_id"] = serde_json::json!("other");
    });
    other_condition.validate().expect("other condition target");
    let other_observation = other_condition
        .parse_json_scalar_token("10".to_owned())
        .expect("other observation");
    let other_state = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        other_condition.contract_digest_sha256().expect("digest"),
    );
    let other_staged = other_condition
        .stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: &other_observation,
            },
            &other_state.condition_contexts(&other_condition),
        )
        .expect("other policy stage");
    assert!(
        StateDocument::new(
            TargetId::new("demo").expect("target id"),
            one_condition.contract_digest_sha256().expect("digest"),
        )
        .apply_valid_observation(
            &one_condition,
            one_condition
                .parse_json_scalar_token("10".to_owned())
                .expect("integer observation"),
            other_staged
                .condition_evaluations()
                .expect("other evaluations"),
            "2026-07-15T00:00:00Z",
        )
        .is_err()
    );
    assert!(
        empty
            .clone()
            .apply_source_suspect(
                SourceSuspectReason::FetchFailed,
                io_detail(
                    IoErrorClass::ConnectionRefused,
                    DiagnosticOperation::HttpFetch,
                    "failed",
                    None,
                ),
                "2026-07-15T00:00:00Z",
                0,
            )
            .is_err()
    );
}

#[test]
fn source_health_rejects_integration_fault_diagnostics_at_mutation_and_state_load_boundaries() {
    let target = target("integer", "");
    let empty = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        target.contract_digest_sha256().expect("digest"),
    );

    for (integration_fault_code, error_class) in [
        (
            IntegrationFaultCode::HtmlcutInternalError,
            HtmlcutErrorClass::InternalError,
        ),
        (
            IntegrationFaultCode::FfhnBoundaryInvariantViolation,
            HtmlcutErrorClass::FfhnBoundaryInvariantViolation,
        ),
    ] {
        let detail = htmlcut_detail(
            "HTMLCut extraction could not uphold its boundary contract",
            HtmlcutFailureDetails::new(error_class, None, "a".repeat(64), Vec::new()),
            Some(integration_fault_code),
        );
        let invalid_state = mutate_state(&empty, |wire| {
            wire["source_health"] = serde_json::json!({
                "state": "suspect",
                "reason_class": "htmlcut_no_match",
                "consecutive_unresolved": 1,
                "first_unresolved_at": "2026-07-15T00:00:00Z",
                "last_details": serde_json::to_value(&detail).expect("diagnostic JSON"),
            });
        });

        assert!(invalid_state.validate().is_err());
        assert!(
            empty
                .clone()
                .apply_source_suspect(
                    SourceSuspectReason::HtmlcutNoMatch,
                    detail,
                    "2026-07-15T00:00:00Z",
                    3,
                )
                .is_err()
        );
    }
}

#[test]
fn source_health_requires_htmlcut_match_index_evidence_to_agree_with_its_reason() {
    let target = target("integer", "");
    let empty = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        target.contract_digest_sha256().expect("digest"),
    );
    let match_index_detail = htmlcut_detail(
        "HTMLCut could not select the configured candidate index",
        HtmlcutFailureDetails::new(HtmlcutErrorClass::NoMatch, None, "a".repeat(64), Vec::new())
            .with_core_diagnostic_code(HtmlcutDiagnosticCode::MatchIndexOutOfRange),
        None,
    );

    empty
        .clone()
        .apply_source_suspect(
            SourceSuspectReason::HtmlcutMatchIndexOutOfRange,
            match_index_detail,
            "2026-07-15T00:00:00Z",
            3,
        )
        .expect("matching HTMLCut source-health evidence");

    let no_match_detail = htmlcut_detail(
        "HTMLCut found no candidate for the configured selection",
        HtmlcutFailureDetails::new(HtmlcutErrorClass::NoMatch, None, "a".repeat(64), Vec::new()),
        None,
    );
    assert!(
        empty
            .clone()
            .apply_source_suspect(
                SourceSuspectReason::HtmlcutMatchIndexOutOfRange,
                no_match_detail,
                "2026-07-15T00:00:00Z",
                3,
            )
            .is_err()
    );
}

#[test]
fn source_health_accepts_every_closed_source_evidence_family() {
    let target = target("integer", "");
    let empty = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        target.contract_digest_sha256().expect("digest"),
    );
    let htmlcut_detail = |error_class: HtmlcutErrorClass| {
        htmlcut_detail(
            "HTMLCut could not produce a selected value",
            HtmlcutFailureDetails::new(error_class, None, "a".repeat(64), Vec::new()),
            None,
        )
    };
    let cases = [
        (
            SourceSuspectReason::FetchFailed,
            io_detail(
                IoErrorClass::NotFound,
                DiagnosticOperation::FileRead,
                "the source file could not be read",
                None,
            ),
        ),
        (
            SourceSuspectReason::FetchFailed,
            io_detail(
                IoErrorClass::ConnectionRefused,
                DiagnosticOperation::HttpFetch,
                "the HTTP source could not be fetched",
                None,
            ),
        ),
        (
            SourceSuspectReason::JsonMalformed,
            plain_detail(
                DiagnosticKind::Json,
                DiagnosticOperation::JsonPointerSelection,
                "the JSON source is malformed",
                None,
            ),
        ),
        (
            SourceSuspectReason::JsonMissingPointerTarget,
            plain_detail(
                DiagnosticKind::Json,
                DiagnosticOperation::JsonPointerSelection,
                "the JSON Pointer selected no value",
                None,
            ),
        ),
        (
            SourceSuspectReason::JsonNonScalarPointerTarget,
            plain_detail(
                DiagnosticKind::Json,
                DiagnosticOperation::JsonPointerSelection,
                "the JSON Pointer selected a non-scalar value",
                None,
            ),
        ),
        (
            SourceSuspectReason::ValueUnparseable,
            plain_detail(
                DiagnosticKind::ValueUnparseable,
                DiagnosticOperation::ValueParse,
                "the selected value does not match the declared type",
                None,
            ),
        ),
        (
            SourceSuspectReason::HtmlcutNoMatch,
            htmlcut_detail(HtmlcutErrorClass::NoMatch),
        ),
        (
            SourceSuspectReason::HtmlcutAmbiguousMatch,
            htmlcut_detail(HtmlcutErrorClass::AmbiguousMatch),
        ),
        (
            SourceSuspectReason::HtmlcutMissingAttribute,
            htmlcut_detail(HtmlcutErrorClass::MissingAttribute),
        ),
    ];

    for (reason, detail) in cases {
        let mut state = empty.clone();
        state
            .apply_source_suspect(reason, detail, "2026-07-15T00:00:00Z", 3)
            .expect("matching source-health evidence");
        state.validate().expect("persistable source-health state");
    }
}

#[test]
fn source_health_rejects_each_crossed_reason_and_diagnostic_boundary() {
    let target = target("integer", "");
    let empty = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        target.contract_digest_sha256().expect("digest"),
    );
    let htmlcut = |error_class: HtmlcutErrorClass| {
        htmlcut_detail(
            "HTMLCut could not produce a selected value",
            HtmlcutFailureDetails::new(error_class, None, "a".repeat(64), Vec::new()),
            None,
        )
    };
    let cases = [
        (
            SourceSuspectReason::FetchFailed,
            io_detail(
                IoErrorClass::Other,
                DiagnosticOperation::TargetLoad,
                "target loading stopped",
                None,
            ),
        ),
        (
            SourceSuspectReason::JsonMalformed,
            plain_detail(
                DiagnosticKind::Json,
                DiagnosticOperation::ValueParse,
                "selected JSON value cannot parse",
                None,
            ),
        ),
        (
            SourceSuspectReason::ValueUnparseable,
            plain_detail(
                DiagnosticKind::ValueUnparseable,
                DiagnosticOperation::JsonPointerSelection,
                "selected value cannot parse",
                None,
            ),
        ),
        (
            SourceSuspectReason::HtmlcutNoMatch,
            htmlcut(HtmlcutErrorClass::MissingAttribute),
        ),
        (
            SourceSuspectReason::HtmlcutMatchIndexOutOfRange,
            htmlcut(HtmlcutErrorClass::NoMatch),
        ),
    ];

    for (reason, detail) in cases {
        assert!(
            empty
                .clone()
                .apply_source_suspect(reason, detail, "2026-07-15T00:00:00Z", 3)
                .is_err()
        );
    }
}

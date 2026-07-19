use super::support::*;
use crate::{HtmlcutDiagnosticCode, HtmlcutErrorClass, IntegrationFaultCode};

#[test]
fn htmlcut_failure_detail_accessors_preserve_all_failure_evidence() {
    use htmlcut_core::interop::v1::{
        ErrorCode, InteropDiagnostic, InteropDiagnosticCode, InteropDiagnosticLevel, InteropError,
    };

    let failure = HtmlcutFailureDetails::new(
        HtmlcutErrorClass::NoMatch,
        Some(2),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        Vec::new(),
    );
    assert_eq!(failure.error_class(), HtmlcutErrorClass::NoMatch);
    assert_eq!(failure.core_diagnostic_code(), None);
    assert_eq!(failure.candidate_count(), Some(2));
    assert_eq!(
        failure.plan_digest_sha256(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert!(failure.diagnostics().is_empty());
    assert_eq!(failure.boundary_evidence(), None);

    let boundary = HtmlcutFailureDetails::new(
        HtmlcutErrorClass::FfhnBoundaryInvariantViolation,
        Some(2),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        Vec::new(),
    )
    .with_boundary_evidence(HtmlcutBoundaryEvidence::SelectedMatchCount {
        selected_match_count: 0,
    });
    assert_eq!(
        boundary.boundary_evidence(),
        Some(&HtmlcutBoundaryEvidence::SelectedMatchCount {
            selected_match_count: 0,
        })
    );
    assert_eq!(
        serde_json::to_value(&boundary).expect("boundary evidence JSON")["boundary_evidence"],
        serde_json::json!({ "kind": "selected_match_count", "selected_match_count": 0 })
    );
    htmlcut_detail(
        "HTMLCut boundary failure",
        boundary.clone(),
        Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation),
    )
    .validate()
    .expect("selected-match boundary evidence is owned and coherent");

    let requested_attribute = HtmlcutFailureDetails::new(
        HtmlcutErrorClass::MissingAttribute,
        Some(1),
        "a".repeat(64),
        Vec::new(),
    )
    .with_optional_boundary_evidence(Some(HtmlcutBoundaryEvidence::RequestedCssAttribute {
        attribute: "content".to_owned(),
    }));
    htmlcut_detail("HTMLCut missing attribute", requested_attribute, None)
        .validate()
        .expect("requested CSS attribute evidence is owned and coherent");

    let detail = htmlcut_detail("no match", failure, None);
    assert_eq!(
        detail
            .htmlcut_failure()
            .map(HtmlcutFailureDetails::error_class),
        Some(HtmlcutErrorClass::NoMatch)
    );

    let nested_error = InteropError::new(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ErrorCode::NoMatch,
        "no match",
        None,
        std::collections::BTreeMap::from([(
            "core_details".to_owned(),
            serde_json::json!({"candidateCount": 7}),
        )]),
        Vec::new(),
    );
    let nested_failure = HtmlcutFailureDetails::from_interop_error(&nested_error)
        .expect("closed HTMLCut failure evidence");
    assert_eq!(nested_failure.error_class(), HtmlcutErrorClass::NoMatch);
    assert_eq!(nested_failure.candidate_count(), Some(7));

    let no_count_error = InteropError::new(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ErrorCode::NoMatch,
        "no match",
        None,
        std::collections::BTreeMap::from([(
            "unrelated".to_owned(),
            serde_json::json!(["not a count"]),
        )]),
        Vec::new(),
    );
    assert_eq!(
        HtmlcutFailureDetails::from_interop_error(&no_count_error)
            .expect("closed HTMLCut failure evidence")
            .candidate_count(),
        None
    );

    let selector_parse = serde_json::json!({
        "line": 1,
        "column_utf16": 1,
        "parse_error_class": "invalid_attribute_selector",
    });
    let selector_error = InteropError::new(
        "a".repeat(64),
        ErrorCode::PlanInvalid,
        "invalid selector",
        None,
        std::collections::BTreeMap::from([
            (
                "core_diagnostic_code".to_owned(),
                serde_json::json!("INVALID_SELECTOR"),
            ),
            (
                "core_details".to_owned(),
                serde_json::json!({ "selector_parse": selector_parse.clone() }),
            ),
        ]),
        vec![InteropDiagnostic {
            level: InteropDiagnosticLevel::Error,
            code: InteropDiagnosticCode::InvalidSelector,
            message: "selector parse failed".to_owned(),
            details: Some(serde_json::json!({ "selector_parse": selector_parse })),
        }],
    );
    let selector_failure = HtmlcutFailureDetails::from_interop_error(&selector_error)
        .expect("matching published selector parse carriers are retained");
    assert!(selector_failure.selector_parse().is_some());
    htmlcut_detail("invalid selector", selector_failure.clone(), None)
        .validate()
        .expect("matching selector parse evidence is durable");

    let missing_selector = HtmlcutFailureDetails::new(
        HtmlcutErrorClass::PlanInvalid,
        None,
        "a".repeat(64),
        Vec::new(),
    )
    .with_core_diagnostic_code(HtmlcutDiagnosticCode::InvalidSelector);
    assert!(
        htmlcut_detail("invalid selector", missing_selector, None)
            .validate()
            .is_err()
    );

    let mut mismatched_selector =
        serde_json::to_value(&selector_failure).expect("selector failure JSON");
    mismatched_selector["selector_parse"]["line"] = serde_json::json!(2);
    assert!(serde_json::from_value::<HtmlcutFailureDetails>(mismatched_selector).is_err());

    let selector_without_matching_core_carrier = InteropError::new(
        "a".repeat(64),
        ErrorCode::PlanInvalid,
        "invalid selector",
        None,
        std::collections::BTreeMap::from([(
            "core_diagnostic_code".to_owned(),
            serde_json::json!("INVALID_SELECTOR"),
        )]),
        selector_error.diagnostics.clone(),
    );
    assert!(
        HtmlcutFailureDetails::from_interop_error(&selector_without_matching_core_carrier).is_err()
    );

    let mut divergent_selector_core = selector_error.clone();
    divergent_selector_core.details.insert(
        "core_details".to_owned(),
        serde_json::json!({
            "selector_parse": {
                "line": 2,
                "column_utf16": 1,
                "parse_error_class": "invalid_attribute_selector",
            }
        }),
    );
    assert!(HtmlcutFailureDetails::from_interop_error(&divergent_selector_core).is_err());

    let mut malformed_selector_core = selector_error.clone();
    malformed_selector_core.details.insert(
        "core_details".to_owned(),
        serde_json::json!({ "selector_parse": {} }),
    );
    assert!(HtmlcutFailureDetails::from_interop_error(&malformed_selector_core).is_err());

    let failure = HtmlcutFailureDetails::new(
        HtmlcutErrorClass::NoMatch,
        None,
        "not-a-digest".to_owned(),
        Vec::new(),
    );
    assert!(
        htmlcut_detail("HTMLCut failed", failure, None)
            .validate()
            .is_err()
    );

    let misowned_boundary =
        HtmlcutFailureDetails::new(HtmlcutErrorClass::NoMatch, None, "a".repeat(64), Vec::new())
            .with_boundary_evidence(HtmlcutBoundaryEvidence::SelectedMatchCount {
                selected_match_count: 0,
            });
    assert!(
        htmlcut_detail("HTMLCut failed", misowned_boundary, None)
            .validate()
            .is_err()
    );

    for invalid_boundary in [
        HtmlcutFailureDetails::new(
            HtmlcutErrorClass::FfhnBoundaryInvariantViolation,
            None,
            "a".repeat(64),
            Vec::new(),
        )
        .with_boundary_evidence(HtmlcutBoundaryEvidence::SelectedMatchCount {
            selected_match_count: 1,
        }),
        HtmlcutFailureDetails::new(HtmlcutErrorClass::NoMatch, None, "a".repeat(64), Vec::new())
            .with_boundary_evidence(HtmlcutBoundaryEvidence::RequestedCssAttribute {
                attribute: "content".to_owned(),
            }),
        HtmlcutFailureDetails::new(
            HtmlcutErrorClass::MissingAttribute,
            None,
            "a".repeat(64),
            Vec::new(),
        )
        .with_boundary_evidence(HtmlcutBoundaryEvidence::RequestedCssAttribute {
            attribute: String::new(),
        }),
        HtmlcutFailureDetails::new(
            HtmlcutErrorClass::MissingAttribute,
            None,
            "a".repeat(64),
            Vec::new(),
        )
        .with_boundary_evidence(HtmlcutBoundaryEvidence::RequestedCssAttribute {
            attribute: "x".repeat(1_025),
        }),
    ] {
        assert!(
            htmlcut_detail("HTMLCut failed", invalid_boundary, None)
                .validate()
                .is_err()
        );
    }
}

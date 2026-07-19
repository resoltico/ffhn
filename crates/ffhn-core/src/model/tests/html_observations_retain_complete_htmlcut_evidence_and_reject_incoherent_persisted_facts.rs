use super::support::*;

#[test]
fn html_observations_retain_complete_htmlcut_evidence_and_reject_incoherent_persisted_facts() {
    use htmlcut_core::interop::v1::{
        InteropDiagnostic, InteropDiagnosticCode, InteropDiagnosticLevel,
    };

    let diagnostics = [
        (InteropDiagnosticLevel::Error, "error"),
        (InteropDiagnosticLevel::Warning, "warning"),
        (InteropDiagnosticLevel::Info, "info"),
    ]
    .into_iter()
    .map(|(level, expected_level)| {
        let diagnostic = HtmlcutDiagnostic::from_interop(InteropDiagnostic {
            level,
            code: InteropDiagnosticCode::MultipleMatches,
            message: format!("{expected_level} diagnostic"),
            details: Some(serde_json::json!({"candidateCount": 2, "selectedIndex": 1})),
        })
        .expect("pinned HTMLCut diagnostic projection");
        assert_eq!(diagnostic.level(), expected_level);
        assert_eq!(diagnostic.code(), "MULTIPLE_MATCHES");
        assert_eq!(diagnostic.message(), format!("{expected_level} diagnostic"));
        assert_eq!(
            serde_json::to_value(diagnostic.details()).expect("closed detail JSON"),
            serde_json::json!({
                "kind": "candidate_selection",
                "candidate_count": 2,
                "selected_index": 1,
            })
        );
        diagnostic
    })
    .collect::<Vec<_>>();

    let html_target_document = html_target("html_text", "article", None, "decimal", "");
    let observation = html_target_document
        .parse_html_projection(HtmlObservationInput {
            raw_selected: " 1.00 ".to_owned(),
            comparison_projection: "1.00".to_owned(),
            acquisition_kind: AcquisitionKind::HtmlPlainText,
            plan_digest_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
            candidate_count: 2,
            diagnostics: diagnostics.clone(),
        })
        .expect("HTML projection parses through the target type contract");
    assert_eq!(observation.raw_selected(), " 1.00 ");
    assert_eq!(observation.comparison_projection(), "1.00");
    assert_eq!(
        observation.acquisition_kind(),
        AcquisitionKind::HtmlPlainText
    );
    assert_eq!(observation.canonical_value(), "1");
    assert_eq!(
        observation.htmlcut_semantics_version(),
        Some(htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION)
    );
    assert_eq!(
        observation.plan_digest_sha256(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert_eq!(observation.htmlcut_candidate_count(), Some(2));
    assert_eq!(observation.htmlcut_diagnostics(), diagnostics.as_slice());
    observation.validate().expect("coherent HTML observation");

    assert_eq!(
        html_target_document
            .parse_html_projection(HtmlObservationInput {
                raw_selected: "not a decimal".to_owned(),
                comparison_projection: "not a decimal".to_owned(),
                acquisition_kind: AcquisitionKind::HtmlPlainText,
                plan_digest_sha256:
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                candidate_count: 1,
                diagnostics: Vec::new(),
            })
            .expect_err("HTML type failures are value-unparseable")
            .kind(),
        DiagnosticKind::ValueUnparseable
    );

    for mutate in [
        |wire: &mut serde_json::Value| {
            wire["acquisition_kind"] = serde_json::json!("json_pointer");
            wire["raw_selected"] = serde_json::json!("1.00");
        },
        |wire: &mut serde_json::Value| {
            wire["htmlcut_semantics_version"] = serde_json::json!(999);
        },
        |wire: &mut serde_json::Value| {
            wire["plan_digest_sha256"] = serde_json::json!("not-a-digest");
        },
        |wire: &mut serde_json::Value| {
            wire["htmlcut_candidate_count"] = serde_json::json!(0);
        },
    ] {
        let mut wire = serde_json::to_value(&observation).expect("observation JSON");
        mutate(&mut wire);
        let invalid: Observation = serde_json::from_value(wire).expect("structurally valid JSON");
        assert!(invalid.validate().is_err());
    }

    let json_observation = target("integer", "")
        .parse_json_scalar_token("1".to_owned())
        .expect("JSON observation");
    for (field, value) in [
        ("htmlcut_semantics_version", serde_json::json!(1)),
        (
            "plan_digest_sha256",
            serde_json::json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        ),
        ("htmlcut_candidate_count", serde_json::json!(1)),
        (
            "htmlcut_diagnostics",
            serde_json::json!([{
                "level": "info",
                "code": "MULTIPLE_MATCHES",
                "message": "unexpected JSON evidence"
            }]),
        ),
    ] {
        let mut wire = serde_json::to_value(&json_observation).expect("observation JSON");
        wire[field] = value;
        let invalid: Observation = serde_json::from_value(wire).expect("structurally valid JSON");
        assert!(invalid.validate().is_err(), "{field}");
    }
}

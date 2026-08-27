use super::*;

#[test]
fn persisted_observation_validation_covers_json_and_html_evidence_coherence() {
    let json = observation(DeclaredType::Integer, &TypeParams::default(), "7").expect("JSON");
    assert_eq!(json.raw_selected(), "7");
    assert_eq!(json.comparison_projection(), "7");
    assert_eq!(json.acquisition_kind(), AcquisitionKind::JsonPointer);
    assert_eq!(json.htmlcut_semantics_version(), None);
    assert_eq!(json.plan_digest_sha256(), None);
    assert_eq!(json.htmlcut_candidate_count(), None);
    assert!(json.htmlcut_diagnostics().is_empty());
    assert!(json.matches_type_contract(DeclaredType::Integer, &TypeParams::default()));
    assert!(!json.matches_type_contract(DeclaredType::Text, &TypeParams::default()));
    assert!(!json.matches_type_contract(
        DeclaredType::Integer,
        &TypeParams {
            locale: Some(NumericLocale::Invariant),
            ..TypeParams::default()
        }
    ));
    let mutations: [fn(&mut serde_json::Value); 9] = [
        |wire: &mut serde_json::Value| wire["parser_id"] = serde_json::json!("future"),
        |wire: &mut serde_json::Value| wire["parser_grammar_version"] = serde_json::json!(99),
        |wire: &mut serde_json::Value| wire["parse_diagnostics"] = serde_json::json!(["invented"]),
        |wire: &mut serde_json::Value| wire["comparison_projection"] = serde_json::json!("8"),
        |wire: &mut serde_json::Value| {
            wire["plan_digest_sha256"] = serde_json::json!("a".repeat(64))
        },
        |wire: &mut serde_json::Value| wire["htmlcut_semantics_version"] = serde_json::json!(4),
        |wire: &mut serde_json::Value| wire["htmlcut_candidate_count"] = serde_json::json!(1),
        |wire: &mut serde_json::Value| {
            wire["htmlcut_diagnostics"] = serde_json::json!([{
                "level": "warning",
                "code": "SOURCE_LOAD_FAILED",
                "message": "diagnostic"
            }])
        },
        |wire: &mut serde_json::Value| wire["canonical_value"] = serde_json::json!("8"),
    ];
    for mutate in mutations {
        let mut wire = serde_json::to_value(&json).expect("JSON wire");
        mutate(&mut wire);
        let invalid: Observation = serde_json::from_value(wire).expect("structural observation");
        assert!(invalid.validate().is_err());
    }
    let mut invalid_token = serde_json::to_value(&json).expect("JSON wire");
    invalid_token["raw_selected"] = serde_json::json!("not-json");
    invalid_token["comparison_projection"] = serde_json::json!("not-json");
    let invalid: Observation = serde_json::from_value(invalid_token).expect("structural JSON");
    assert!(invalid.validate().is_err());

    for acquisition_kind in [
        AcquisitionKind::HtmlPlainText,
        AcquisitionKind::HtmlRenderedText,
        AcquisitionKind::HtmlAttribute,
    ] {
        let html = parse_html_projection_for_contract(
            DeclaredType::Text,
            &TypeParams::default(),
            HtmlObservationInput {
                raw_selected: "raw".to_owned(),
                comparison_projection: "comparison".to_owned(),
                acquisition_kind,
                plan_digest_sha256: "a".repeat(64),
                candidate_count: 2,
                diagnostics: Vec::new(),
            },
        )
        .expect("HTML observation");
        html.validate().expect("valid HTML evidence");
        assert_eq!(html.raw_selected(), "raw");
        assert_eq!(html.comparison_projection(), "comparison");
        assert_eq!(html.acquisition_kind(), acquisition_kind);
        assert!(html.htmlcut_semantics_version().is_some());
        let digest = "a".repeat(64);
        assert_eq!(html.plan_digest_sha256(), Some(digest.as_str()));
        assert_eq!(html.htmlcut_candidate_count(), Some(2));

        for (field, value) in [
            ("htmlcut_semantics_version", serde_json::json!(99)),
            ("plan_digest_sha256", serde_json::json!("invalid")),
            ("htmlcut_candidate_count", serde_json::json!(0)),
            ("canonical_value", serde_json::json!("different")),
        ] {
            let mut wire = serde_json::to_value(&html).expect("HTML wire");
            wire[field] = value;
            let invalid: Observation =
                serde_json::from_value(wire).expect("structural HTML observation");
            assert!(invalid.validate().is_err());
        }
        for field in [
            "htmlcut_semantics_version",
            "plan_digest_sha256",
            "htmlcut_candidate_count",
        ] {
            let mut wire = serde_json::to_value(&html).expect("HTML wire");
            wire.as_object_mut().expect("object").remove(field);
            let invalid: Observation =
                serde_json::from_value(wire).expect("structural HTML observation");
            assert!(invalid.validate().is_err(), "{field}");
        }

        let mut invalid_diagnostic = serde_json::to_value(&html).expect("HTML wire");
        invalid_diagnostic["htmlcut_diagnostics"] = serde_json::json!([{
            "level": "warning",
            "code": "MULTIPLE_MATCHES",
            "message": "diagnostic",
            "details": {
                "kind": "candidate_selection",
                "candidate_count": 1,
                "selected_index": 1
            }
        }]);
        let invalid: Observation =
            serde_json::from_value(invalid_diagnostic).expect("structural diagnostic");
        assert!(invalid.validate().is_err());

        let mut unparseable = serde_json::to_value(&html).expect("HTML wire");
        unparseable["declared_type"] = serde_json::json!("integer");
        unparseable["canonical_value"] = serde_json::json!("0");
        let invalid: Observation =
            serde_json::from_value(unparseable).expect("structural unparseable observation");
        assert!(invalid.validate().is_err());
    }

    let invalid_html = parse_html_projection_for_contract(
        DeclaredType::Integer,
        &TypeParams::default(),
        HtmlObservationInput {
            raw_selected: "text".to_owned(),
            comparison_projection: "text".to_owned(),
            acquisition_kind: AcquisitionKind::HtmlPlainText,
            plan_digest_sha256: "a".repeat(64),
            candidate_count: 1,
            diagnostics: Vec::new(),
        },
    );
    assert!(invalid_html.is_err());

    let diagnostic: crate::HtmlcutDiagnostic = serde_json::from_value(serde_json::json!({
        "level": "warning",
        "code": "MULTIPLE_MATCHES",
        "message": "selected first",
        "details": {
            "kind": "candidate_selection",
            "candidate_count": 2,
            "selected_index": 1
        }
    }))
    .expect("diagnostic");
    let money_params = TypeParams {
        currency: Some("USD".to_owned()),
        ..TypeParams::default()
    };
    let money = parse_html_projection_for_contract(
        DeclaredType::Money,
        &money_params,
        HtmlObservationInput {
            raw_selected: "1.00".to_owned(),
            comparison_projection: "1.00".to_owned(),
            acquisition_kind: AcquisitionKind::HtmlPlainText,
            plan_digest_sha256: "b".repeat(64),
            candidate_count: 2,
            diagnostics: vec![diagnostic.clone()],
        },
    )
    .expect("money HTML observation");
    assert_eq!(money.htmlcut_diagnostics(), [diagnostic]);
    assert_eq!(money.declared_type_for_policy(), DeclaredType::Money);
    assert_eq!(money.type_params_for_policy(), &money_params);
}

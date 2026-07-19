use super::support::*;

#[test]
fn target_contracts_bind_policy_semantics_and_htmlcut_semantics_only_for_html() {
    #[derive(serde::Serialize)]
    struct JsonContract<'a> {
        source_kind: &'static str,
        target: &'a TargetSource,
        fetch: &'a FetchConfig,
        projection: &'a Projection,
        declared_type: DeclaredType,
        parser_id: &'static str,
        parser_grammar_version: u32,
        type_params: &'a TypeParams,
        conditions: &'a [Condition],
        escalate_after: u32,
        policy_evaluation_semantics_version: u32,
    }
    #[derive(serde::Serialize)]
    struct HtmlContract<'a> {
        source_kind: &'static str,
        target: &'a TargetSource,
        fetch: &'a FetchConfig,
        projection: &'a Projection,
        declared_type: DeclaredType,
        parser_id: &'static str,
        parser_grammar_version: u32,
        type_params: &'a TypeParams,
        conditions: &'a [Condition],
        escalate_after: u32,
        policy_evaluation_semantics_version: u32,
        htmlcut_extraction_semantics_version: u32,
    }

    let json = target("integer", "");
    let json_expected = crate::stable_json::stable_digest(&JsonContract {
        source_kind: "json_pointer",
        target: json.source(),
        fetch: json.fetch(),
        projection: json.projection(),
        declared_type: json.declared_type(),
        parser_id: PARSER_ID,
        parser_grammar_version: PARSER_GRAMMAR_VERSION,
        type_params: json.type_params(),
        conditions: json.conditions(),
        escalate_after: json.escalate_after(),
        policy_evaluation_semantics_version: POLICY_EVALUATION_SEMANTICS_VERSION,
    })
    .expect("JSON expected digest");
    assert_eq!(
        json.contract_digest_sha256().expect("JSON digest"),
        json_expected
    );

    let html = html_target(
        "html_attribute",
        "meta#price",
        Some("content"),
        "decimal",
        "",
    );
    let html_expected = crate::stable_json::stable_digest(&HtmlContract {
        source_kind: "html_attribute",
        target: html.source(),
        fetch: html.fetch(),
        projection: html.projection(),
        declared_type: html.declared_type(),
        parser_id: PARSER_ID,
        parser_grammar_version: PARSER_GRAMMAR_VERSION,
        type_params: html.type_params(),
        conditions: html.conditions(),
        escalate_after: html.escalate_after(),
        policy_evaluation_semantics_version: POLICY_EVALUATION_SEMANTICS_VERSION,
        htmlcut_extraction_semantics_version:
            htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION,
    })
    .expect("HTML expected digest");
    assert_eq!(
        html.contract_digest_sha256().expect("HTML digest"),
        html_expected
    );

    let changed_policy_semantics = crate::stable_json::stable_digest(&JsonContract {
        source_kind: "json_pointer",
        target: json.source(),
        fetch: json.fetch(),
        projection: json.projection(),
        declared_type: json.declared_type(),
        parser_id: PARSER_ID,
        parser_grammar_version: PARSER_GRAMMAR_VERSION,
        type_params: json.type_params(),
        conditions: json.conditions(),
        escalate_after: json.escalate_after(),
        policy_evaluation_semantics_version: POLICY_EVALUATION_SEMANTICS_VERSION + 1,
    })
    .expect("changed policy semantics digest");
    assert_ne!(
        json.contract_digest_sha256().expect("JSON digest"),
        changed_policy_semantics
    );

    let changed_html_policy_semantics = crate::stable_json::stable_digest(&HtmlContract {
        source_kind: "html_attribute",
        target: html.source(),
        fetch: html.fetch(),
        projection: html.projection(),
        declared_type: html.declared_type(),
        parser_id: PARSER_ID,
        parser_grammar_version: PARSER_GRAMMAR_VERSION,
        type_params: html.type_params(),
        conditions: html.conditions(),
        escalate_after: html.escalate_after(),
        policy_evaluation_semantics_version: POLICY_EVALUATION_SEMANTICS_VERSION + 1,
        htmlcut_extraction_semantics_version:
            htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION,
    })
    .expect("changed HTML policy semantics digest");
    assert_ne!(
        html.contract_digest_sha256().expect("HTML digest"),
        changed_html_policy_semantics
    );

    let changed_htmlcut_semantics = crate::stable_json::stable_digest(&HtmlContract {
        source_kind: "html_attribute",
        target: html.source(),
        fetch: html.fetch(),
        projection: html.projection(),
        declared_type: html.declared_type(),
        parser_id: PARSER_ID,
        parser_grammar_version: PARSER_GRAMMAR_VERSION,
        type_params: html.type_params(),
        conditions: html.conditions(),
        escalate_after: html.escalate_after(),
        policy_evaluation_semantics_version: POLICY_EVALUATION_SEMANTICS_VERSION,
        htmlcut_extraction_semantics_version:
            htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION + 1,
    })
    .expect("incorrect counter digest");
    assert_ne!(
        html.contract_digest_sha256().expect("HTML digest"),
        changed_htmlcut_semantics
    );

    let mut wire = serde_json::to_value(&html).expect("HTML target JSON");
    wire["projection"]["output"] = serde_json::json!({"kind": "structured"});
    assert!(serde_json::from_value::<TargetDocument>(wire).is_err());
}

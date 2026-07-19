use super::support::*;

#[test]
fn policy_semantics_changes_reset_every_projection_while_htmlcut_is_html_only() {
    let (_temporary, json_paths) = fixture_paths();
    write_target(&json_paths, "integer", "", "/value");
    fs::write(
        json_paths.target_dir().join("source.json"),
        r#"{"value":7}"#,
    )
    .expect("JSON source");
    assert_policy_semantics_change_requires_reset(&json_paths);

    let json_target = load_target(&json_paths).expect("JSON target");
    let current_json_digest = json_target.contract_digest_sha256().expect("JSON digest");
    let htmlcut_only_json_digest = json_target
        .contract_digest_sha256_with_semantics_versions_for_test(
            crate::model::POLICY_EVALUATION_SEMANTICS_VERSION,
            htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION + 1,
        )
        .expect("JSON digest with changed HTMLCut semantics");
    assert_eq!(current_json_digest, htmlcut_only_json_digest);
    replace_state_contract_digest(&json_paths, htmlcut_only_json_digest);
    assert_eq!(
        run_once(&json_paths)
            .expect("JSON run after HTMLCut-only change")
            .outcome(),
        RunOutcome::Unchanged
    );

    let (_temporary, html_text_paths) = fixture_paths();
    write_html_target(&html_text_paths, "html_text", ".price", None, "integer", "");
    fs::write(
        html_text_paths.target_dir().join("source.html"),
        "<span class=\"price\">7</span>",
    )
    .expect("HTML text source");
    assert_policy_semantics_change_requires_reset(&html_text_paths);

    let (_temporary, html_attribute_paths) = fixture_paths();
    write_html_target(
        &html_attribute_paths,
        "html_attribute",
        "meta#price",
        Some("content"),
        "integer",
        "",
    );
    fs::write(
        html_attribute_paths.target_dir().join("source.html"),
        "<meta id=\"price\" content=\"7\">",
    )
    .expect("HTML attribute source");
    assert_policy_semantics_change_requires_reset(&html_attribute_paths);
}

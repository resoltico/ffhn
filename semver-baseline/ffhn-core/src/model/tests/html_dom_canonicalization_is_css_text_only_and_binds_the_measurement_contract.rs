use super::support::*;

#[test]
fn html_dom_canonicalization_is_css_text_only_and_binds_the_measurement_contract() {
    let html_text = html_target("html_text", "article.measurement a", None, "integer", "");
    let canonicalized = mutate_target(&html_text, |wire| {
        wire["projection"]["selection"]["dom_canonicalization"] = serde_json::json!({
            "ignore_attributes": ["href"],
            "strip_whitespace_nodes": true
        });
    });
    canonicalized
        .validate()
        .expect("CSS HTML text accepts DOM canonicalization");
    assert_ne!(
        canonicalized
            .contract_digest_sha256()
            .expect("canonicalized digest"),
        html_text.contract_digest_sha256().expect("plain digest")
    );

    let Projection::HtmlText { selection } = canonicalized.projection() else {
        panic!("fixture must remain an HTML text projection");
    };
    let policy = selection
        .dom_canonicalization()
        .expect("canonicalization policy is retained");
    assert_eq!(policy.ignore_attributes.len(), 1);
    assert!(policy.strip_whitespace_nodes);
    assert_eq!(
        selection
            .structured_plan()
            .dom_canonicalization
            .as_ref()
            .expect("HTMLCut plan retains policy"),
        policy
    );

    let delimiter = mutate_target(&canonicalized, |wire| {
        wire["projection"]["selection"]["strategy"] = serde_json::json!({
            "kind": "delimiter_pair",
            "start": "<article>",
            "end": "</article>",
            "mode": "literal",
            "boundary_retention": "exclude_both"
        });
    });
    let delimiter_error = delimiter
        .permanent_error()
        .expect("HTMLCut evidence projection")
        .expect("non-CSS canonicalization must be permanently invalid");
    assert_eq!(
        delimiter_error.code(),
        PermanentErrorCode::HtmlcutPlanInvalid
    );
    assert!(delimiter.validate().is_err());

    let plain_delimiter = mutate_target(&html_text, |wire| {
        wire["projection"]["selection"]["strategy"] = serde_json::json!({
            "kind": "delimiter_pair",
            "start": "<article>",
            "end": "</article>",
            "mode": "literal",
            "boundary_retention": "exclude_both"
        });
    });
    let plain_delimiter_error = plain_delimiter
        .permanent_error()
        .expect("plain-text projection validation")
        .expect("plain DOM text cannot use a delimiter fragment");
    assert_eq!(
        plain_delimiter_error.code(),
        PermanentErrorCode::HtmlTextRequiresCssSelector
    );
    assert!(plain_delimiter.validate().is_err());

    let rendered_text = html_target(
        "html_rendered_text",
        "article.measurement a",
        None,
        "integer",
        "",
    );
    let rendered_delimiter = mutate_target(&rendered_text, |wire| {
        wire["projection"]["selection"]["strategy"] = serde_json::json!({
            "kind": "delimiter_pair",
            "start": "<article>",
            "end": "</article>",
            "mode": "literal",
            "boundary_retention": "exclude_both"
        });
    });
    rendered_delimiter
        .validate()
        .expect("rendered text supports a delimiter fragment");

    let html_attribute = html_target(
        "html_attribute",
        "meta#price",
        Some("content"),
        "decimal",
        "",
    );
    let canonicalized_attribute = mutate_target(&html_attribute, |wire| {
        wire["projection"]["selection"]["dom_canonicalization"] = serde_json::json!({
            "ignore_attributes": ["content"],
            "strip_whitespace_nodes": false
        });
    });
    let attribute_error = canonicalized_attribute
        .permanent_error()
        .expect("HTMLCut evidence projection")
        .expect("attribute canonicalization must be permanently invalid");
    assert_eq!(
        attribute_error.code(),
        PermanentErrorCode::HtmlcutPlanInvalid
    );
    assert!(canonicalized_attribute.validate().is_err());

    for removed_control in ["sort_attributes", "strip_comments"] {
        let mut wire = serde_json::to_value(&html_text).expect("HTML target JSON");
        wire["projection"]["selection"]["dom_canonicalization"] = serde_json::json!({
            "ignore_attributes": [],
            "strip_whitespace_nodes": false,
            removed_control: true
        });
        assert!(
            serde_json::from_value::<TargetDocument>(wire).is_err(),
            "removed {removed_control} control must be rejected rather than ignored"
        );
    }
}

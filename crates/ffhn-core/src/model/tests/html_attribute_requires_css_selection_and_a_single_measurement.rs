use super::support::*;

#[test]
fn html_attribute_requires_css_selection_and_a_single_measurement() {
    let css_attribute = html_target(
        "html_attribute",
        "time#published",
        Some("datetime"),
        "datetime",
        "[type_params]\nformat = \"rfc3339\"\n",
    );
    assert!(matches!(
        css_attribute.projection(),
        Projection::HtmlAttribute { .. }
    ));

    let delimiter_attribute = mutate_target(&css_attribute, |wire| {
        wire["projection"]["selection"]["strategy"] = serde_json::json!({
            "kind": "delimiter_pair",
            "start": "<time>",
            "end": "</time>",
            "mode": "literal",
            "boundary_retention": "exclude_both"
        });
    });
    assert_eq!(
        delimiter_attribute
            .permanent_error()
            .expect("HTMLCut evidence projection")
            .map(|error| error.code()),
        Some(PermanentErrorCode::HtmlAttributeRequiresCssSelector)
    );
    assert!(delimiter_attribute.validate().is_err());

    let every_match = mutate_target(&css_attribute, |wire| {
        wire["projection"]["selection"]["selection"] = serde_json::json!({"mode": "all"});
    });
    assert_eq!(
        every_match
            .permanent_error()
            .expect("HTMLCut evidence projection")
            .map(|error| error.code()),
        Some(PermanentErrorCode::HtmlSelectionMustSelectOne)
    );
    assert!(every_match.validate().is_err());
}

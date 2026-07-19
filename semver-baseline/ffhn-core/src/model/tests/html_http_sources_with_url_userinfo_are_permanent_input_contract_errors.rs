use super::support::*;

#[test]
fn html_http_sources_with_url_userinfo_are_permanent_input_contract_errors() {
    let html = html_target("html_text", "article", None, "integer", "");
    let configured_http = |source_url: &str| {
        mutate_target(&html, |wire| {
            wire["target"] = serde_json::json!({
                "kind": "http",
                "source_url": source_url
            });
            wire["fetch"] = serde_json::json!({
                "engine": "http",
                "method": "GET",
                "timeout_ms": 1000,
                "max_bytes": 1024,
                "user_agent": "ffhn-test",
                "follow_redirects": false,
                "accept": "text/html",
                "headers": {}
            });
        })
    };
    let credential_free = configured_http("https://example.test/article");
    assert!(
        credential_free
            .permanent_error()
            .expect("HTMLCut evidence projection")
            .is_none()
    );

    let invalid = mutate_target(&html, |wire| {
        wire["target"] = serde_json::json!({
            "kind": "http",
            "source_url": "https://reader:secret@example.test/article"
        });
        wire["fetch"] = serde_json::json!({
            "engine": "http",
            "method": "GET",
            "timeout_ms": 1000,
            "max_bytes": 1024,
            "user_agent": "ffhn-test",
            "follow_redirects": false,
            "accept": "text/html",
            "headers": {}
        });
    });

    assert_eq!(
        invalid
            .permanent_error()
            .expect("HTMLCut evidence projection")
            .map(|error| error.code()),
        Some(PermanentErrorCode::HtmlcutInputInvalid)
    );
    assert!(invalid.validate_without_projection().is_ok());
    assert!(invalid.validate().is_err());

    let password_only = configured_http("https://:secret@example.test/article");
    assert_eq!(
        password_only
            .permanent_error()
            .expect("HTMLCut evidence projection")
            .map(|error| error.code()),
        Some(PermanentErrorCode::HtmlcutInputInvalid)
    );
}

use super::support::*;

#[test]
fn target_validation_covers_source_fetch_pointer_and_type_parameter_boundaries() {
    let valid_file = target("decimal", "");
    assert_eq!(valid_file.target_id(), "demo");
    assert_eq!(valid_file.display_name(), "Demo");
    assert!(valid_file.enabled());
    assert_eq!(valid_file.escalate_after(), 3);
    assert!(matches!(valid_file.source(), TargetSource::File { .. }));
    assert_eq!(valid_file.fetch().engine(), FetchEngine::File);
    assert!(matches!(
        valid_file.projection(),
        Projection::JsonPointer { .. }
    ));
    assert_eq!(valid_file.declared_type(), DeclaredType::Decimal);
    assert_eq!(valid_file.type_params(), &TypeParams::default());
    assert_eq!(
        valid_file.contract_digest_sha256().expect("digest").len(),
        64
    );

    let mut invalid = mutate_target(&valid_file, |wire| {
        wire["schema_version"] = serde_json::json!(8);
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["schema_name"] = serde_json::json!("other");
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["display_name"] = serde_json::json!(" ");
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["escalate_after"] = serde_json::json!(0);
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["target"] = serde_json::json!({"kind": "file", "file_path": "relative.json"});
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["projection"] = serde_json::json!({"kind": "json_pointer", "pointer": "value"});
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["projection"] =
            serde_json::json!({"kind": "json_pointer", "pointer": "/broken~2escape"});
    });
    assert!(invalid.validate().is_err());
    assert!(validate_json_pointer("/~0/~1").is_ok());
    invalid = mutate_target(&valid_file, |wire| {
        wire["fetch"] = serde_json::json!({"engine": "file", "max_bytes": 1});
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["type_params"] = serde_json::json!({"currency": "USD"});
    });
    assert!(invalid.validate().is_err());

    let http = http_target();
    assert_eq!(http.fetch().engine(), FetchEngine::Http);
    let mut invalid_http = mutate_target(&http, |wire| {
        wire["target"] =
            serde_json::json!({"kind": "http", "source_url": "ftp://example.test/value"});
    });
    assert!(invalid_http.validate().is_err());
    invalid_http = mutate_target(&http, |wire| {
        wire["fetch"] = serde_json::json!({"engine": "file", "max_bytes": 1_024});
    });
    assert!(invalid_http.validate().is_err());
    invalid_http = mutate_target(&http, |wire| {
        wire["fetch"]["timeout_ms"] = serde_json::json!(999);
    });
    assert!(invalid_http.validate().is_err());
    invalid_http = mutate_target(&http, |wire| {
        wire["fetch"]["user_agent"] = serde_json::json!(" ");
        wire["fetch"]["accept"] = serde_json::json!(" ");
        wire["fetch"]["headers"] = serde_json::json!({" ": " "});
    });
    assert!(invalid_http.validate().is_err());

    let invalid_money = mutate_target(&valid_file, |wire| {
        wire["declared_type"] = serde_json::json!("money");
        wire["type_params"] = serde_json::json!({"currency": "US"});
    });
    assert!(invalid_money.validate().is_err());
    assert!(
        target("datetime", "[type_params]\nformat = \"rfc3339\"\n")
            .clone()
            .validate()
            .is_ok()
    );
    assert!(
        validate_type_params(
            DeclaredType::Datetime,
            &TypeParams {
                format: Some("[bad".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Datetime,
            &TypeParams {
                format: Some("rfc3339".to_owned()),
                assumed_offset: Some("+02:00".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Datetime,
            &TypeParams {
                format: Some("[year]".to_owned()),
                assumed_offset: Some("not-an-offset".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
}

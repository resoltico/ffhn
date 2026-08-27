use super::parse::{
    JsonAcquisitionFailure, JsonScalarError, decode_json_pointer_token, json_string_value,
    normalize_decimal_input, normalize_grouped_decimal, normalized_raw_token, parse_datetime,
    parse_html_projection_for_contract, parse_json_array_index,
    parse_json_scalar_token_for_contract, parse_offset, select_json_pointer_child,
    select_json_scalar_token, validated_scalar_selection,
};
use super::{AcquisitionKind, HtmlObservationInput, Observation};
use crate::{DeclaredType, NumericLocale, TypeParams};

fn observation(
    declared_type: DeclaredType,
    type_params: &TypeParams,
    raw: &str,
) -> Result<super::Observation, String> {
    parse_json_scalar_token_for_contract(declared_type, type_params, raw.to_owned())
}

#[test]
fn validated_scalar_selection_classifies_internal_invalid_and_non_scalar_tokens() {
    assert!(matches!(
        validated_scalar_selection(" 1".to_owned()),
        Err(JsonAcquisitionFailure::Malformed)
    ));
    assert!(matches!(
        validated_scalar_selection("[]".to_owned()),
        Err(JsonAcquisitionFailure::NonScalarPointerTarget)
    ));
    assert_eq!(
        JsonScalarError::NonScalar.message(),
        "projection.pointer must select a scalar JSON leaf"
    );
    let invalid = JsonScalarError::Invalid("owned".to_owned());
    assert_eq!(invalid.message(), "owned");
    assert!(crate::model::validate::is_sha256(&"a".repeat(64)));
    assert!(crate::model::validate::is_sha256(&"0".repeat(64)));
    assert!(!crate::model::validate::is_sha256(&"A".repeat(64)));
    assert!(!crate::model::validate::is_sha256(&"a".repeat(63)));
    assert!(crate::model::require_canonical_utc_rfc3339("time", "2026-08-25T00:00:00Z").is_ok());
    assert!(
        crate::model::require_canonical_utc_rfc3339("time", "2026-08-25T00:00:00+00:00").is_err()
    );
    assert!(
        crate::model::require_canonical_utc_rfc3339("time", "2026-08-25T01:00:00+01:00").is_err()
    );
}

#[test]
fn json_pointer_helpers_cover_root_object_array_escape_and_missing_boundaries() {
    assert_eq!(normalized_raw_token(" 42 ").expect("normalized"), "42");
    assert!(normalized_raw_token(" ").is_err());
    assert!(normalized_raw_token("not-json").is_err());

    assert_eq!(
        decode_json_pointer_token("a~1b~0c").expect("escape"),
        "a/b~c"
    );
    assert!(decode_json_pointer_token("broken~2").is_err());
    assert_eq!(parse_json_array_index("0"), Some(0));
    assert_eq!(parse_json_array_index("01"), None);
    assert_eq!(parse_json_array_index("2"), Some(2));
    assert_eq!(parse_json_array_index("no"), None);

    assert_eq!(
        select_json_scalar_token(r#"{"a/b":[false,42],"~":7}"#, "/a~1b/1").expect("array child"),
        "42"
    );
    assert_eq!(
        select_json_scalar_token(r#"{"a/b":[false,42],"~":7}"#, "/~0").expect("object child"),
        "7"
    );
    assert!(matches!(
        select_json_scalar_token("not-json", ""),
        Err(JsonAcquisitionFailure::Malformed)
    ));
    assert!(matches!(
        select_json_scalar_token(r#"{"a":1}"#, "/missing"),
        Err(JsonAcquisitionFailure::MissingPointerTarget)
    ));
    assert!(matches!(
        select_json_scalar_token("1", "/child"),
        Err(JsonAcquisitionFailure::MissingPointerTarget)
    ));
    assert!(matches!(
        select_json_scalar_token("[]", ""),
        Err(JsonAcquisitionFailure::NonScalarPointerTarget)
    ));
    assert!(select_json_pointer_child("[1]", "broken").is_err());
    assert!(select_json_pointer_child("1", "0").is_err());
}

#[test]
fn text_json_values_require_an_unpadded_string_scalar() {
    assert!(matches!(
        json_string_value(" \"text\""),
        Err(JsonScalarError::Invalid(_))
    ));
    assert!(matches!(
        json_string_value("42"),
        Err(JsonScalarError::Invalid(_))
    ));
    assert!(matches!(
        json_string_value("{}"),
        Err(JsonScalarError::NonScalar)
    ));
}

#[test]
fn typed_parser_covers_every_value_family_and_owned_failure_message() {
    assert_eq!(
        observation(DeclaredType::Text, &TypeParams::default(), r#""In Stock""#)
            .expect("text")
            .canonical_value(),
        "In Stock"
    );
    assert_eq!(
        observation(
            DeclaredType::Integer,
            &TypeParams::default(),
            &i128::MAX.to_string(),
        )
        .expect("integer")
        .canonical_value(),
        i128::MAX.to_string()
    );
    assert_eq!(
        observation(DeclaredType::Decimal, &TypeParams::default(), "1.00")
            .expect("decimal")
            .canonical_value(),
        "1"
    );
    assert_eq!(
        observation(
            DeclaredType::Money,
            &TypeParams {
                currency: Some("USD".to_owned()),
                locale: Some(NumericLocale::EnUs),
                ..TypeParams::default()
            },
            r#""1,234.50""#,
        )
        .expect("money")
        .canonical_value(),
        "1234.5"
    );
    assert_eq!(
        observation(
            DeclaredType::Semver,
            &TypeParams::default(),
            r#""1.2.3+build.7""#,
        )
        .expect("semver")
        .canonical_value(),
        "1.2.3+build.7"
    );
    assert_eq!(
        observation(
            DeclaredType::Datetime,
            &TypeParams {
                format: Some("rfc3339".to_owned()),
                ..TypeParams::default()
            },
            r#""2026-07-19T12:00:00+02:00""#,
        )
        .expect("datetime")
        .canonical_value(),
        "2026-07-19T10:00:00Z"
    );
    for (kind, params, raw, message) in [
        (
            DeclaredType::Integer,
            TypeParams::default(),
            r#""not-an-integer""#,
            "integer value is not a valid signed 128-bit integer",
        ),
        (
            DeclaredType::Decimal,
            TypeParams::default(),
            "999999999999999999999999999999999999999",
            "decimal value is invalid or out of the supported range",
        ),
        (
            DeclaredType::Semver,
            TypeParams::default(),
            r#""not-a-semver""#,
            "semantic version value is invalid",
        ),
        (
            DeclaredType::Datetime,
            TypeParams {
                format: Some("rfc3339".to_owned()),
                ..TypeParams::default()
            },
            r#""not-a-timestamp""#,
            "datetime must be RFC 3339 with an explicit offset",
        ),
    ] {
        assert_eq!(
            observation(kind, &params, raw).expect_err("invalid value"),
            message
        );
    }
}

#[test]
fn numeric_locale_and_datetime_helpers_cover_valid_and_invalid_grammars() {
    assert_eq!(
        normalize_decimal_input("12.50", None).expect("invariant"),
        "12.50"
    );
    assert!(normalize_decimal_input("1,000", None).is_err());
    assert_eq!(
        normalize_decimal_input("1,234.50", Some(NumericLocale::EnUs)).expect("en"),
        "1234.50"
    );
    assert_eq!(
        normalize_decimal_input("1.234,50", Some(NumericLocale::DeDe)).expect("de"),
        "1234.50"
    );
    for invalid in [
        "12,34", "1.2.3", ".2", "1,234.", "1,,234", "1234,567", "1,2a4", "1,234.5x",
    ] {
        assert!(
            normalize_grouped_decimal(invalid, ',', '.').is_err(),
            "{invalid}"
        );
    }
    for (raw, canonical) in [
        ("123", "123"),
        ("1234", "1234"),
        ("1,234", "1234"),
        ("123,456", "123456"),
        ("1,234.5", "1234.5"),
    ] {
        assert_eq!(
            normalize_grouped_decimal(raw, ',', '.').expect("grouped decimal"),
            canonical
        );
    }
    assert_eq!(
        parse_offset("-02:30").expect("offset").whole_seconds(),
        -9_000
    );
    for invalid in ["+99:99", "02:00", "*02:00", "+02-00"] {
        assert!(parse_offset(invalid).is_err(), "{invalid}");
    }
    assert_eq!(
        parse_datetime(
            "2026-07-14 10:00 +02:00",
            &TypeParams {
                format: Some("[year]-[month]-[day] [hour]:[minute] [offset_hour sign:mandatory]:[offset_minute]".to_owned()),
                ..TypeParams::default()
            },
        )
        .expect("explicit offset format"),
        "2026-07-14T08:00:00Z"
    );
    assert_eq!(
        parse_datetime(
            "2026-07-14 10:00",
            &TypeParams {
                format: Some("[year]-[month]-[day] [hour]:[minute]".to_owned()),
                assumed_offset: Some("+02:00".to_owned()),
                ..TypeParams::default()
            },
        )
        .expect("assumed offset"),
        "2026-07-14T08:00:00Z"
    );
    assert!(
        parse_datetime(
            "2026-07-14 10:00",
            &TypeParams {
                format: Some("[year]-[month]-[day] [hour]:[minute]".to_owned()),
                ..TypeParams::default()
            },
        )
        .is_err()
    );
}

#[test]
fn type_parameter_validation_rejects_every_crossed_family() {
    for kind in [
        DeclaredType::Text,
        DeclaredType::Integer,
        DeclaredType::Semver,
    ] {
        crate::model::validate_type_params(kind, &TypeParams::default()).expect("default params");
    }
    crate::model::validate_type_params(DeclaredType::Decimal, &TypeParams::default())
        .expect("decimal params");
    crate::model::validate_type_params(
        DeclaredType::Money,
        &TypeParams {
            currency: Some("USD".to_owned()),
            locale: Some(NumericLocale::EnUs),
            ..TypeParams::default()
        },
    )
    .expect("money params");
    crate::model::validate_type_params(
        DeclaredType::Datetime,
        &TypeParams {
            format: Some("rfc3339".to_owned()),
            ..TypeParams::default()
        },
    )
    .expect("datetime params");
    crate::model::validate_type_params(
        DeclaredType::Datetime,
        &TypeParams {
            format: Some("[year]-[month]-[day]".to_owned()),
            assumed_offset: Some("+00:00".to_owned()),
            ..TypeParams::default()
        },
    )
    .expect("custom datetime params");
    for (kind, params) in [
        (
            DeclaredType::Integer,
            TypeParams {
                locale: Some(NumericLocale::Invariant),
                ..TypeParams::default()
            },
        ),
        (
            DeclaredType::Semver,
            TypeParams {
                currency: Some("USD".to_owned()),
                ..TypeParams::default()
            },
        ),
        (
            DeclaredType::Decimal,
            TypeParams {
                currency: Some("USD".to_owned()),
                ..TypeParams::default()
            },
        ),
        (
            DeclaredType::Decimal,
            TypeParams {
                format: Some("ignored".to_owned()),
                ..TypeParams::default()
            },
        ),
        (
            DeclaredType::Decimal,
            TypeParams {
                assumed_offset: Some("+00:00".to_owned()),
                ..TypeParams::default()
            },
        ),
        (DeclaredType::Money, TypeParams::default()),
        (
            DeclaredType::Money,
            TypeParams {
                currency: Some("usd".to_owned()),
                ..TypeParams::default()
            },
        ),
        (
            DeclaredType::Money,
            TypeParams {
                currency: Some("US".to_owned()),
                ..TypeParams::default()
            },
        ),
        (
            DeclaredType::Datetime,
            TypeParams {
                currency: Some("USD".to_owned()),
                format: Some("rfc3339".to_owned()),
                ..TypeParams::default()
            },
        ),
        (DeclaredType::Datetime, TypeParams::default()),
        (
            DeclaredType::Datetime,
            TypeParams {
                format: Some("rfc3339".to_owned()),
                locale: Some(NumericLocale::Invariant),
                ..TypeParams::default()
            },
        ),
        (
            DeclaredType::Money,
            TypeParams {
                currency: Some("USD".to_owned()),
                format: Some("rfc3339".to_owned()),
                ..TypeParams::default()
            },
        ),
        (
            DeclaredType::Money,
            TypeParams {
                currency: Some("USD".to_owned()),
                assumed_offset: Some("+00:00".to_owned()),
                ..TypeParams::default()
            },
        ),
        (
            DeclaredType::Datetime,
            TypeParams {
                format: Some("[invalid".to_owned()),
                ..TypeParams::default()
            },
        ),
        (
            DeclaredType::Datetime,
            TypeParams {
                format: Some("[year]".to_owned()),
                assumed_offset: Some("invalid".to_owned()),
                ..TypeParams::default()
            },
        ),
    ] {
        assert!(crate::model::validate_type_params(kind, &params).is_err());
    }
    assert!(
        crate::model::validate_type_params(
            DeclaredType::Datetime,
            &TypeParams {
                format: Some("rfc3339".to_owned()),
                assumed_offset: Some("+02:00".to_owned()),
                ..TypeParams::default()
            },
        )
        .is_err()
    );
}

mod persistence;

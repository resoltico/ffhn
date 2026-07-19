use super::support::*;

#[test]
fn typed_value_helpers_reject_invalid_presentations_and_cover_all_formats() {
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
    assert!(normalize_grouped_decimal("12,34", ',', '.').is_err());
    assert!(normalize_grouped_decimal("1.2.3", ',', '.').is_err());
    assert!(normalize_grouped_decimal(".2", ',', '.').is_err());
    assert!(normalize_grouped_decimal("1,234.", ',', '.').is_err());
    assert!(normalize_grouped_decimal("1,,234", ',', '.').is_err());
    assert!(normalize_grouped_decimal("1234,567", ',', '.').is_err());
    assert!(normalize_grouped_decimal("1,2a4", ',', '.').is_err());
    assert!(normalize_grouped_decimal("1,234.5x", ',', '.').is_err());
    assert_eq!(
        normalize_grouped_decimal("123", ',', '.').expect("ungrouped decimal"),
        "123"
    );
    assert_eq!(
        normalize_grouped_decimal("1,234", ',', '.').expect("whole grouped decimal"),
        "1234"
    );
    assert_eq!(
        normalize_grouped_decimal("1,234.5", ',', '.').expect("decimal fraction"),
        "1234.5"
    );
    assert_eq!(
        parse_offset("-02:30").expect("offset").whole_seconds(),
        -9_000
    );
    assert!(parse_offset("+99:99").is_err());
    assert!(parse_offset("02:00").is_err());
    assert!(parse_offset("*02:00").is_err());
    assert!(parse_offset("+02-00").is_err());
    assert!(require_text("field", " \t").is_err());
    assert!(validate_max_bytes(1_024).is_ok());
    assert!(validate_max_bytes(1_023).is_err());
    assert_eq!(default_timeout_ms(), 15_000);
    assert_eq!(default_max_bytes(), 2_000_000);
    assert!(default_follow_redirects());
    assert!(is_sha256(&"a".repeat(64)));
    assert!(!is_sha256(&"A".repeat(64)));
    assert!(!is_sha256(&"!".repeat(64)));
    assert!(parse_canonical_value(DeclaredType::Integer, &TypeParams::default(), "x").is_err());
    assert!(parse_canonical_value(DeclaredType::Semver, &TypeParams::default(), "1.2").is_err());
    assert!(
        parse_canonical_value(
            DeclaredType::Datetime,
            &TypeParams {
                format: Some("rfc3339".to_owned()),
                ..TypeParams::default()
            },
            "not-a-date"
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Integer,
            &TypeParams {
                locale: Some(NumericLocale::Invariant),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Semver,
            &TypeParams {
                currency: Some("USD".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Decimal,
            &TypeParams {
                currency: Some("USD".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Decimal,
            &TypeParams {
                format: Some("ignored".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Decimal,
            &TypeParams {
                assumed_offset: Some("+02:00".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Money,
            &TypeParams {
                currency: Some("USD".to_owned()),
                format: Some("rfc3339".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Money,
            &TypeParams {
                currency: Some("usd".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Money,
            &TypeParams {
                currency: Some("USD".to_owned()),
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
                format: Some("rfc3339".to_owned()),
                locale: Some(NumericLocale::Invariant),
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
                currency: Some("USD".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert_eq!(
            parse_datetime(
                "2026-07-14 10:00 +02:00",
                &TypeParams {
                    format: Some(
                        "[year]-[month]-[day] [hour]:[minute] [offset_hour sign:mandatory]:[offset_minute]"
                            .to_owned()
                    ),
                    ..TypeParams::default()
                }
            )
            .expect("offset format"),
            "2026-07-14T08:00:00Z"
        );
}

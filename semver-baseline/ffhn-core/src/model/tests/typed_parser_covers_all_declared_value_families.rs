use super::support::*;

#[test]
fn typed_parser_covers_all_declared_value_families() {
    assert_eq!(
        target("text", "")
            .parse_json_scalar_token(r#""In Stock""#.to_owned())
            .expect("text")
            .canonical_value(),
        "In Stock"
    );
    assert_eq!(
        target("integer", "")
            .parse_json_scalar_token(i128::MAX.to_string())
            .expect("i128")
            .canonical_value(),
        i128::MAX.to_string()
    );
    assert_eq!(
        target("decimal", "")
            .parse_json_scalar_token("1.00".to_owned())
            .expect("decimal")
            .canonical_value(),
        "1"
    );
    assert_eq!(
        target("integer", "")
            .parse_json_scalar_token("not-json".to_owned())
            .expect_err("invalid JSON scalar token")
            .kind(),
        DiagnosticKind::ValueUnparseable
    );
    assert!(
        target("decimal", "")
            .parse_json_scalar_token("999999999999999999999999999999999999999".to_owned())
            .is_err()
    );
    assert_eq!(
        target("money", "[type_params]\ncurrency = \"USD\"\n")
            .parse_json_scalar_token(r#""1,234.50""#.to_owned())
            .expect_err("invariant money rejects grouping")
            .kind(),
        DiagnosticKind::ValueUnparseable
    );
    assert_eq!(
        target(
            "money",
            "[type_params]\ncurrency = \"USD\"\nlocale = \"en_us\"\n"
        )
        .parse_json_scalar_token(r#""1,234.50""#.to_owned())
        .expect("money")
        .canonical_value(),
        "1234.5"
    );
    assert_eq!(
        target("semver", "")
            .parse_json_scalar_token(r#""1.2.3+build.7""#.to_owned())
            .expect("semver")
            .canonical_value(),
        "1.2.3+build.7"
    );
    assert_eq!(
        target("datetime", "[type_params]\nformat = \"rfc3339\"\n")
            .parse_json_scalar_token(r#""2026-07-19T12:00:00+02:00""#.to_owned())
            .expect("datetime")
            .canonical_value(),
        "2026-07-19T10:00:00Z"
    );
}

#[test]
fn typed_parse_failures_emit_owned_messages_without_parser_prose() {
    let malformed_json = select_json_scalar_token("{", "/value")
        .expect_err("malformed JSON")
        .into_detail();
    assert_eq!(malformed_json.message(), "source is not valid JSON");

    let invalid_integer = target("integer", "")
        .parse_json_scalar_token(r#""not-an-integer""#.to_owned())
        .expect_err("invalid integer");
    assert_eq!(
        invalid_integer.message(),
        "integer value is not a valid signed 128-bit integer"
    );

    let invalid_decimal = target("decimal", "")
        .parse_json_scalar_token("999999999999999999999999999999999999999".to_owned())
        .expect_err("out-of-range decimal");
    assert_eq!(
        invalid_decimal.message(),
        "decimal value is invalid or out of the supported range"
    );

    let invalid_semver = target("semver", "")
        .parse_json_scalar_token(r#""not-a-semver""#.to_owned())
        .expect_err("invalid semantic version");
    assert_eq!(
        invalid_semver.message(),
        "semantic version value is invalid"
    );

    let invalid_datetime = target("datetime", "[type_params]\nformat = \"rfc3339\"\n")
        .parse_json_scalar_token(r#""not-a-timestamp""#.to_owned())
        .expect_err("invalid timestamp");
    assert_eq!(
        invalid_datetime.message(),
        "datetime must be RFC 3339 with an explicit offset"
    );
}

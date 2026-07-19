use super::support::*;

#[test]
fn datetime_requires_a_declared_format_and_never_uses_machine_timezone() {
    assert_eq!(
        target("datetime", "[type_params]\nformat = \"rfc3339\"\n")
            .parse_json_scalar_token(r#""2026-07-14T10:00:00+02:00""#.to_owned())
            .expect("explicit offset")
            .canonical_value(),
        "2026-07-14T08:00:00Z"
    );
    assert_eq!(
            target(
                "datetime",
                "[type_params]\nformat = \"[year]-[month]-[day] [hour]:[minute]\"\nassumed_offset = \"+02:00\"\n"
            )
            .parse_json_scalar_token(r#""2026-07-14 10:00""#.to_owned())
            .expect("configured assumed offset")
            .canonical_value(),
            "2026-07-14T08:00:00Z"
        );
    assert!(
        target("datetime", "[type_params]\nformat = \"rfc3339\"\n")
            .parse_json_scalar_token(r#""2026-07-14T10:00:00""#.to_owned())
            .is_err()
    );
    assert!(
        target(
            "datetime",
            "[type_params]\nformat = \"[year]-[month]-[day] [hour]:[minute]\"\n",
        )
        .parse_json_scalar_token(r#""2026-07-14 10:00""#.to_owned())
        .is_err()
    );
}

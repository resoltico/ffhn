use super::support::*;

#[test]
fn condition_configuration_validates_predicate_type_thresholds_and_bands() {
    let changed = measurement(
        "semver",
        "",
        &one_condition("kind = \"changed\"\nreference = \"fixed_initial_baseline\""),
    );
    assert_eq!(changed.conditions()[0].condition_id(), "condition");
    assert!(matches!(
        changed.conditions()[0].predicate(),
        ConditionPredicate::Changed {
            reference: ConditionReference::FixedInitialBaseline
        }
    ));

    assert!(
        decode_measurement(&measurement_toml(
            "semver",
            "",
            &one_condition(
                "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"1\"",
            ),
        ))
        .is_err()
    );

    assert!(decode_measurement(&measurement_toml(
        "integer",
        "",
        &one_condition(
            "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"-1\"",
        ),
    )).is_err());

    assert!(decode_measurement(&measurement_toml(
        "decimal",
        "",
        &one_condition(
            "kind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"-0.1\"",
        ),
    )).is_err());

    for (declared_type, type_params) in [
        ("integer", ""),
        ("decimal", ""),
        ("money", "[type_params]\ncurrency = \"USD\""),
    ] {
        let negative_zero_absolute = decode_measurement(&measurement_toml(
            declared_type,
            type_params,
            &one_condition(
                "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"-0\"",
            ),
        ))
        .expect("valid negative-zero threshold");
        negative_zero_absolute
            .validate()
            .expect("valid measurement");

        let negative_zero_percentage = decode_measurement(&measurement_toml(
            declared_type,
            type_params,
            &one_condition(
                "kind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"-0\"",
            ),
        ))
        .expect("valid negative-zero percentage");
        negative_zero_percentage
            .validate()
            .expect("valid measurement");
    }

    assert!(decode_measurement(&measurement_toml(
        "decimal",
        "",
        &one_condition("kind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"not-a-percentage\""),
    )).is_err());

    assert!(decode_measurement(&measurement_toml(
        "integer",
        "",
        &one_condition("kind = \"band\"\nenter_threshold = \"2\"\nexit_threshold = \"3\"\ndirection = \"rising\""),
    )).is_err());

    for predicate in [
        "kind = \"band\"\nenter_threshold = \"not-an-integer\"\nexit_threshold = \"3\"\ndirection = \"rising\"",
        "kind = \"band\"\nenter_threshold = \"3\"\nexit_threshold = \"not-an-integer\"\ndirection = \"rising\"",
    ] {
        assert!(
            decode_measurement(&measurement_toml("integer", "", &one_condition(predicate),))
                .is_err()
        );
    }

    assert!(
        decode_measurement(&measurement_toml(
            "integer",
            "",
            &one_condition("kind = \"gt\"\nthreshold = \"not-an-integer\""),
        ))
        .is_err()
    );

    let money = measurement(
        "money",
        "[type_params]\ncurrency = \"USD\"",
        &one_condition("kind = \"crosses\"\nthreshold = \"19.99\"\ndirection = \"rising\""),
    );
    assert_eq!(money.conditions().len(), 1);
}

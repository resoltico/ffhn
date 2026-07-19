use super::support::*;

#[test]
fn condition_configuration_validates_predicate_type_thresholds_and_bands() {
    let changed = target(
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

    let numeric_only = toml::from_str::<TargetDocument>(&target_toml(
        "semver",
        "",
        &one_condition(
            "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"1\"",
        ),
    ))
    .expect("structural target");
    assert!(numeric_only.validate().is_err());

    let negative_abs = toml::from_str::<TargetDocument>(&target_toml(
        "integer",
        "",
        &one_condition(
            "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"-1\"",
        ),
    ))
    .expect("structural target");
    assert!(negative_abs.validate().is_err());

    let negative_pct = toml::from_str::<TargetDocument>(&target_toml(
        "decimal",
        "",
        &one_condition(
            "kind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"-0.1\"",
        ),
    ))
    .expect("structural target");
    assert!(negative_pct.validate().is_err());

    for (declared_type, type_params) in [
        ("integer", ""),
        ("decimal", ""),
        ("money", "[type_params]\ncurrency = \"USD\""),
    ] {
        let negative_zero_absolute = toml::from_str::<TargetDocument>(&target_toml(
            declared_type,
            type_params,
            &one_condition(
                "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"-0\"",
            ),
        ))
        .expect("structural target");
        assert!(negative_zero_absolute.validate().is_ok(), "{declared_type}");

        let negative_zero_percentage = toml::from_str::<TargetDocument>(&target_toml(
            declared_type,
            type_params,
            &one_condition(
                "kind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"-0\"",
            ),
        ))
        .expect("structural target");
        assert!(
            negative_zero_percentage.validate().is_ok(),
            "{declared_type}"
        );
    }

    let malformed_pct = toml::from_str::<TargetDocument>(&target_toml(
        "decimal",
        "",
        &one_condition("kind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"not-a-percentage\""),
    ))
    .expect("structural target");
    assert!(malformed_pct.validate().is_err());

    let invalid_band = toml::from_str::<TargetDocument>(&target_toml(
        "integer",
        "",
        &one_condition("kind = \"band\"\nenter_threshold = \"2\"\nexit_threshold = \"3\"\ndirection = \"rising\""),
    ))
    .expect("structural target");
    assert!(invalid_band.validate().is_err());

    for predicate in [
        "kind = \"band\"\nenter_threshold = \"not-an-integer\"\nexit_threshold = \"3\"\ndirection = \"rising\"",
        "kind = \"band\"\nenter_threshold = \"3\"\nexit_threshold = \"not-an-integer\"\ndirection = \"rising\"",
    ] {
        let invalid_band = toml::from_str::<TargetDocument>(&target_toml(
            "integer",
            "",
            &one_condition(predicate),
        ))
        .expect("structural target");
        assert!(invalid_band.validate().is_err());
    }

    let invalid_threshold = toml::from_str::<TargetDocument>(&target_toml(
        "integer",
        "",
        &one_condition("kind = \"gt\"\nthreshold = \"not-an-integer\""),
    ))
    .expect("structural target");
    assert!(invalid_threshold.validate().is_err());

    let money = target(
        "money",
        "[type_params]\ncurrency = \"USD\"",
        &one_condition("kind = \"crosses\"\nthreshold = \"19.99\"\ndirection = \"rising\""),
    );
    assert_eq!(money.conditions().len(), 1);
}

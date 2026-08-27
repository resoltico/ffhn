use super::support::*;

#[test]
fn text_conditions_are_changed_only_with_exact_unicode_identity() {
    let current = observation("text", "", r#""Out of Stock""#);
    let reference = observation("text", "", r#""In Stock""#);
    for (reference_name, contexts) in [
        (
            "last_accepted_observation",
            context(Some(&reference), None, None, false),
        ),
        (
            "fixed_initial_baseline",
            context(None, Some(&reference), None, false),
        ),
        (
            "last_condition_transition",
            context(None, None, Some(&reference), false),
        ),
    ] {
        let target = measurement(
            "text",
            "",
            &one_condition(&format!(
                "kind = \"changed\"\nreference = \"{reference_name}\""
            )),
        );
        let evaluation = evaluate(&target, &current, &contexts);
        assert_eq!(evaluation.outcome(), ConditionOutcome::Satisfied);
        assert!(evaluation.trigger());
    }

    let target = measurement(
        "text",
        "",
        &one_condition("kind = \"changed\"\nreference = \"last_accepted_observation\""),
    );
    let escaped = observation("text", "", r#""\u00e9""#);
    let literal = observation("text", "", r#""é""#);
    let decomposed = observation("text", "", r#""e\u0301""#);
    assert_eq!(
        evaluate(
            &target,
            &literal,
            &context(Some(&escaped), None, None, false)
        )
        .outcome(),
        ConditionOutcome::NotSatisfied
    );
    assert_eq!(
        evaluate(
            &target,
            &decomposed,
            &context(Some(&literal), None, None, false),
        )
        .outcome(),
        ConditionOutcome::Satisfied
    );

    for predicate in [
        "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"1\"",
        "kind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"1\"",
        "kind = \"crosses\"\nthreshold = \"In Stock\"\ndirection = \"rising\"",
        "kind = \"lt\"\nthreshold = \"In Stock\"",
        "kind = \"gt\"\nthreshold = \"In Stock\"",
        "kind = \"band\"\nenter_threshold = \"In Stock\"\nexit_threshold = \"Out of Stock\"\ndirection = \"rising\"",
    ] {
        assert!(
            decode_measurement(&measurement_toml("text", "", &one_condition(predicate),)).is_err(),
            "text rejects {predicate}"
        );
    }
}

use super::support::*;

#[test]
fn absent_and_incompatible_contexts_are_unavailable_without_inferred_conversion() {
    let absolute = measurement(
        "integer",
        "",
        &one_condition(
            "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"1\"",
        ),
    );
    let integer = observation("integer", "", "2");
    let no_context = BTreeMap::new();
    assert_eq!(
        evaluate(&absolute, &integer, &no_context).outcome(),
        ConditionOutcome::Unavailable
    );
    let decimal = observation("decimal", "", "1");
    assert_eq!(
        evaluate(
            &absolute,
            &integer,
            &context(Some(&decimal), None, None, false)
        )
        .outcome(),
        ConditionOutcome::Unavailable
    );

    let changed = measurement(
        "integer",
        "",
        &one_condition("kind = \"changed\"\nreference = \"last_accepted_observation\""),
    );
    assert_eq!(
        evaluate(
            &changed,
            &integer,
            &context(Some(&decimal), None, None, false)
        )
        .outcome(),
        ConditionOutcome::Unavailable
    );

    let percentage = measurement(
        "integer",
        "",
        &one_condition(
            "kind = \"delta_pct\"\nreference = \"last_condition_transition\"\nthreshold = \"1\"",
        ),
    );
    assert_eq!(
        evaluate(&percentage, &integer, &context(None, None, None, false)).outcome(),
        ConditionOutcome::Unavailable
    );

    let crosses = measurement(
        "integer",
        "",
        &one_condition("kind = \"crosses\"\nthreshold = \"2\"\ndirection = \"rising\""),
    );
    assert_eq!(
        evaluate(
            &crosses,
            &integer,
            &context(Some(&decimal), None, None, false)
        )
        .outcome(),
        ConditionOutcome::Unavailable
    );
}

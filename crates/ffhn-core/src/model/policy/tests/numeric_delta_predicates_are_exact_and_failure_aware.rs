use super::support::*;

#[test]
fn numeric_delta_predicates_are_exact_and_failure_aware() {
    let absolute = measurement(
        "integer",
        "",
        &one_condition(
            "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"5\"",
        ),
    );
    let current = observation("integer", "", "15");
    let previous = observation("integer", "", "10");
    assert_eq!(
        evaluate(
            &absolute,
            &current,
            &context(Some(&previous), None, None, false)
        )
        .outcome(),
        ConditionOutcome::Satisfied
    );
    assert!(
        evaluate(
            &absolute,
            &current,
            &context(Some(&previous), None, None, false)
        )
        .trigger()
    );
    let next_absolute = observation("integer", "", "20");
    assert!(
        evaluate(
            &absolute,
            &next_absolute,
            &context(Some(&current), None, None, false)
        )
        .trigger()
    );
    let small = observation("integer", "", "14");
    assert_eq!(
        evaluate(
            &absolute,
            &small,
            &context(Some(&previous), None, None, false)
        )
        .outcome(),
        ConditionOutcome::NotSatisfied
    );

    let percentage = measurement(
        "decimal",
        "",
        &one_condition(
            "kind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"5.0000001\"",
        ),
    );
    let precise = observation("decimal", "", "105.0000001");
    let hundred = observation("decimal", "", "100");
    assert_eq!(
        evaluate(
            &percentage,
            &precise,
            &context(Some(&hundred), None, None, false)
        )
        .outcome(),
        ConditionOutcome::Satisfied
    );
    assert!(
        evaluate(
            &percentage,
            &precise,
            &context(Some(&hundred), None, None, false)
        )
        .trigger()
    );
    let next_precise = observation("decimal", "", "110.250001");
    assert!(
        evaluate(
            &percentage,
            &next_precise,
            &context(Some(&precise), None, None, false)
        )
        .trigger()
    );
    let zero = observation("decimal", "", "0");
    assert_eq!(
        evaluate(
            &percentage,
            &precise,
            &context(Some(&zero), None, None, false)
        )
        .outcome(),
        ConditionOutcome::ZeroReference
    );

    let integer_percentage = measurement(
        "integer",
        "",
        &one_condition(
            "kind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"5\"",
        ),
    );
    let maximum = observation("integer", "", &i128::MAX.to_string());
    let integer_zero = observation("integer", "", "0");
    assert_eq!(
        evaluate(
            &integer_percentage,
            &maximum,
            &context(Some(&integer_zero), None, None, false)
        )
        .outcome(),
        ConditionOutcome::ZeroReference
    );

    let overflow_current = observation("integer", "", &i128::MAX.to_string());
    let overflow_previous = observation("integer", "", &i128::MIN.to_string());
    assert_eq!(
        evaluate(
            &absolute,
            &overflow_current,
            &context(Some(&overflow_previous), None, None, false)
        )
        .outcome(),
        ConditionOutcome::ArithmeticOverflow
    );

    let money = measurement(
        "money",
        "[type_params]\ncurrency = \"USD\"",
        &one_condition(
            "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"1.50\"",
        ),
    );
    let money_current = observation("money", "[type_params]\ncurrency = \"USD\"", "12.50");
    let money_previous = observation("money", "[type_params]\ncurrency = \"USD\"", "11");
    assert_eq!(
        evaluate(
            &money,
            &money_current,
            &context(Some(&money_previous), None, None, false)
        )
        .outcome(),
        ConditionOutcome::Satisfied
    );
}

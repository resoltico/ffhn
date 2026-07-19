use super::support::*;

#[test]
fn changed_uses_canonical_identity_and_named_references() {
    let changed_target = target(
        "decimal",
        "",
        &one_condition("kind = \"changed\"\nreference = \"last_accepted_observation\""),
    );
    let current = observation("decimal", "", "1.00");
    let equal = observation("decimal", "", "1.0");
    let evaluation = valid_stage(
        &changed_target,
        &current,
        &context(Some(&equal), None, None, false),
    );
    assert_eq!(evaluation.condition_id(), "condition");
    assert_eq!(evaluation.outcome(), ConditionOutcome::NotSatisfied);
    assert!(!evaluation.trigger());
    assert!(!evaluation.active_after());

    let different = observation("decimal", "", "2");
    let evaluation = valid_stage(
        &changed_target,
        &different,
        &context(Some(&current), None, None, true),
    );
    assert_eq!(evaluation.outcome(), ConditionOutcome::Satisfied);
    assert!(evaluation.trigger());
    assert!(evaluation.active_after());

    let later = observation("decimal", "", "3");
    let second_distinct_change = valid_stage(
        &changed_target,
        &later,
        &context(Some(&different), None, None, false),
    );
    assert_eq!(
        second_distinct_change.outcome(),
        ConditionOutcome::Satisfied
    );
    assert!(second_distinct_change.trigger());

    let unavailable = valid_stage(
        &changed_target,
        &different,
        &context(None, None, None, false),
    );
    assert_eq!(unavailable.outcome(), ConditionOutcome::Unavailable);

    let fixed_target = target(
        "integer",
        "",
        &one_condition("kind = \"changed\"\nreference = \"fixed_initial_baseline\""),
    );
    let integer = observation("integer", "", "3");
    let initial = observation("integer", "", "1");
    assert_eq!(
        valid_stage(
            &fixed_target,
            &integer,
            &context(None, Some(&initial), None, false)
        )
        .outcome(),
        ConditionOutcome::Satisfied
    );

    let semver_target = target(
        "semver",
        "",
        &one_condition("kind = \"changed\"\nreference = \"last_condition_transition\""),
    );
    let left = observation("semver", "", r#""1.2.3+left""#);
    let right = observation("semver", "", r#""1.2.3+right""#);
    assert_eq!(
        valid_stage(
            &semver_target,
            &right,
            &context(None, None, Some(&left), false)
        )
        .outcome(),
        ConditionOutcome::Satisfied
    );
}

use super::support::*;

#[test]
fn ordered_crossing_and_level_predicates_have_their_specified_triggers() {
    let rising = measurement(
        "integer",
        "",
        &one_condition("kind = \"crosses\"\nthreshold = \"10\"\ndirection = \"rising\""),
    );
    let nine = observation("integer", "", "9");
    let ten = observation("integer", "", "10");
    let evaluation = evaluate(&rising, &ten, &context(Some(&nine), None, None, false));
    assert_eq!(evaluation.outcome(), ConditionOutcome::Satisfied);
    assert!(evaluation.trigger());
    let eleven = observation("integer", "", "11");
    assert_eq!(
        evaluate(&rising, &eleven, &context(Some(&ten), None, None, false)).outcome(),
        ConditionOutcome::NotSatisfied
    );
    assert!(!evaluate(&rising, &eleven, &context(Some(&ten), None, None, false)).trigger());
    assert!(!evaluate(&rising, &nine, &context(Some(&eleven), None, None, false)).trigger());
    let recross = evaluate(&rising, &ten, &context(Some(&nine), None, None, false));
    assert_eq!(recross.outcome(), ConditionOutcome::Satisfied);
    assert!(recross.trigger());
    assert_eq!(
        evaluate(&rising, &eleven, &context(None, None, None, false)).outcome(),
        ConditionOutcome::Unavailable
    );

    let falling = measurement(
        "datetime",
        "[type_params]\nformat = \"rfc3339\"",
        &one_condition(
            "kind = \"crosses\"\nthreshold = \"2026-01-01T00:00:00Z\"\ndirection = \"falling\"",
        ),
    );
    let after = observation(
        "datetime",
        "[type_params]\nformat = \"rfc3339\"",
        r#""2026-01-02T00:00:00Z""#,
    );
    let threshold = observation(
        "datetime",
        "[type_params]\nformat = \"rfc3339\"",
        r#""2026-01-01T00:00:00Z""#,
    );
    assert_eq!(
        evaluate(
            &falling,
            &threshold,
            &context(Some(&after), None, None, false)
        )
        .outcome(),
        ConditionOutcome::Satisfied
    );
    let later = observation(
        "datetime",
        "[type_params]\nformat = \"rfc3339\"",
        r#""2026-01-03T00:00:00Z""#,
    );
    assert_eq!(
        evaluate(&falling, &after, &context(Some(&later), None, None, false)).outcome(),
        ConditionOutcome::NotSatisfied
    );
    let before = observation(
        "datetime",
        "[type_params]\nformat = \"rfc3339\"",
        r#""2025-12-31T00:00:00Z""#,
    );
    assert_eq!(
        evaluate(
            &falling,
            &threshold,
            &context(Some(&before), None, None, false)
        )
        .outcome(),
        ConditionOutcome::NotSatisfied
    );

    let lt = measurement(
        "integer",
        "",
        &one_condition("kind = \"lt\"\nthreshold = \"10\""),
    );
    let five = observation("integer", "", "5");
    let entry = evaluate(&lt, &five, &context(None, None, None, false));
    assert_eq!(entry.outcome(), ConditionOutcome::Satisfied);
    assert!(entry.trigger());
    assert!(entry.active_after());
    let retained = evaluate(&lt, &five, &context(None, None, None, true));
    assert!(!retained.trigger());
    let fifteen = observation("integer", "", "15");
    let leave = evaluate(&lt, &fifteen, &context(None, None, None, true));
    assert_eq!(leave.outcome(), ConditionOutcome::NotSatisfied);
    assert!(!leave.active_after());
    let threshold = observation("integer", "", "10");
    assert_eq!(
        evaluate(&lt, &threshold, &context(None, None, None, false)).outcome(),
        ConditionOutcome::NotSatisfied
    );
    assert!(evaluate(&lt, &five, &context(None, None, None, false)).trigger());

    let gt = measurement(
        "integer",
        "",
        &one_condition("kind = \"gt\"\nthreshold = \"10\""),
    );
    let gt_entry = evaluate(&gt, &fifteen, &context(None, None, None, false));
    assert_eq!(gt_entry.outcome(), ConditionOutcome::Satisfied);
    assert!(gt_entry.trigger());
    assert!(!evaluate(&gt, &fifteen, &context(None, None, None, true)).trigger());
    assert_eq!(
        evaluate(&gt, &five, &context(None, None, None, false)).outcome(),
        ConditionOutcome::NotSatisfied
    );
    assert_eq!(
        evaluate(&gt, &threshold, &context(None, None, None, false)).outcome(),
        ConditionOutcome::NotSatisfied
    );
    let gt_leave = evaluate(&gt, &threshold, &context(None, None, None, true));
    assert!(!gt_leave.active_after());
    assert!(evaluate(&gt, &fifteen, &context(None, None, None, false)).trigger());
}

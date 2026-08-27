use super::support::*;

#[test]
fn bands_apply_directional_hysteresis_without_resetting_on_unavailability() {
    let rising = measurement(
        "integer",
        "",
        &one_condition(
            "kind = \"band\"\nenter_threshold = \"10\"\nexit_threshold = \"8\"\ndirection = \"rising\"",
        ),
    );
    let ten = observation("integer", "", "10");
    let entry = evaluate(&rising, &ten, &context(None, None, None, false));
    assert_eq!(entry.outcome(), ConditionOutcome::Satisfied);
    assert!(entry.trigger());
    let nine = observation("integer", "", "9");
    let retained = evaluate(&rising, &nine, &context(None, None, None, true));
    assert_eq!(retained.outcome(), ConditionOutcome::Satisfied);
    assert!(!retained.trigger());
    let eight = observation("integer", "", "8");
    assert_eq!(
        evaluate(&rising, &eight, &context(None, None, None, true)).outcome(),
        ConditionOutcome::Satisfied
    );
    let seven = observation("integer", "", "7");
    let leave = evaluate(&rising, &seven, &context(None, None, None, true));
    assert_eq!(leave.outcome(), ConditionOutcome::NotSatisfied);
    assert!(!leave.active_after());

    let falling = measurement(
        "integer",
        "",
        &one_condition(
            "kind = \"band\"\nenter_threshold = \"8\"\nexit_threshold = \"10\"\ndirection = \"falling\"",
        ),
    );
    let eight = observation("integer", "", "8");
    let falling_entry = evaluate(&falling, &eight, &context(None, None, None, false));
    assert_eq!(falling_entry.outcome(), ConditionOutcome::Satisfied);
    assert!(falling_entry.trigger());
    let nine = observation("integer", "", "9");
    let falling_retained = evaluate(&falling, &nine, &context(None, None, None, true));
    assert_eq!(falling_retained.outcome(), ConditionOutcome::Satisfied);
    assert!(!falling_retained.trigger());
    let ten = observation("integer", "", "10");
    assert_eq!(
        evaluate(&falling, &ten, &context(None, None, None, true)).outcome(),
        ConditionOutcome::Satisfied
    );
    let eleven = observation("integer", "", "11");
    let falling_leave = evaluate(&falling, &eleven, &context(None, None, None, true));
    assert_eq!(falling_leave.outcome(), ConditionOutcome::NotSatisfied);
    assert!(!falling_leave.active_after());
    assert!(evaluate(&falling, &eight, &context(None, None, None, false)).trigger());
}

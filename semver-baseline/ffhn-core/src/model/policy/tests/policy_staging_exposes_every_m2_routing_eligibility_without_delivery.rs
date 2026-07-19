use super::support::*;
use crate::{DeliveryEventKind, RouteFamily};

#[test]
fn policy_staging_exposes_every_m2_routing_eligibility_without_delivery() {
    assert_eq!(
        [
            ConditionOutcome::Satisfied,
            ConditionOutcome::NotSatisfied,
            ConditionOutcome::Unavailable,
            ConditionOutcome::ArithmeticOverflow,
            ConditionOutcome::ZeroReference,
        ]
        .map(ConditionOutcome::as_str),
        [
            "satisfied",
            "not_satisfied",
            "unavailable",
            "arithmetic_overflow",
            "zero_reference",
        ]
    );
    assert_eq!(
        [
            ConditionReference::LastAcceptedObservation,
            ConditionReference::FixedInitialBaseline,
            ConditionReference::LastConditionTransition,
        ]
        .map(ConditionReference::as_str),
        [
            "last_accepted_observation",
            "fixed_initial_baseline",
            "last_condition_transition",
        ]
    );
    let reset = StagedEventEligibility::OnRun {
        cause: OnRunEventCause::Reset,
    };
    assert_eq!(reset.route_family(), RouteFamily::OnRun);
    assert_eq!(reset.event_kind(), DeliveryEventKind::Reset);

    let changed = target(
        "integer",
        "",
        &one_condition("kind = \"changed\"\nreference = \"last_accepted_observation\""),
    );
    let previous = observation("integer", "", "1");
    let current = observation("integer", "", "2");
    let changed_stage = changed
        .stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: &current,
            },
            &context(Some(&previous), None, None, false),
        )
        .expect("changed stage");
    assert_eq!(
        changed_stage.event_eligibilities(),
        &[StagedEventEligibility::OnCondition {
            condition_id: ConditionId::new("condition").expect("condition id"),
        }]
    );

    let percentage = target(
        "decimal",
        "",
        &one_condition(
            "kind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"1\"",
        ),
    );
    let zero = observation("decimal", "", "0");
    let one = observation("decimal", "", "1");
    let zero_reference_stage = percentage
        .stage_policy_run(
            PolicyRunInput::ValidObservation { observation: &one },
            &context(Some(&zero), None, None, false),
        )
        .expect("zero-reference stage");
    assert_eq!(
        zero_reference_stage.event_eligibilities(),
        &[StagedEventEligibility::OnRun {
            cause: OnRunEventCause::ConditionIssue {
                condition_id: ConditionId::new("condition").expect("condition id"),
                issue: ConditionIssue::ZeroReference,
            },
        }]
    );

    let absolute = target(
        "integer",
        "",
        &one_condition(
            "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"0\"",
        ),
    );
    let maximum = observation("integer", "", &i128::MAX.to_string());
    let minimum = observation("integer", "", &i128::MIN.to_string());
    let overflow_stage = absolute
        .stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: &maximum,
            },
            &context(Some(&minimum), None, None, false),
        )
        .expect("overflow stage");
    assert_eq!(
        overflow_stage.event_eligibilities(),
        &[StagedEventEligibility::OnRun {
            cause: OnRunEventCause::ConditionIssue {
                condition_id: ConditionId::new("condition").expect("condition id"),
                issue: ConditionIssue::ArithmeticOverflow,
            },
        }]
    );
}

use std::cmp::Ordering;
use std::collections::BTreeMap;

use super::evaluate::{
    ConditionContext, ConditionEvaluation, ConditionIssue, ConditionOutcome, OnRunEventCause,
    PolicyRunInput, StagedEventEligibility, StagedPolicyRun,
};
use super::value::{ArithmeticResult, PolicyValue, parse_config_value, parse_percentage};
use super::*;
use crate::{ConditionId, PermanentErrorCode, SourceSuspectReason, TargetDocument};
use rust_decimal::Decimal;
use semver::Version;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

fn target(declared_type: &str, type_params: &str, conditions: &str) -> TargetDocument {
    let document: TargetDocument =
        toml::from_str(&target_toml(declared_type, type_params, conditions)).expect("target TOML");
    document.validate().expect("valid target");
    document
}

fn observation(declared_type: &str, type_params: &str, raw: &str) -> crate::Observation {
    target(declared_type, type_params, "conditions = []")
        .parse_json_scalar_token(raw.to_owned())
        .expect("valid observation")
}

fn one_condition(predicate: &str) -> String {
    format!("[[conditions]]\ncondition_id = \"condition\"\n\n[conditions.predicate]\n{predicate}")
}

fn context<'a>(
    last: Option<&'a crate::Observation>,
    fixed: Option<&'a crate::Observation>,
    transition: Option<&'a crate::Observation>,
    active: bool,
) -> BTreeMap<ConditionId, ConditionContext<'a>> {
    BTreeMap::from([(
        ConditionId::new("condition").expect("condition id"),
        ConditionContext::new(last, fixed, transition, active),
    )])
}

fn valid_stage<'a>(
    target: &TargetDocument,
    current: &'a crate::Observation,
    contexts: &BTreeMap<ConditionId, ConditionContext<'a>>,
) -> ConditionEvaluation {
    let StagedPolicyRun::ValidObservation {
        condition_evaluations,
        ..
    } = target
        .stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: current,
            },
            contexts,
        )
        .expect("valid policy stage")
    else {
        panic!("valid input must stage a valid observation");
    };
    assert_eq!(condition_evaluations.len(), 1);
    condition_evaluations
        .into_iter()
        .next()
        .expect("evaluation")
}

#[test]
fn condition_id_is_stable_target_local_identity() {
    let identifier = ConditionId::new("price_drop").expect("identifier");
    assert_eq!(identifier.as_str(), "price_drop");
    assert_eq!(identifier.to_string(), "price_drop");
    assert_eq!(identifier.as_ref(), "price_drop");
    assert_eq!(String::from(identifier.clone()), "price_drop");
    assert_eq!(
        "price_drop".parse::<ConditionId>().expect("parsed"),
        identifier
    );
    assert_eq!(
        ConditionId::new("1price")
            .expect("digit-led identifier")
            .as_str(),
        "1price"
    );
    assert_eq!(
        ConditionId::new("a-1")
            .expect("mixed separator identifier")
            .as_str(),
        "a-1"
    );
    assert_eq!(
        serde_json::from_str::<ConditionId>("\"price-drop\"")
            .expect("deserialized")
            .as_str(),
        "price-drop"
    );
    for invalid in [
        "",
        "Price",
        "price--drop",
        "price__drop",
        "price-_drop",
        "price_-drop",
        "price_",
        "-price",
        &"a".repeat(65),
    ] {
        assert!(ConditionId::new(invalid).is_err(), "{invalid}");
    }
    assert!(serde_json::from_str::<ConditionId>("\"Price\"").is_err());
}

#[test]
fn policy_configuration_is_a_hard_schema_eight_contract() {
    let without_conditions = toml::from_str::<TargetDocument>(
        "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\n\n[target]\nkind = \"file\"\nfile_path = \"/tmp/source.json\"\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
    );
    assert!(without_conditions.is_err());

    let mut wrong_version = target("integer", "", "conditions = []");
    let mut wire = serde_json::to_value(&wrong_version).expect("wire");
    wire["schema_version"] = serde_json::json!(5);
    wrong_version = serde_json::from_value(wire).expect("structural target");
    assert!(wrong_version.validate().is_err());
    assert!(
        wrong_version
            .stage_policy_run(
                PolicyRunInput::PermanentContractError {
                    error_code: PermanentErrorCode::InvalidJsonPointer,
                    episode_began: false,
                },
                &BTreeMap::new(),
            )
            .is_err()
    );

    let unordered = toml::from_str::<TargetDocument>(&target_toml(
        "integer",
        "",
        "[[conditions]]\ncondition_id = \"zeta\"\n[conditions.predicate]\nkind = \"lt\"\nthreshold = \"1\"\n\n[[conditions]]\ncondition_id = \"alpha\"\n[conditions.predicate]\nkind = \"gt\"\nthreshold = \"0\"",
    ))
    .expect("structural target");
    assert!(unordered.validate().is_err());

    let duplicate = toml::from_str::<TargetDocument>(&target_toml(
        "integer",
        "",
        "[[conditions]]\ncondition_id = \"same\"\n[conditions.predicate]\nkind = \"lt\"\nthreshold = \"1\"\n\n[[conditions]]\ncondition_id = \"same\"\n[conditions.predicate]\nkind = \"gt\"\nthreshold = \"0\"",
    ))
    .expect("structural target");
    assert!(duplicate.validate().is_err());

    let ordered = target(
        "integer",
        "",
        "[[conditions]]\ncondition_id = \"alpha\"\n[conditions.predicate]\nkind = \"lt\"\nthreshold = \"1\"\n\n[[conditions]]\ncondition_id = \"beta\"\n[conditions.predicate]\nkind = \"gt\"\nthreshold = \"0\"",
    );
    assert_eq!(
        ordered
            .conditions()
            .iter()
            .map(Condition::condition_id)
            .collect::<Vec<_>>(),
        ["alpha", "beta"]
    );
}

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

#[test]
fn crossing_refuses_an_unparseable_persisted_predecessor() {
    let target = target(
        "integer",
        "",
        &one_condition("kind = \"crosses\"\nthreshold = \"10\"\ndirection = \"rising\""),
    );
    let current = observation("integer", "", "10");
    let prior = observation("integer", "", "9");
    let mut wire = serde_json::to_value(prior).expect("prior JSON");
    wire["canonical_value"] = serde_json::json!("not-an-integer");
    let prior = serde_json::from_value(wire).expect("structurally valid predecessor");
    assert!(
        target
            .stage_policy_run(
                PolicyRunInput::ValidObservation {
                    observation: &current,
                },
                &context(Some(&prior), None, None, false),
            )
            .is_err()
    );
}

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

#[test]
fn numeric_delta_predicates_are_exact_and_failure_aware() {
    let absolute = target(
        "integer",
        "",
        &one_condition(
            "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"5\"",
        ),
    );
    let current = observation("integer", "", "15");
    let previous = observation("integer", "", "10");
    assert_eq!(
        valid_stage(
            &absolute,
            &current,
            &context(Some(&previous), None, None, false)
        )
        .outcome(),
        ConditionOutcome::Satisfied
    );
    assert!(
        valid_stage(
            &absolute,
            &current,
            &context(Some(&previous), None, None, false)
        )
        .trigger()
    );
    let next_absolute = observation("integer", "", "20");
    assert!(
        valid_stage(
            &absolute,
            &next_absolute,
            &context(Some(&current), None, None, false)
        )
        .trigger()
    );
    let small = observation("integer", "", "14");
    assert_eq!(
        valid_stage(
            &absolute,
            &small,
            &context(Some(&previous), None, None, false)
        )
        .outcome(),
        ConditionOutcome::NotSatisfied
    );

    let percentage = target(
        "decimal",
        "",
        &one_condition(
            "kind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"5.0000001\"",
        ),
    );
    let precise = observation("decimal", "", "105.0000001");
    let hundred = observation("decimal", "", "100");
    assert_eq!(
        valid_stage(
            &percentage,
            &precise,
            &context(Some(&hundred), None, None, false)
        )
        .outcome(),
        ConditionOutcome::Satisfied
    );
    assert!(
        valid_stage(
            &percentage,
            &precise,
            &context(Some(&hundred), None, None, false)
        )
        .trigger()
    );
    let next_precise = observation("decimal", "", "110.250001");
    assert!(
        valid_stage(
            &percentage,
            &next_precise,
            &context(Some(&precise), None, None, false)
        )
        .trigger()
    );
    let zero = observation("decimal", "", "0");
    assert_eq!(
        valid_stage(
            &percentage,
            &precise,
            &context(Some(&zero), None, None, false)
        )
        .outcome(),
        ConditionOutcome::ZeroReference
    );

    let integer_percentage = target(
        "integer",
        "",
        &one_condition(
            "kind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"5\"",
        ),
    );
    let maximum = observation("integer", "", &i128::MAX.to_string());
    let integer_zero = observation("integer", "", "0");
    assert_eq!(
        valid_stage(
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
        valid_stage(
            &absolute,
            &overflow_current,
            &context(Some(&overflow_previous), None, None, false)
        )
        .outcome(),
        ConditionOutcome::ArithmeticOverflow
    );

    let money = target(
        "money",
        "[type_params]\ncurrency = \"USD\"",
        &one_condition(
            "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"1.50\"",
        ),
    );
    let money_current = observation("money", "[type_params]\ncurrency = \"USD\"", "12.50");
    let money_previous = observation("money", "[type_params]\ncurrency = \"USD\"", "11");
    assert_eq!(
        valid_stage(
            &money,
            &money_current,
            &context(Some(&money_previous), None, None, false)
        )
        .outcome(),
        ConditionOutcome::Satisfied
    );
}

#[test]
fn ordered_crossing_and_level_predicates_have_their_specified_triggers() {
    let rising = target(
        "integer",
        "",
        &one_condition("kind = \"crosses\"\nthreshold = \"10\"\ndirection = \"rising\""),
    );
    let nine = observation("integer", "", "9");
    let ten = observation("integer", "", "10");
    let evaluation = valid_stage(&rising, &ten, &context(Some(&nine), None, None, false));
    assert_eq!(evaluation.outcome(), ConditionOutcome::Satisfied);
    assert!(evaluation.trigger());
    let eleven = observation("integer", "", "11");
    assert_eq!(
        valid_stage(&rising, &eleven, &context(Some(&ten), None, None, false)).outcome(),
        ConditionOutcome::NotSatisfied
    );
    assert!(!valid_stage(&rising, &eleven, &context(Some(&ten), None, None, false)).trigger());
    assert!(!valid_stage(&rising, &nine, &context(Some(&eleven), None, None, false)).trigger());
    let recross = valid_stage(&rising, &ten, &context(Some(&nine), None, None, false));
    assert_eq!(recross.outcome(), ConditionOutcome::Satisfied);
    assert!(recross.trigger());
    assert_eq!(
        valid_stage(&rising, &eleven, &context(None, None, None, false)).outcome(),
        ConditionOutcome::Unavailable
    );

    let falling = target(
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
        valid_stage(
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
        valid_stage(&falling, &after, &context(Some(&later), None, None, false)).outcome(),
        ConditionOutcome::NotSatisfied
    );
    let before = observation(
        "datetime",
        "[type_params]\nformat = \"rfc3339\"",
        r#""2025-12-31T00:00:00Z""#,
    );
    assert_eq!(
        valid_stage(
            &falling,
            &threshold,
            &context(Some(&before), None, None, false)
        )
        .outcome(),
        ConditionOutcome::NotSatisfied
    );

    let lt = target(
        "integer",
        "",
        &one_condition("kind = \"lt\"\nthreshold = \"10\""),
    );
    let five = observation("integer", "", "5");
    let entry = valid_stage(&lt, &five, &context(None, None, None, false));
    assert_eq!(entry.outcome(), ConditionOutcome::Satisfied);
    assert!(entry.trigger());
    assert!(entry.active_after());
    let retained = valid_stage(&lt, &five, &context(None, None, None, true));
    assert!(!retained.trigger());
    let fifteen = observation("integer", "", "15");
    let leave = valid_stage(&lt, &fifteen, &context(None, None, None, true));
    assert_eq!(leave.outcome(), ConditionOutcome::NotSatisfied);
    assert!(!leave.active_after());
    let threshold = observation("integer", "", "10");
    assert_eq!(
        valid_stage(&lt, &threshold, &context(None, None, None, false)).outcome(),
        ConditionOutcome::NotSatisfied
    );
    assert!(valid_stage(&lt, &five, &context(None, None, None, false)).trigger());

    let gt = target(
        "integer",
        "",
        &one_condition("kind = \"gt\"\nthreshold = \"10\""),
    );
    let gt_entry = valid_stage(&gt, &fifteen, &context(None, None, None, false));
    assert_eq!(gt_entry.outcome(), ConditionOutcome::Satisfied);
    assert!(gt_entry.trigger());
    assert!(!valid_stage(&gt, &fifteen, &context(None, None, None, true)).trigger());
    assert_eq!(
        valid_stage(&gt, &five, &context(None, None, None, false)).outcome(),
        ConditionOutcome::NotSatisfied
    );
    assert_eq!(
        valid_stage(&gt, &threshold, &context(None, None, None, false)).outcome(),
        ConditionOutcome::NotSatisfied
    );
    let gt_leave = valid_stage(&gt, &threshold, &context(None, None, None, true));
    assert!(!gt_leave.active_after());
    assert!(valid_stage(&gt, &fifteen, &context(None, None, None, false)).trigger());
}

#[test]
fn bands_apply_directional_hysteresis_without_resetting_on_unavailability() {
    let rising = target(
        "integer",
        "",
        &one_condition(
            "kind = \"band\"\nenter_threshold = \"10\"\nexit_threshold = \"8\"\ndirection = \"rising\"",
        ),
    );
    let ten = observation("integer", "", "10");
    let entry = valid_stage(&rising, &ten, &context(None, None, None, false));
    assert_eq!(entry.outcome(), ConditionOutcome::Satisfied);
    assert!(entry.trigger());
    let nine = observation("integer", "", "9");
    let retained = valid_stage(&rising, &nine, &context(None, None, None, true));
    assert_eq!(retained.outcome(), ConditionOutcome::Satisfied);
    assert!(!retained.trigger());
    let eight = observation("integer", "", "8");
    assert_eq!(
        valid_stage(&rising, &eight, &context(None, None, None, true)).outcome(),
        ConditionOutcome::Satisfied
    );
    let seven = observation("integer", "", "7");
    let leave = valid_stage(&rising, &seven, &context(None, None, None, true));
    assert_eq!(leave.outcome(), ConditionOutcome::NotSatisfied);
    assert!(!leave.active_after());

    let falling = target(
        "integer",
        "",
        &one_condition(
            "kind = \"band\"\nenter_threshold = \"8\"\nexit_threshold = \"10\"\ndirection = \"falling\"",
        ),
    );
    let eight = observation("integer", "", "8");
    let falling_entry = valid_stage(&falling, &eight, &context(None, None, None, false));
    assert_eq!(falling_entry.outcome(), ConditionOutcome::Satisfied);
    assert!(falling_entry.trigger());
    let nine = observation("integer", "", "9");
    let falling_retained = valid_stage(&falling, &nine, &context(None, None, None, true));
    assert_eq!(falling_retained.outcome(), ConditionOutcome::Satisfied);
    assert!(!falling_retained.trigger());
    let ten = observation("integer", "", "10");
    assert_eq!(
        valid_stage(&falling, &ten, &context(None, None, None, true)).outcome(),
        ConditionOutcome::Satisfied
    );
    let eleven = observation("integer", "", "11");
    let falling_leave = valid_stage(&falling, &eleven, &context(None, None, None, true));
    assert_eq!(falling_leave.outcome(), ConditionOutcome::NotSatisfied);
    assert!(!falling_leave.active_after());
    assert!(valid_stage(&falling, &eight, &context(None, None, None, false)).trigger());
}

#[test]
fn policy_staging_exposes_every_m2_routing_eligibility_without_delivery() {
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

#[test]
fn absent_and_incompatible_contexts_are_unavailable_without_inferred_conversion() {
    let absolute = target(
        "integer",
        "",
        &one_condition(
            "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"1\"",
        ),
    );
    let integer = observation("integer", "", "2");
    let no_context = BTreeMap::new();
    assert_eq!(
        valid_stage(&absolute, &integer, &no_context).outcome(),
        ConditionOutcome::Unavailable
    );
    let decimal = observation("decimal", "", "1");
    assert_eq!(
        valid_stage(
            &absolute,
            &integer,
            &context(Some(&decimal), None, None, false)
        )
        .outcome(),
        ConditionOutcome::Unavailable
    );

    let changed = target(
        "integer",
        "",
        &one_condition("kind = \"changed\"\nreference = \"last_accepted_observation\""),
    );
    assert_eq!(
        valid_stage(
            &changed,
            &integer,
            &context(Some(&decimal), None, None, false)
        )
        .outcome(),
        ConditionOutcome::Unavailable
    );

    let percentage = target(
        "integer",
        "",
        &one_condition(
            "kind = \"delta_pct\"\nreference = \"last_condition_transition\"\nthreshold = \"1\"",
        ),
    );
    assert_eq!(
        valid_stage(&percentage, &integer, &context(None, None, None, false)).outcome(),
        ConditionOutcome::Unavailable
    );

    let crosses = target(
        "integer",
        "",
        &one_condition("kind = \"crosses\"\nthreshold = \"2\"\ndirection = \"rising\""),
    );
    assert_eq!(
        valid_stage(
            &crosses,
            &integer,
            &context(Some(&decimal), None, None, false)
        )
        .outcome(),
        ConditionOutcome::Unavailable
    );
}

#[test]
fn policy_values_cover_each_typed_comparison_and_checked_arithmetic_path() {
    let dollar = PolicyValue::Money {
        amount: Decimal::from(2),
        currency: "USD".to_owned(),
    };
    let dollar_three = PolicyValue::Money {
        amount: Decimal::from(3),
        currency: "USD".to_owned(),
    };
    let euro = PolicyValue::Money {
        amount: Decimal::from(2),
        currency: "EUR".to_owned(),
    };
    let semver_one = PolicyValue::Semver(Version::parse("1.0.0+left").expect("semver"));
    let semver_two = PolicyValue::Semver(Version::parse("1.0.1").expect("semver"));
    let instant = OffsetDateTime::parse("2026-01-01T00:00:00Z", &Rfc3339).expect("time");
    let later = OffsetDateTime::parse("2026-01-02T00:00:00Z", &Rfc3339).expect("time");
    let datetime = PolicyValue::Datetime(instant);
    let later_datetime = PolicyValue::Datetime(later);

    assert_eq!(dollar.compare(&dollar_three), Some(Ordering::Less));
    assert_eq!(dollar.compare(&euro), None);
    assert_eq!(semver_one.compare(&semver_two), Some(Ordering::Less));
    assert_eq!(datetime.compare(&later_datetime), Some(Ordering::Less));
    assert!(!dollar.is_negative_numeric());
    assert!(PolicyValue::Integer(-1).is_negative_numeric());
    assert!(PolicyValue::Decimal(Decimal::NEGATIVE_ONE).is_negative_numeric());
    assert!(
        PolicyValue::Money {
            amount: Decimal::NEGATIVE_ONE,
            currency: "USD".to_owned(),
        }
        .is_negative_numeric()
    );
    assert!(!semver_one.is_negative_numeric());
    assert!(!datetime.is_negative_numeric());
    assert!(dollar.canonical_identity_eq(&dollar));
    assert!(!dollar.canonical_identity_eq(&euro));
    assert!(!dollar.canonical_identity_eq(&dollar_three));
    assert!(semver_one.canonical_identity_eq(&semver_one));
    assert!(datetime.canonical_identity_eq(&datetime));
    assert!(!dollar.canonical_identity_eq(&PolicyValue::Integer(2)));

    assert_eq!(
        PolicyValue::Decimal(Decimal::from(3)).exact_abs_delta_at_least(
            &PolicyValue::Decimal(Decimal::ONE),
            &PolicyValue::Decimal(Decimal::from(2)),
        ),
        ArithmeticResult::Decision(true)
    );
    assert_eq!(
        PolicyValue::Decimal(Decimal::MAX).exact_abs_delta_at_least(
            &PolicyValue::Decimal(-Decimal::MAX),
            &PolicyValue::Decimal(Decimal::ZERO),
        ),
        ArithmeticResult::Overflow
    );
    assert_eq!(
        dollar_three.exact_abs_delta_at_least(&dollar, &dollar),
        ArithmeticResult::Decision(false)
    );
    assert_eq!(
        dollar.exact_abs_delta_at_least(&euro, &dollar),
        ArithmeticResult::Unavailable
    );
    assert_eq!(
        dollar_three.exact_abs_delta_at_least(&dollar, &euro),
        ArithmeticResult::Unavailable
    );
    assert_eq!(
        PolicyValue::Integer(i128::MIN)
            .exact_abs_delta_at_least(&PolicyValue::Integer(0), &PolicyValue::Integer(0),),
        ArithmeticResult::Overflow
    );
    let dollar_max = PolicyValue::Money {
        amount: Decimal::MAX,
        currency: "USD".to_owned(),
    };
    let dollar_negative_max = PolicyValue::Money {
        amount: -Decimal::MAX,
        currency: "USD".to_owned(),
    };
    assert_eq!(
        dollar_max.exact_abs_delta_at_least(
            &dollar_negative_max,
            &PolicyValue::Money {
                amount: Decimal::ZERO,
                currency: "USD".to_owned(),
            },
        ),
        ArithmeticResult::Overflow
    );

    assert_eq!(
        PolicyValue::Integer(105)
            .exact_percentage_delta_at_least(&PolicyValue::Integer(100), Decimal::from(5),),
        ArithmeticResult::Decision(true)
    );
    assert_eq!(
        PolicyValue::Integer(i128::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Integer(1), Decimal::ONE,),
        ArithmeticResult::Decision(true)
    );
    assert_eq!(
        PolicyValue::Integer(i128::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Integer(i128::MAX), Decimal::ONE,),
        ArithmeticResult::Decision(false)
    );
    assert_eq!(
        PolicyValue::Integer(i128::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Integer(i128::MIN), Decimal::from(199),),
        ArithmeticResult::Decision(true)
    );
    assert_eq!(
        PolicyValue::Integer(i128::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Integer(i128::MIN), Decimal::from(200),),
        ArithmeticResult::Decision(false)
    );
    assert_eq!(
        PolicyValue::Integer(i128::MAX).exact_percentage_delta_at_least(
            &PolicyValue::Integer(i128::MIN),
            "199.9999999999999999999999999"
                .parse::<Decimal>()
                .expect("fractional percentage"),
        ),
        ArithmeticResult::Decision(true)
    );
    assert_eq!(
        PolicyValue::Integer(i128::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Integer(1), Decimal::ZERO,),
        ArithmeticResult::Decision(true)
    );
    assert_eq!(
        PolicyValue::Decimal(Decimal::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Decimal(-Decimal::MAX), Decimal::ONE,),
        ArithmeticResult::Overflow
    );
    assert_eq!(
        PolicyValue::Decimal(Decimal::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Decimal(Decimal::ONE), Decimal::ONE,),
        ArithmeticResult::Overflow
    );
    assert_eq!(
        PolicyValue::Decimal(Decimal::ONE)
            .exact_percentage_delta_at_least(&PolicyValue::Decimal(Decimal::MAX), Decimal::MAX,),
        ArithmeticResult::Overflow
    );
    assert_eq!(
        PolicyValue::Decimal(Decimal::from(3)).exact_percentage_delta_at_least(
            &PolicyValue::Decimal(Decimal::from(2)),
            Decimal::MAX,
        ),
        ArithmeticResult::Overflow
    );
    assert_eq!(
        dollar_three.exact_percentage_delta_at_least(&dollar, Decimal::from(50)),
        ArithmeticResult::Decision(true)
    );
    assert_eq!(
        dollar_three.exact_percentage_delta_at_least(&euro, Decimal::from(50)),
        ArithmeticResult::Unavailable
    );
    assert_eq!(
        PolicyValue::Integer(105).exact_percentage_delta_at_least(
            &PolicyValue::Decimal(Decimal::from(100)),
            Decimal::from(5)
        ),
        ArithmeticResult::Unavailable
    );
    assert_eq!(
        semver_one.exact_percentage_delta_at_least(&semver_two, Decimal::ONE),
        ArithmeticResult::Unavailable
    );

    assert!(parse_percentage(" ").is_err());
    assert!(parse_percentage("").is_err());
    assert!(parse_percentage("not-a-number").is_err());
    assert!(
        parse_config_value(
            crate::DeclaredType::Money,
            &crate::TypeParams::default(),
            "1"
        )
        .is_err()
    );
}

#[test]
fn policy_staging_keeps_the_three_t0_branches_disjoint_and_side_effect_free() {
    let target = target(
        "integer",
        "",
        &one_condition("kind = \"gt\"\nthreshold = \"1\""),
    );
    let current = observation("integer", "", "2");
    let contexts = context(None, None, None, false);

    assert_eq!(
        target
            .stage_policy_run(
                PolicyRunInput::PermanentContractError {
                    error_code: PermanentErrorCode::InvalidJsonPointer,
                    episode_began: true,
                },
                &contexts,
            )
            .expect("permanent stage"),
        StagedPolicyRun::PermanentContractError {
            error_code: PermanentErrorCode::InvalidJsonPointer,
            event_eligibilities: vec![StagedEventEligibility::OnRun {
                cause: OnRunEventCause::PermanentContractErrorEpisodeBegan {
                    error_code: PermanentErrorCode::InvalidJsonPointer,
                },
            }],
        }
    );
    let continued_permanent = target
        .stage_policy_run(
            PolicyRunInput::PermanentContractError {
                error_code: PermanentErrorCode::InvalidJsonPointer,
                episode_began: false,
            },
            &contexts,
        )
        .expect("continued permanent stage");
    assert!(continued_permanent.event_eligibilities().is_empty());
    let invalid_pointer: TargetDocument = toml::from_str(
        &target_toml("integer", "", "conditions = []")
            .replace("pointer = \"/value\"", "pointer = \"not-a-pointer\""),
    )
    .expect("structurally readable target");
    assert!(invalid_pointer.validate().is_err());
    assert!(matches!(
        invalid_pointer
            .stage_policy_run(
                PolicyRunInput::PermanentContractError {
                    error_code: PermanentErrorCode::InvalidJsonPointer,
                    episode_began: false,
                },
                &BTreeMap::new(),
            )
            .expect("permanent invalid-pointer stage"),
        StagedPolicyRun::PermanentContractError { error_code, .. }
            if error_code == PermanentErrorCode::InvalidJsonPointer
    ));
    assert_eq!(
        target
            .stage_policy_run(
                PolicyRunInput::SourceSuspect {
                    reason_class: SourceSuspectReason::ValueUnparseable,
                    escalation_reached: true,
                },
                &contexts,
            )
            .expect("source stage"),
        StagedPolicyRun::SourceSuspect {
            reason_class: SourceSuspectReason::ValueUnparseable,
            event_eligibilities: vec![StagedEventEligibility::OnRun {
                cause: OnRunEventCause::SourceSuspectEscalated {
                    reason_class: SourceSuspectReason::ValueUnparseable,
                },
            }],
        }
    );
    let pre_escalation_source = target
        .stage_policy_run(
            PolicyRunInput::SourceSuspect {
                reason_class: SourceSuspectReason::ValueUnparseable,
                escalation_reached: false,
            },
            &contexts,
        )
        .expect("pre-escalation source stage");
    assert!(pre_escalation_source.event_eligibilities().is_empty());
    assert_eq!(
        PermanentErrorCode::InvalidJsonPointer.as_str(),
        "invalid_json_pointer"
    );
    assert_eq!(
        SourceSuspectReason::ValueUnparseable.as_str(),
        "value_unparseable"
    );
    assert!(matches!(
        target.stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: &current,
            },
            &contexts,
        )
        .expect("valid stage"),
        StagedPolicyRun::ValidObservation {
            condition_evaluations,
            ..
        } if condition_evaluations[0].outcome() == ConditionOutcome::Satisfied
    ));

    let unknown_context = BTreeMap::from([(
        ConditionId::new("unknown").expect("condition id"),
        ConditionContext::new(None, None, None, false),
    )]);
    assert!(
        target
            .stage_policy_run(
                PolicyRunInput::ValidObservation {
                    observation: &current,
                },
                &unknown_context,
            )
            .is_err()
    );
}

#[test]
fn policy_staging_rejects_current_observations_outside_the_target_contract() {
    let decimal_target = target(
        "decimal",
        "",
        &one_condition("kind = \"lt\"\nthreshold = \"2\""),
    );
    let integer = observation("integer", "", "1");
    assert!(
        decimal_target
            .stage_policy_run(
                PolicyRunInput::ValidObservation {
                    observation: &integer,
                },
                &context(None, None, None, false),
            )
            .is_err()
    );
}

fn target_toml(declared_type: &str, type_params: &str, conditions: &str) -> String {
    format!(
        "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"{declared_type}\"\n{conditions}\n{type_params}\n[target]\nkind = \"file\"\nfile_path = \"/tmp/source.json\"\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n"
    )
}

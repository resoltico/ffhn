//! Focused exact policy-evaluation and predicate scenarios.

mod absent_and_incompatible_contexts_are_unavailable_without_inferred_conversion;
mod bands_apply_directional_hysteresis_without_resetting_on_unavailability;
mod changed_uses_canonical_identity_and_named_references;
mod condition_configuration_validates_predicate_type_thresholds_and_bands;
mod condition_id_is_stable_measurement_local_identity;
mod crossing_refuses_an_unparseable_persisted_predecessor;
mod numeric_delta_predicates_are_exact_and_failure_aware;
mod ordered_crossing_and_level_predicates_have_their_specified_triggers;
mod policy_values_cover_each_typed_comparison_and_exact_arithmetic_path;
mod rejects_observations_outside_the_measurement_contract;
mod support;
mod text_conditions_are_changed_only_with_exact_unicode_identity;

#[test]
fn public_policy_value_objects_expose_the_complete_closed_vocabulary() {
    use std::str::FromStr;

    use super::{ConditionId, ConditionOutcome, ConditionReference, ConditionReferenceEvidence};

    let id = ConditionId::from_str("stable-id").expect("condition id");
    assert_eq!(id.as_ref(), "stable-id");
    assert_eq!(id.to_string(), "stable-id");
    assert_eq!(String::from(id.clone()), "stable-id");
    let serde_id: ConditionId = serde_json::from_str(r#""stable-id""#).expect("serde id");
    assert_eq!(serde_id, id);

    assert_eq!(
        ConditionReference::LastAcceptedObservation.as_str(),
        "last_accepted_observation"
    );
    assert_eq!(
        ConditionReference::FixedInitialBaseline.as_str(),
        "fixed_initial_baseline"
    );
    assert_eq!(
        ConditionReference::LastConditionTransition.as_str(),
        "last_condition_transition"
    );
    assert_eq!(ConditionOutcome::Satisfied.as_str(), "satisfied");
    assert_eq!(ConditionOutcome::NotSatisfied.as_str(), "not_satisfied");
    assert_eq!(ConditionOutcome::Unavailable.as_str(), "unavailable");
    assert_eq!(
        ConditionOutcome::ArithmeticOverflow.as_str(),
        "arithmetic_overflow"
    );
    assert_eq!(ConditionOutcome::ZeroReference.as_str(), "zero_reference");

    let condition: super::Condition = toml::from_str("condition_id = \"stable-id\"\n[predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"\n").expect("condition");
    assert_eq!(condition.condition_id(), "stable-id");
    let measurement = support::measurement(
        "integer",
        "",
        &support::one_condition("kind = \"changed\"\nreference = \"last_accepted_observation\""),
    );
    let current = support::observation("integer", "", "1");
    let evaluation = support::evaluate(
        &measurement,
        &current,
        &support::context(None, None, None, false),
    );
    assert_eq!(evaluation.condition_id(), "condition");
    assert_eq!(evaluation.outcome(), ConditionOutcome::Unavailable);
    assert!(!evaluation.trigger());
    assert!(!evaluation.active_before());
    assert!(!evaluation.active_after());
    assert!(matches!(
        evaluation.reference_evidence(),
        Some(ConditionReferenceEvidence::Unavailable {
            reference: ConditionReference::LastAcceptedObservation
        })
    ));
    let active = support::evaluate(
        &measurement,
        &current,
        &support::context(None, None, None, true),
    );
    assert!(active.active_before());
    assert!(active.active_after());

    let duplicate = vec![condition.clone(), condition];
    assert!(
        super::validate_conditions(
            crate::DeclaredType::Integer,
            &crate::TypeParams::default(),
            &duplicate,
        )
        .is_err()
    );
}

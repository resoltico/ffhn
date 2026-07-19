pub(super) use std::cmp::Ordering;
pub(super) use std::collections::BTreeMap;

pub(super) use super::super::evaluate::{
    ConditionContext, ConditionEvaluation, ConditionIssue, ConditionOutcome, OnRunEventCause,
    PolicyRunInput, StagedEventEligibility, StagedPolicyRun,
};
pub(super) use super::super::value::{
    ArithmeticResult, PolicyValue, parse_config_value, parse_percentage,
};
pub(super) use super::super::*;
pub(super) use crate::{
    ConditionId, IntegrationFaultCode, PermanentErrorCode, SourceSuspectReason, TargetDocument,
};
pub(super) use rust_decimal::Decimal;
pub(super) use semver::Version;
pub(super) use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub(super) fn target(declared_type: &str, type_params: &str, conditions: &str) -> TargetDocument {
    let document: TargetDocument =
        toml::from_str(&target_toml(declared_type, type_params, conditions)).expect("target TOML");
    document.validate().expect("valid target");
    document
}

pub(super) fn observation(declared_type: &str, type_params: &str, raw: &str) -> crate::Observation {
    target(declared_type, type_params, "conditions = []")
        .parse_json_scalar_token(raw.to_owned())
        .expect("valid observation")
}

pub(super) fn one_condition(predicate: &str) -> String {
    format!("[[conditions]]\ncondition_id = \"condition\"\n\n[conditions.predicate]\n{predicate}")
}

pub(super) fn context<'a>(
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

pub(super) fn valid_stage<'a>(
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

pub(super) fn target_toml(declared_type: &str, type_params: &str, conditions: &str) -> String {
    let source_path = crate::test_support::absolute_file_path("source.json");
    format!(
        "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"{declared_type}\"\n{conditions}\n{type_params}\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n"
    )
}

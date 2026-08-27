pub(super) use std::cmp::Ordering;
pub(super) use std::collections::BTreeMap;

pub(super) use super::super::evaluate::{
    ConditionContext, ConditionEvaluation, ConditionOutcome, PolicyContract, evaluate_conditions,
};
pub(super) use super::super::value::{
    ArithmeticResult, PolicyValue, parse_config_value, parse_percentage,
};
pub(super) use super::super::*;
pub(super) use crate::ConditionId;
pub(super) use rust_decimal::Decimal;
pub(super) use semver::Version;
pub(super) use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MeasurementPolicyContract {
    declared_type: crate::DeclaredType,
    #[serde(default)]
    type_params: crate::TypeParams,
    conditions: Vec<Condition>,
}

impl MeasurementPolicyContract {
    pub(super) const fn declared_type(&self) -> crate::DeclaredType {
        self.declared_type
    }

    pub(super) const fn type_params(&self) -> &crate::TypeParams {
        &self.type_params
    }

    pub(super) fn conditions(&self) -> &[Condition] {
        &self.conditions
    }

    pub(super) fn validate(&self) -> Result<(), crate::CoreError> {
        crate::model::validate_type_params(self.declared_type, &self.type_params)?;
        validate_conditions(self.declared_type, &self.type_params, &self.conditions)
    }
}

pub(super) fn decode_measurement(
    document: &str,
) -> Result<MeasurementPolicyContract, crate::CoreError> {
    let contract: MeasurementPolicyContract = toml::from_str(document)?;
    contract.validate()?;
    Ok(contract)
}

pub(super) fn measurement(
    declared_type: &str,
    type_params: &str,
    conditions: &str,
) -> MeasurementPolicyContract {
    decode_measurement(&measurement_toml(declared_type, type_params, conditions))
        .expect("measurement policy contract")
}

pub(super) fn observation(declared_type: &str, type_params: &str, raw: &str) -> crate::Observation {
    let measurement = measurement(declared_type, type_params, "conditions = []");
    crate::model::parse_json_scalar_token_for_contract(
        measurement.declared_type(),
        measurement.type_params(),
        raw.to_owned(),
    )
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

pub(super) fn evaluate<'a>(
    measurement: &MeasurementPolicyContract,
    current: &'a crate::Observation,
    contexts: &BTreeMap<ConditionId, ConditionContext<'a>>,
) -> ConditionEvaluation {
    let contract = PolicyContract::new(
        measurement.declared_type(),
        measurement.type_params(),
        measurement.conditions(),
    );
    let mut evaluations = evaluate_conditions(&contract, current, contexts).expect("evaluation");
    assert_eq!(evaluations.len(), 1);
    evaluations.remove(0)
}

pub(super) fn measurement_toml(declared_type: &str, type_params: &str, conditions: &str) -> String {
    format!("declared_type = \"{declared_type}\"\n{conditions}\n{type_params}\n")
}

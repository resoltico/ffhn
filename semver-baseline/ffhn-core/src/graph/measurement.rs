//! Measurement-owned v11 projection and value configuration.

use serde::{Deserialize, Deserializer, Serialize};

use crate::{Condition, ConditionPredicate, CoreError, DeclaredType, Projection, TypeParams};

use super::{
    DeliveryPolicy, GraphRoute, GraphRouteFamily, MeasurementId, SourceDocument,
    delivery_config::validate_delivery,
};

/// Canonical measurement configuration schema name.
pub const MEASUREMENT_SCHEMA_NAME: &str = "ffhn.measurement";
/// Canonical measurement configuration schema version.
pub const MEASUREMENT_SCHEMA_VERSION: u32 = 1;
/// Version of source-to-typed-value acquisition semantics bound into every MVD.
pub const MEASUREMENT_VALUE_SEMANTICS_VERSION: u32 = 1;
/// Version of condition-decision semantics bound into per-condition definition digests.
pub const MEASUREMENT_POLICY_SEMANTICS_VERSION: u32 = 1;

/// One scalar projection and its type contract, independently owned by a source measurement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MeasurementDocument {
    schema_name: String,
    schema_version: u32,
    measurement_id: MeasurementId,
    display_name: String,
    enabled: bool,
    escalate_after: u32,
    projection: Projection,
    declared_type: DeclaredType,
    type_params: TypeParams,
    conditions: Vec<Condition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outbox: Option<DeliveryPolicy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    routes: Vec<GraphRoute>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasurementDocumentWire {
    schema_name: String,
    schema_version: u32,
    measurement_id: MeasurementId,
    display_name: String,
    enabled: bool,
    escalate_after: u32,
    projection: Projection,
    declared_type: DeclaredType,
    #[serde(default)]
    type_params: TypeParams,
    conditions: Vec<Condition>,
    #[serde(default)]
    outbox: Option<DeliveryPolicy>,
    #[serde(default)]
    routes: Vec<GraphRoute>,
}

impl<'de> Deserialize<'de> for MeasurementDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = MeasurementDocumentWire::deserialize(deserializer)?;
        let document = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            measurement_id: wire.measurement_id,
            display_name: wire.display_name,
            enabled: wire.enabled,
            escalate_after: wire.escalate_after,
            projection: wire.projection,
            declared_type: wire.declared_type,
            type_params: wire.type_params,
            conditions: wire.conditions,
            outbox: wire.outbox,
            routes: wire.routes,
        };
        document.validate().map_err(serde::de::Error::custom)?;
        Ok(document)
    }
}

impl MeasurementDocument {
    /// Returns the measurement directory identifier.
    pub fn measurement_id(&self) -> &MeasurementId {
        &self.measurement_id
    }

    /// Returns the user-visible measurement display name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns whether this measurement is eligible for projection.
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the measurement extraction-health escalation threshold.
    pub const fn escalate_after(&self) -> u32 {
        self.escalate_after
    }

    /// Returns the scalar projection definition.
    pub const fn projection(&self) -> &Projection {
        &self.projection
    }

    /// Returns the declared typed-value family.
    pub const fn declared_type(&self) -> DeclaredType {
        self.declared_type
    }

    /// Returns the declared type parameters used by typed parsing and policy evaluation.
    pub const fn type_params(&self) -> &TypeParams {
        &self.type_params
    }

    /// Returns condition definitions in their declared evaluation and admission order.
    pub fn conditions(&self) -> &[Condition] {
        &self.conditions
    }

    /// Returns the measurement-owned delivery policy when measurement routing is enabled.
    pub const fn outbox(&self) -> Option<&DeliveryPolicy> {
        self.outbox.as_ref()
    }

    /// Returns measurement routes in declared admission order.
    pub fn routes(&self) -> &[GraphRoute] {
        &self.routes
    }

    /// Computes a stable policy-definition digest for each declared condition.
    pub fn condition_definition_digests(&self) -> Result<Vec<(String, String)>, CoreError> {
        #[derive(Serialize)]
        struct Digest {
            condition: serde_json::Value,
            policy_evaluation_semantics_version: u32,
        }
        self.conditions
            .iter()
            .map(|condition| {
                let condition_value =
                    normalized_condition_value(condition, self.declared_type, &self.type_params)?;
                crate::stable_json::stable_digest(&Digest {
                    condition: condition_value,
                    policy_evaluation_semantics_version: MEASUREMENT_POLICY_SEMANTICS_VERSION,
                })
                .map(|digest| (condition.condition_id().to_owned(), digest))
            })
            .collect()
    }

    /// Computes the measurement value digest from source representation and typed-value semantics.
    pub fn measurement_value_digest(&self, source: &SourceDocument) -> Result<String, CoreError> {
        self.validate()?;
        #[derive(Serialize)]
        struct Digest<'a> {
            source_representation_digest: String,
            projection: &'a Projection,
            declared_type: DeclaredType,
            type_params: &'a TypeParams,
            parser_id: &'static str,
            parser_grammar_version: u32,
            acquisition_semantics_version: u32,
            htmlcut_extraction_semantics_version: Option<u32>,
        }
        let htmlcut_extraction_semantics_version = matches!(
            self.projection,
            Projection::HtmlText { .. }
                | Projection::HtmlRenderedText { .. }
                | Projection::HtmlAttribute { .. }
        )
        .then_some(htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION);
        crate::stable_json::stable_digest(&Digest {
            source_representation_digest: source.source_representation_digest()?,
            projection: &self.projection,
            declared_type: self.declared_type,
            type_params: &self.type_params,
            parser_id: crate::PARSER_ID,
            parser_grammar_version: crate::PARSER_GRAMMAR_VERSION,
            acquisition_semantics_version: MEASUREMENT_VALUE_SEMANTICS_VERSION,
            htmlcut_extraction_semantics_version,
        })
    }

    /// Validates the measurement envelope and source-independent scalar contract facts.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_name != MEASUREMENT_SCHEMA_NAME
            || self.schema_version != MEASUREMENT_SCHEMA_VERSION
        {
            return Err(CoreError::contract(
                "measurement document is not a current FFHN measurement document",
            ));
        }
        if self.display_name.trim().is_empty() {
            return Err(CoreError::contract(
                "measurement.display_name must not be blank",
            ));
        }
        if self.escalate_after == 0 {
            return Err(CoreError::contract(
                "measurement.escalate_after must be positive",
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        if self
            .conditions
            .iter()
            .any(|condition| !ids.insert(condition.id().clone()))
        {
            return Err(CoreError::contract(
                "measurement condition ids must be unique",
            ));
        }
        crate::model::validate_type_params(self.declared_type, &self.type_params)?;
        crate::model::policy::validate_conditions(
            self.declared_type,
            &self.type_params,
            &self.conditions,
        )?;
        validate_delivery(
            self.outbox.as_ref(),
            &self.routes,
            &[
                GraphRouteFamily::OnCondition,
                GraphRouteFamily::OnMeasurement,
            ],
        )
    }
}

fn normalized_condition_value(
    condition: &Condition,
    declared_type: DeclaredType,
    type_params: &TypeParams,
) -> Result<serde_json::Value, CoreError> {
    let mut value = serde_json::to_value(condition)?;
    match condition.predicate() {
        ConditionPredicate::Changed { .. } => {}
        ConditionPredicate::DeltaPct { threshold, .. } => {
            let canonical = crate::model::policy::parse_percentage(threshold)
                .map_err(CoreError::contract)?
                .normalize()
                .to_string();
            set_predicate_field(&mut value, "threshold", canonical)?;
        }
        ConditionPredicate::DeltaAbs { threshold, .. }
        | ConditionPredicate::Crosses { threshold, .. }
        | ConditionPredicate::Lt { threshold }
        | ConditionPredicate::Gt { threshold } => normalize_typed_field(
            &mut value,
            "threshold",
            threshold,
            declared_type,
            type_params,
        )?,
        ConditionPredicate::Band {
            enter_threshold,
            exit_threshold,
            ..
        } => {
            normalize_typed_field(
                &mut value,
                "enter_threshold",
                enter_threshold,
                declared_type,
                type_params,
            )?;
            normalize_typed_field(
                &mut value,
                "exit_threshold",
                exit_threshold,
                declared_type,
                type_params,
            )?;
        }
    }
    Ok(value)
}

fn normalize_typed_field(
    condition: &mut serde_json::Value,
    field: &str,
    raw: &str,
    declared_type: DeclaredType,
    type_params: &TypeParams,
) -> Result<(), CoreError> {
    let canonical = crate::model::policy::canonical_config_value(declared_type, type_params, raw)
        .map_err(CoreError::contract)?;
    set_predicate_field(condition, field, canonical)
}

fn set_predicate_field(
    condition: &mut serde_json::Value,
    field: &str,
    value: String,
) -> Result<(), CoreError> {
    let predicate = condition
        .get_mut("predicate")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| CoreError::internal("condition serialization omitted its predicate"))?;
    let slot = predicate.get_mut(field).ok_or_else(|| {
        CoreError::internal("condition serialization omitted a normalized threshold field")
    })?;
    *slot = serde_json::Value::String(value);
    Ok(())
}

#[cfg(test)]
#[path = "measurement/tests.rs"]
mod tests;

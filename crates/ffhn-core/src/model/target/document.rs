//! Target aggregate behavior: validation coordination, policy staging, parsing, and digests.

use std::collections::BTreeMap;

use htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION;
use serde::Serialize;

use crate::CoreError;

use super::super::delivery::validate_routes;
use super::super::observation::parse::{parse_html_projection, parse_json_scalar_token};
use super::super::observation::{HtmlObservationInput, PARSER_GRAMMAR_VERSION, PARSER_ID};
use super::super::policy::{
    Condition, ConditionContext, ConditionId, POLICY_EVALUATION_SEMANTICS_VERSION, PolicyRunInput,
    StagedPolicyRun, stage_policy_run, validate_conditions,
};
use super::super::{DeliveryRoute, OutboxPolicy, RouteFamily, RouteId};
use super::schema::{
    DeclaredType, FetchConfig, PermanentTargetError, Projection, TARGET_SCHEMA_NAME,
    TARGET_SCHEMA_VERSION, TargetDocument, TargetSource, TypeParams,
};
use super::validation::{
    htmlcut_input_permanent_error, projection_permanent_error, require_text, validate_fetch,
    validate_source, validate_type_params,
};
use crate::{DiagnosticDetail, Observation};

impl TargetDocument {
    /// Validates the target as one complete measurement contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        self.validate_without_projection()?;
        if let Some(error) = self.permanent_error()? {
            return Err(CoreError::contract(error.message));
        }
        Ok(())
    }

    /// Validates every target requirement other than projection syntax.
    ///
    /// Runtime classification uses this boundary to persist a permanent invalid-JSON-Pointer
    /// episode while still refusing all other malformed target contracts before execution.
    pub(crate) fn validate_without_projection(&self) -> Result<(), CoreError> {
        if self.schema_name != TARGET_SCHEMA_NAME || self.schema_version != TARGET_SCHEMA_VERSION {
            return Err(CoreError::contract(format!(
                "target must use schema_name = {TARGET_SCHEMA_NAME:?} and schema_version = {TARGET_SCHEMA_VERSION}"
            )));
        }
        require_text("display_name", &self.display_name)?;
        if self.escalate_after == 0 {
            return Err(CoreError::contract("escalate_after must be positive"));
        }
        validate_source(&self.target)?;
        validate_fetch(&self.target, &self.fetch)?;
        validate_type_params(self.declared_type, &self.type_params)?;
        validate_conditions(self.declared_type, &self.type_params, &self.conditions)?;
        self.outbox.validate()?;
        validate_routes(&self.routes)
    }

    /// Returns the stable permanent projection error and its public diagnostic when present.
    pub(crate) fn permanent_error(&self) -> Result<Option<PermanentTargetError>, CoreError> {
        match htmlcut_input_permanent_error(&self.target, &self.projection) {
            Some(error) => Ok(Some(error)),
            None => projection_permanent_error(&self.projection),
        }
    }

    /// Returns whether a persisted observation belongs to this target's type contract.
    pub(crate) fn observation_matches(&self, observation: &Observation) -> bool {
        observation.declared_type_for_policy() == self.declared_type
            && observation.type_params_for_policy() == &self.type_params
    }

    /// Returns the configured target id.
    pub fn target_id(&self) -> &str {
        self.target_id.as_str()
    }
    /// Returns the display name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    /// Returns whether live runs are enabled.
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
    /// Returns the number of consecutive source-suspect runs that reaches escalation.
    pub const fn escalate_after(&self) -> u32 {
        self.escalate_after
    }
    /// Returns the configured source.
    pub const fn source(&self) -> &TargetSource {
        &self.target
    }
    /// Returns the fetch policy.
    pub const fn fetch(&self) -> &FetchConfig {
        &self.fetch
    }
    /// Returns the acquisition projection.
    pub const fn projection(&self) -> &Projection {
        &self.projection
    }
    /// Returns the declared type.
    pub const fn declared_type(&self) -> DeclaredType {
        self.declared_type
    }
    /// Returns the declared type parameters.
    pub const fn type_params(&self) -> &TypeParams {
        &self.type_params
    }
    /// Returns all named conditions in target declaration order.
    pub fn conditions(&self) -> &[Condition] {
        &self.conditions
    }
    /// Returns the operational outbox policy, which does not affect measurement identity.
    pub const fn outbox(&self) -> &OutboxPolicy {
        &self.outbox
    }
    /// Returns configured delivery routes in target declaration order.
    pub fn routes(&self) -> &[DeliveryRoute] {
        &self.routes
    }

    pub(crate) fn routes_for(&self, family: RouteFamily) -> impl Iterator<Item = &DeliveryRoute> {
        self.routes
            .iter()
            .filter(move |route| route.route_family() == family)
    }

    pub(crate) fn route(&self, id: &RouteId) -> Option<&DeliveryRoute> {
        self.routes.iter().find(|route| route.id() == id)
    }

    pub(crate) fn condition(&self, id: &ConditionId) -> Option<&Condition> {
        self.conditions
            .iter()
            .find(|condition| condition.id() == id)
    }

    /// Stages one classified policy branch without persisting state or delivering events.
    ///
    /// The target and current observation are validated, and the observation must use this
    /// target's declared type and type parameters. Contexts are keyed by the stable typed
    /// [`ConditionId`] and may contain only conditions declared by this target. Failure inputs
    /// carry the state-owned episode transition that determines whether an immediate `on_run`
    /// event is eligible. A later temporal coordinator owns persistence of the staged active
    /// state and references, while M4 materializes these eligibilities into durable delivery.
    pub fn stage_policy_run<'a>(
        &self,
        input: PolicyRunInput<'a>,
        contexts: &BTreeMap<ConditionId, ConditionContext<'a>>,
    ) -> Result<StagedPolicyRun, CoreError> {
        stage_policy_run(self, input, contexts)
    }

    /// Computes the source-kind-specific measurement contract digest.
    pub fn contract_digest_sha256(&self) -> Result<String, CoreError> {
        self.contract_digest_sha256_with_semantics_versions(
            POLICY_EVALUATION_SEMANTICS_VERSION,
            HTMLCUT_EXTRACTION_SEMANTICS_VERSION,
        )
    }

    /// Computes a digest under supplied semantic versions for cross-version contract tests.
    #[cfg(test)]
    pub(crate) fn contract_digest_sha256_with_semantics_versions_for_test(
        &self,
        policy_evaluation_semantics_version: u32,
        htmlcut_extraction_semantics_version: u32,
    ) -> Result<String, CoreError> {
        self.contract_digest_sha256_with_semantics_versions(
            policy_evaluation_semantics_version,
            htmlcut_extraction_semantics_version,
        )
    }

    fn contract_digest_sha256_with_semantics_versions(
        &self,
        policy_evaluation_semantics_version: u32,
        htmlcut_extraction_semantics_version: u32,
    ) -> Result<String, CoreError> {
        // Condition order is operational delivery-admission priority, not measurement identity.
        // Canonicalizing this digest input lets authors reprioritize bounded-queue admission
        // without invalidating typed observations or temporal policy state.
        let mut contract_conditions = self.conditions.clone();
        contract_conditions.sort_unstable_by(|left, right| left.id().cmp(right.id()));

        #[derive(Serialize)]
        struct JsonContract<'a> {
            source_kind: &'static str,
            target: &'a TargetSource,
            fetch: &'a FetchConfig,
            projection: &'a Projection,
            declared_type: DeclaredType,
            parser_id: &'static str,
            parser_grammar_version: u32,
            type_params: &'a TypeParams,
            conditions: &'a [Condition],
            escalate_after: u32,
            policy_evaluation_semantics_version: u32,
        }
        #[derive(Serialize)]
        struct HtmlContract<'a> {
            source_kind: &'static str,
            target: &'a TargetSource,
            fetch: &'a FetchConfig,
            projection: &'a Projection,
            declared_type: DeclaredType,
            parser_id: &'static str,
            parser_grammar_version: u32,
            type_params: &'a TypeParams,
            conditions: &'a [Condition],
            escalate_after: u32,
            policy_evaluation_semantics_version: u32,
            htmlcut_extraction_semantics_version: u32,
        }
        match self.projection {
            Projection::JsonPointer { .. } => crate::stable_json::stable_digest(&JsonContract {
                source_kind: "json_pointer",
                target: &self.target,
                fetch: &self.fetch,
                projection: &self.projection,
                declared_type: self.declared_type,
                parser_id: PARSER_ID,
                parser_grammar_version: PARSER_GRAMMAR_VERSION,
                type_params: &self.type_params,
                conditions: &contract_conditions,
                escalate_after: self.escalate_after,
                policy_evaluation_semantics_version,
            }),
            Projection::HtmlText { .. } => crate::stable_json::stable_digest(&HtmlContract {
                source_kind: "html_text",
                target: &self.target,
                fetch: &self.fetch,
                projection: &self.projection,
                declared_type: self.declared_type,
                parser_id: PARSER_ID,
                parser_grammar_version: PARSER_GRAMMAR_VERSION,
                type_params: &self.type_params,
                conditions: &contract_conditions,
                escalate_after: self.escalate_after,
                policy_evaluation_semantics_version,
                htmlcut_extraction_semantics_version,
            }),
            Projection::HtmlRenderedText { .. } => {
                crate::stable_json::stable_digest(&HtmlContract {
                    source_kind: "html_rendered_text",
                    target: &self.target,
                    fetch: &self.fetch,
                    projection: &self.projection,
                    declared_type: self.declared_type,
                    parser_id: PARSER_ID,
                    parser_grammar_version: PARSER_GRAMMAR_VERSION,
                    type_params: &self.type_params,
                    conditions: &contract_conditions,
                    escalate_after: self.escalate_after,
                    policy_evaluation_semantics_version,
                    htmlcut_extraction_semantics_version,
                })
            }
            Projection::HtmlAttribute { .. } => crate::stable_json::stable_digest(&HtmlContract {
                source_kind: "html_attribute",
                target: &self.target,
                fetch: &self.fetch,
                projection: &self.projection,
                declared_type: self.declared_type,
                parser_id: PARSER_ID,
                parser_grammar_version: PARSER_GRAMMAR_VERSION,
                type_params: &self.type_params,
                conditions: &contract_conditions,
                escalate_after: self.escalate_after,
                policy_evaluation_semantics_version,
                htmlcut_extraction_semantics_version,
            }),
        }
    }

    /// Parses one raw JSON scalar token into a valid persisted observation.
    pub(crate) fn parse_json_scalar_token(
        &self,
        raw_selected: String,
    ) -> Result<Observation, DiagnosticDetail> {
        parse_json_scalar_token(self, raw_selected)
    }

    /// Parses one HTMLCut projection into a valid persisted observation.
    pub(crate) fn parse_html_projection(
        &self,
        input: HtmlObservationInput,
    ) -> Result<Observation, DiagnosticDetail> {
        parse_html_projection(self, input)
    }
}

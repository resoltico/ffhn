//! Measurement-owned accepted-observation and independently rebased policy state.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    ConditionContext, ConditionEvaluation, ConditionId, ConditionOutcome, CoreError, Observation,
};

use super::super::{
    ExtractionFailureReason, GraphIntegrationFaultCode, IntegrationFaultEpisode,
    MeasurementDocument, MeasurementExtractionHealth, MeasurementInstanceId, OutboxOverflowFact,
};
use super::{SourceInstanceId, validate_sha256};

#[path = "measurement_state/access.rs"]
mod access;

/// Canonical measurement-state schema name.
pub const MEASUREMENT_STATE_SCHEMA_NAME: &str = "ffhn.measurement_state";
/// Canonical measurement-state schema version.
pub const MEASUREMENT_STATE_SCHEMA_VERSION: u32 = 1;

/// Per-measurement durable state bearing both authoritative lineage stamps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MeasurementState {
    schema_name: String,
    schema_version: u32,
    source_instance_id: SourceInstanceId,
    measurement_instance_id: MeasurementInstanceId,
    measurement_episode_seq: u64,
    observation_seq: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    accepted_observation: Option<Observation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    measurement_value_digest: Option<String>,
    condition_state: BTreeMap<ConditionId, MeasurementConditionState>,
    policy_revision: u64,
    extraction_health: MeasurementExtractionHealth,
    outbox_overflow: Vec<OutboxOverflowFact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_fault_episode: Option<IntegrationFaultEpisode>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasurementStateWire {
    schema_name: String,
    schema_version: u32,
    source_instance_id: SourceInstanceId,
    measurement_instance_id: MeasurementInstanceId,
    measurement_episode_seq: u64,
    observation_seq: u64,
    #[serde(default)]
    accepted_observation: Option<Observation>,
    #[serde(default)]
    measurement_value_digest: Option<String>,
    #[serde(default)]
    condition_state: BTreeMap<ConditionId, MeasurementConditionState>,
    #[serde(default)]
    policy_revision: u64,
    #[serde(default = "MeasurementExtractionHealth::healthy")]
    extraction_health: MeasurementExtractionHealth,
    outbox_overflow: Vec<OutboxOverflowFact>,
    #[serde(default)]
    integration_fault_episode: Option<IntegrationFaultEpisode>,
}

/// Per-condition temporal state that is rebased independently of accepted observations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasurementConditionState {
    definition_digest: String,
    result: ConditionOutcome,
    active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    fixed_initial_baseline: Option<Observation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_condition_transition: Option<Observation>,
}
impl<'de> Deserialize<'de> for MeasurementState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = MeasurementStateWire::deserialize(deserializer)?;
        let state = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            source_instance_id: wire.source_instance_id,
            measurement_instance_id: wire.measurement_instance_id,
            measurement_episode_seq: wire.measurement_episode_seq,
            observation_seq: wire.observation_seq,
            accepted_observation: wire.accepted_observation,
            measurement_value_digest: wire.measurement_value_digest,
            condition_state: wire.condition_state,
            policy_revision: wire.policy_revision,
            extraction_health: wire.extraction_health,
            outbox_overflow: wire.outbox_overflow,
            integration_fault_episode: wire.integration_fault_episode,
        };
        state.validate().map_err(serde::de::Error::custom)?;
        Ok(state)
    }
}
impl MeasurementState {
    /// Creates the fresh state document for one measurement lineage.
    pub fn fresh(
        source_instance_id: SourceInstanceId,
        measurement_instance_id: MeasurementInstanceId,
    ) -> Self {
        Self {
            schema_name: MEASUREMENT_STATE_SCHEMA_NAME.to_owned(),
            schema_version: MEASUREMENT_STATE_SCHEMA_VERSION,
            source_instance_id,
            measurement_instance_id,
            measurement_episode_seq: 0,
            observation_seq: 0,
            accepted_observation: None,
            measurement_value_digest: None,
            condition_state: BTreeMap::new(),
            policy_revision: 0,
            extraction_health: MeasurementExtractionHealth::healthy(),
            outbox_overflow: Vec::new(),
            integration_fault_episode: None,
        }
    }

    /// Validates the closed measurement-state schema and both lineage stamps.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_name != MEASUREMENT_STATE_SCHEMA_NAME
            || self.schema_version != MEASUREMENT_STATE_SCHEMA_VERSION
        {
            return Err(CoreError::contract(
                "measurement state is not a current FFHN measurement state",
            ));
        }
        self.source_instance_id.validate()?;
        self.measurement_instance_id.validate()?;
        match (&self.accepted_observation, &self.measurement_value_digest) {
            (None, None) if self.observation_seq == 0 => {}
            (Some(observation), Some(digest)) if self.observation_seq > 0 => {
                observation.validate()?;
                validate_sha256("measurement value digest", digest)?;
            }
            (None | Some(_), None | Some(_)) => {
                return Err(CoreError::contract(
                    "measurement accepted observation, sequence, and value digest disagree",
                ));
            }
        }
        for state in self.condition_state.values() {
            state.validate()?;
        }
        self.extraction_health.validate()?;
        for overflow in &self.outbox_overflow {
            overflow.validate()?;
        }
        if let Some(episode) = &self.integration_fault_episode {
            episode.validate()?;
            if episode.code() == GraphIntegrationFaultCode::SecretUnavailable {
                return Err(CoreError::contract(
                    "measurement state must not contain a source-scoped integration fault",
                ));
            }
        }
        Ok(())
    }

    /// Validates all accepted and condition-reference observations against a live measurement
    /// value contract without allowing policy edits to invalidate the observations themselves.
    pub fn validate_for_measurement(
        &self,
        measurement: &MeasurementDocument,
    ) -> Result<(), CoreError> {
        self.validate()?;
        let matches = |observation: &Observation| {
            observation
                .matches_type_contract(measurement.declared_type(), measurement.type_params())
        };
        if self
            .accepted_observation
            .as_ref()
            .is_some_and(|value| !matches(value))
            || self.condition_state.values().any(|state| {
                state
                    .fixed_initial_baseline
                    .as_ref()
                    .is_some_and(|value| !matches(value))
                    || state
                        .last_condition_transition
                        .as_ref()
                        .is_some_and(|value| !matches(value))
            })
        {
            return Err(CoreError::contract(
                "measurement state contains an observation outside the measurement value contract",
            ));
        }
        Ok(())
    }

    /// Reconciles independently versioned condition definitions without changing observations.
    ///
    /// Unchanged definitions retain every temporal fact. Changed and new definitions are seeded
    /// from the latest accepted observation and emit nothing; removed definitions disappear from
    /// temporal state while any already queued event remains owned by its outbox snapshot.
    pub fn rebase_policy(&mut self, measurement: &MeasurementDocument) -> Result<(), CoreError> {
        self.validate_for_measurement(measurement)?;
        let definition_digests = measurement
            .condition_definition_digests()?
            .into_iter()
            .map(|(id, digest)| ConditionId::new(id).map(|id| (id, digest)))
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let mut next = BTreeMap::new();
        for condition in measurement.conditions() {
            let id = ConditionId::new(condition.condition_id())?;
            let digest = required_definition_digest(&definition_digests, &id)?;
            let state = match self.condition_state.get(&id) {
                Some(current) if current.definition_digest == *digest => current.clone(),
                Some(_) | None => self.rebased_condition_state(measurement, &id, digest)?,
            };
            next.insert(id, state);
        }
        if next != self.condition_state {
            self.policy_revision = self
                .policy_revision
                .checked_add(1)
                .ok_or_else(|| CoreError::contract("measurement policy revision overflowed"))?;
            self.condition_state = next;
        }
        self.validate_for_measurement(measurement)
    }

    /// Evaluates and records one accepted observation after policy rebasing, returning the staged
    /// decisions for event admission. It never performs delivery or creates an event itself.
    pub fn apply_accepted_observation(
        &mut self,
        measurement: &MeasurementDocument,
        observation: Observation,
        measurement_value_digest: String,
    ) -> Result<Vec<ConditionEvaluation>, CoreError> {
        validate_sha256("measurement value digest", &measurement_value_digest)?;
        if self
            .measurement_value_digest
            .as_deref()
            .is_some_and(|stored| stored != measurement_value_digest)
        {
            return Err(CoreError::contract(
                "measurement state belongs to a different measurement value digest",
            ));
        }
        self.rebase_policy(measurement)?;
        if !observation
            .matches_type_contract(measurement.declared_type(), measurement.type_params())
        {
            return Err(CoreError::contract(
                "accepted observation does not match the measurement value contract",
            ));
        }
        let contexts = measurement
            .conditions()
            .iter()
            .map(|condition| {
                let id = ConditionId::new(condition.condition_id())?;
                let state = required_condition_state(&self.condition_state, &id)?;
                Ok((
                    id,
                    ConditionContext::new(
                        self.accepted_observation.as_ref(),
                        state.fixed_initial_baseline.as_ref(),
                        state.last_condition_transition.as_ref(),
                        state.active,
                    ),
                ))
            })
            .collect::<Result<BTreeMap<_, _>, CoreError>>()?;
        let contract = crate::model::policy::PolicyContract::new(
            measurement.declared_type(),
            measurement.type_params(),
            measurement.conditions(),
        );
        let evaluations =
            crate::model::policy::evaluate_conditions(&contract, &observation, &contexts)?;
        let expected = measurement
            .conditions()
            .iter()
            .map(|condition| ConditionId::new(condition.condition_id()))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let observed = evaluations
            .iter()
            .map(|evaluation| ConditionId::new(evaluation.condition_id()))
            .collect::<Result<BTreeSet<_>, _>>()?;
        require_exact_evaluation_ids(&expected, &observed)?;
        for evaluation in &evaluations {
            let id = ConditionId::new(evaluation.condition_id())?;
            let state = required_condition_state_mut(&mut self.condition_state, &id)?;
            if state.result != evaluation.outcome() {
                state.last_condition_transition = Some(observation.clone());
            }
            state.result = evaluation.outcome();
            state.active = evaluation.active_after();
            if state.fixed_initial_baseline.is_none() {
                state.fixed_initial_baseline = Some(observation.clone());
            }
        }
        self.observation_seq = self
            .observation_seq
            .checked_add(1)
            .ok_or_else(|| CoreError::contract("measurement observation sequence overflowed"))?;
        self.accepted_observation = Some(observation);
        self.measurement_value_digest = Some(measurement_value_digest);
        self.extraction_health.clear();
        self.integration_fault_episode = None;
        self.validate_for_measurement(measurement)?;
        Ok(evaluations)
    }

    /// Records one measurement-local extraction failure and returns whether it just escalated.
    pub fn apply_extraction_failure(
        &mut self,
        reason: ExtractionFailureReason,
        now_utc: &str,
        escalate_after: u32,
    ) -> Result<bool, CoreError> {
        let episode_began = self.extraction_health.reason() != Some(reason);
        let mut extraction_health = self.extraction_health.clone();
        let escalated = extraction_health.observe(reason, now_utc, escalate_after)?;
        let measurement_episode_seq = if episode_began {
            self.measurement_episode_seq
                .checked_add(1)
                .ok_or_else(|| CoreError::contract("measurement episode sequence overflowed"))?
        } else {
            self.measurement_episode_seq
        };
        self.extraction_health = extraction_health;
        self.measurement_episode_seq = measurement_episode_seq;
        self.integration_fault_episode = None;
        self.validate()?;
        Ok(escalated)
    }

    /// Records one measurement-scoped integration fault and returns whether its episode began.
    pub fn apply_measurement_integration_fault(
        &mut self,
        code: GraphIntegrationFaultCode,
        now_utc: String,
    ) -> Result<bool, CoreError> {
        if code == GraphIntegrationFaultCode::SecretUnavailable {
            return Err(CoreError::contract(
                "secret_unavailable is a source-scoped integration fault",
            ));
        }
        if self
            .integration_fault_episode
            .as_ref()
            .is_some_and(|episode| episode.code() == code)
        {
            return Ok(false);
        }
        let measurement_episode_seq = self
            .measurement_episode_seq
            .checked_add(1)
            .ok_or_else(|| CoreError::contract("measurement episode sequence overflowed"))?;
        let episode = IntegrationFaultEpisode::new(code, now_utc)?;
        self.measurement_episode_seq = measurement_episode_seq;
        self.integration_fault_episode = Some(episode);
        self.validate()?;
        Ok(true)
    }

    fn rebased_condition_state(
        &self,
        measurement: &MeasurementDocument,
        condition_id: &ConditionId,
        definition_digest: &str,
    ) -> Result<MeasurementConditionState, CoreError> {
        let Some(observation) = self.accepted_observation.as_ref() else {
            return Ok(MeasurementConditionState {
                definition_digest: definition_digest.to_owned(),
                result: ConditionOutcome::NotSatisfied,
                active: false,
                fixed_initial_baseline: None,
                last_condition_transition: None,
            });
        };
        let condition = measurement
            .conditions()
            .iter()
            .find(|condition| condition.condition_id() == condition_id.as_str())
            .ok_or_else(|| CoreError::internal("rebasing requested an undeclared condition"))?;
        let contexts = BTreeMap::from([(
            condition_id.clone(),
            ConditionContext::new(
                Some(observation),
                Some(observation),
                Some(observation),
                false,
            ),
        )]);
        let contract = crate::model::policy::PolicyContract::new(
            measurement.declared_type(),
            measurement.type_params(),
            std::slice::from_ref(condition),
        );
        let evaluation =
            crate::model::policy::evaluate_conditions(&contract, observation, &contexts)?
                .into_iter()
                .next()
                .ok_or_else(|| CoreError::internal("rebasing did not evaluate its condition"))?;
        Ok(MeasurementConditionState {
            definition_digest: definition_digest.to_owned(),
            result: evaluation.outcome(),
            active: evaluation.active_after(),
            fixed_initial_baseline: Some(observation.clone()),
            last_condition_transition: Some(observation.clone()),
        })
    }
}

fn required_definition_digest<'a>(
    digests: &'a BTreeMap<ConditionId, String>,
    id: &ConditionId,
) -> Result<&'a String, CoreError> {
    digests.get(id).ok_or_else(|| {
        CoreError::internal("condition digest calculation omitted a declared condition")
    })
}

fn required_condition_state<'a>(
    states: &'a BTreeMap<ConditionId, MeasurementConditionState>,
    id: &ConditionId,
) -> Result<&'a MeasurementConditionState, CoreError> {
    states
        .get(id)
        .ok_or_else(|| CoreError::internal("policy rebasing omitted a declared condition"))
}

fn required_condition_state_mut<'a>(
    states: &'a mut BTreeMap<ConditionId, MeasurementConditionState>,
    id: &ConditionId,
) -> Result<&'a mut MeasurementConditionState, CoreError> {
    states
        .get_mut(id)
        .ok_or_else(|| CoreError::internal("policy evaluation produced an unknown condition"))
}

fn require_exact_evaluation_ids(
    expected: &BTreeSet<ConditionId>,
    observed: &BTreeSet<ConditionId>,
) -> Result<(), CoreError> {
    if expected == observed {
        Ok(())
    } else {
        Err(CoreError::internal(
            "policy evaluation did not produce exactly one result for every declared condition",
        ))
    }
}
impl MeasurementConditionState {
    fn validate(&self) -> Result<(), CoreError> {
        validate_sha256("condition definition digest", &self.definition_digest)?;
        if let Some(observation) = &self.fixed_initial_baseline {
            observation.validate()?;
        }
        if let Some(observation) = &self.last_condition_transition {
            observation.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "measurement_state/tests.rs"]
mod tests;

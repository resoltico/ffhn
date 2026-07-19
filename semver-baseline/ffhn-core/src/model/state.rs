use std::collections::BTreeMap;

use crate::CoreError;
use serde::{Deserialize, Deserializer, Serialize};

use super::failure::{IntegrationFaultCode, PermanentErrorCode, SourceSuspectReason};
use super::observation::{PARSER_GRAMMAR_VERSION, PARSER_ID};
use super::outbox::{Outbox, OutboxOverflow, PendingOutboxRecord, StagedOutboxRecord};
use super::{
    ConditionContext, ConditionEvaluation, ConditionId, ConditionOutcome, DiagnosticDetail,
    IntegrationFaultEpisodeSnapshot, LifecycleSnapshot, Observation, OutboxPolicy,
    PermanentErrorEpisodeSnapshot, RouteId, TargetDocument, TargetId,
};

#[path = "state/source_health.rs"]
mod source_health;

use source_health::SourceHealth;

/// Canonical schema name for persisted state.
pub const STATE_SCHEMA_NAME: &str = "ffhn.state";
/// Canonical state-schema version.
pub const STATE_SCHEMA_VERSION: u32 = 17;

/// Persisted target state, including temporal policy and health facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StateDocument {
    schema_name: String,
    schema_version: u32,
    target_id: TargetId,
    contract_digest_sha256: String,
    parser_id: String,
    parser_grammar_version: u32,
    observation_seq: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    accepted_observation: Option<Observation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fixed_initial_baseline: Option<Observation>,
    condition_state: BTreeMap<ConditionId, ConditionState>,
    source_health: SourceHealth,
    #[serde(skip_serializing_if = "Option::is_none")]
    permanent_error_episode: Option<PermanentErrorEpisode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_fault_episode: Option<IntegrationFaultEpisode>,
    outbox: Outbox,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StateDocumentWire {
    schema_name: String,
    schema_version: u32,
    target_id: TargetId,
    contract_digest_sha256: String,
    parser_id: String,
    parser_grammar_version: u32,
    observation_seq: u64,
    #[serde(default)]
    accepted_observation: Option<Observation>,
    #[serde(default)]
    fixed_initial_baseline: Option<Observation>,
    condition_state: BTreeMap<ConditionId, ConditionState>,
    source_health: SourceHealth,
    #[serde(default)]
    permanent_error_episode: Option<PermanentErrorEpisode>,
    #[serde(default)]
    integration_fault_episode: Option<IntegrationFaultEpisode>,
    outbox: Outbox,
}

impl<'de> Deserialize<'de> for StateDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = StateDocumentWire::deserialize(deserializer)?;
        let state = Self::from_wire(wire);
        state.validate().map_err(serde::de::Error::custom)?;
        Ok(state)
    }
}

impl StateDocument {
    fn from_wire(wire: StateDocumentWire) -> Self {
        Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            target_id: wire.target_id,
            contract_digest_sha256: wire.contract_digest_sha256,
            parser_id: wire.parser_id,
            parser_grammar_version: wire.parser_grammar_version,
            observation_seq: wire.observation_seq,
            accepted_observation: wire.accepted_observation,
            fixed_initial_baseline: wire.fixed_initial_baseline,
            condition_state: wire.condition_state,
            source_health: wire.source_health,
            permanent_error_episode: wire.permanent_error_episode,
            integration_fault_episode: wire.integration_fault_episode,
            outbox: wire.outbox,
        }
    }

    /// Builds deliberately invalid state only for tests of state self-validation.
    #[cfg(test)]
    pub(crate) fn from_unvalidated_wire_for_test(wire: serde_json::Value) -> Self {
        let wire = serde_json::from_value(wire)
            .expect("test state wire must remain structurally decodable");
        Self::from_wire(wire)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConditionState {
    result: ConditionOutcome,
    active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_transition_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_transition_value: Option<Observation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PermanentErrorEpisode {
    error_code: PermanentErrorCode,
    first_seen_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IntegrationFaultEpisode {
    integration_fault_code: IntegrationFaultCode,
    first_seen_at: String,
}

impl StateDocument {
    /// Builds an empty state document before any valid observation is accepted.
    pub(crate) fn new(target_id: TargetId, contract_digest_sha256: String) -> Self {
        Self {
            schema_name: STATE_SCHEMA_NAME.to_owned(),
            schema_version: STATE_SCHEMA_VERSION,
            target_id,
            contract_digest_sha256,
            parser_id: PARSER_ID.to_owned(),
            parser_grammar_version: PARSER_GRAMMAR_VERSION,
            observation_seq: 0,
            accepted_observation: None,
            fixed_initial_baseline: None,
            condition_state: BTreeMap::new(),
            source_health: SourceHealth::healthy(),
            permanent_error_episode: None,
            integration_fault_episode: None,
            outbox: Outbox::default(),
        }
    }

    /// Validates this state document independent of a current target definition.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_name != STATE_SCHEMA_NAME || self.schema_version != STATE_SCHEMA_VERSION {
            return Err(CoreError::contract("state is not an FFHN state document"));
        }
        if self.parser_id != PARSER_ID || self.parser_grammar_version != PARSER_GRAMMAR_VERSION {
            return Err(CoreError::contract(
                "state was produced by an incompatible typed parser",
            ));
        }
        if !is_sha256(&self.contract_digest_sha256) {
            return Err(CoreError::contract(
                "state.contract_digest_sha256 must be lowercase SHA-256",
            ));
        }
        match (&self.accepted_observation, &self.fixed_initial_baseline) {
            (None, None) if self.observation_seq == 0 && self.condition_state.is_empty() => {}
            (Some(accepted), Some(initial)) if self.observation_seq > 0 => {
                accepted.validate()?;
                initial.validate()?;
            }
            _ => {
                return Err(CoreError::contract(
                    "state accepted-observation, baseline, sequence, and condition facts disagree",
                ));
            }
        }
        for state in self.condition_state.values() {
            state.validate()?;
        }
        self.source_health.validate()?;
        if let Some(episode) = &self.permanent_error_episode {
            episode.validate()?;
        }
        if let Some(episode) = &self.integration_fault_episode {
            episode.validate()?;
        }
        self.outbox
            .validate(&self.target_id, &self.contract_digest_sha256)?;
        Ok(())
    }

    /// Validates state facts against the current, digest-matching target definition.
    pub(crate) fn validate_for_target(&self, target: &TargetDocument) -> Result<(), CoreError> {
        self.validate()?;
        for observation in [&self.accepted_observation, &self.fixed_initial_baseline]
            .into_iter()
            .flatten()
        {
            if !target.observation_matches(observation) {
                return Err(CoreError::contract(
                    "stored observation does not match the current target type contract",
                ));
            }
        }
        for condition in self.condition_state.values() {
            if let Some(value) = &condition.last_transition_value
                && !target.observation_matches(value)
            {
                return Err(CoreError::contract(
                    "stored condition transition does not match the current target type contract",
                ));
            }
        }
        if self.accepted_observation.is_some()
            && (self.condition_state.len() != target.conditions().len()
                || target
                    .conditions()
                    .iter()
                    .any(|condition| !self.condition_state.contains_key(condition.id())))
        {
            return Err(CoreError::contract(
                "stored condition state does not match the current target contract",
            ));
        }
        for record in self.outbox.records() {
            let route = target.route(record.route_id()).ok_or_else(|| {
                CoreError::contract(
                    "stored outbox record references a route absent from the target",
                )
            })?;
            if route.route_family() != record.route_family() {
                return Err(CoreError::contract(
                    "stored outbox record route family differs from the target route",
                ));
            }
        }
        Ok(())
    }

    /// Returns pre-run contexts for all conditions in the current target contract.
    pub(crate) fn condition_contexts<'a>(
        &'a self,
        target: &TargetDocument,
    ) -> BTreeMap<ConditionId, ConditionContext<'a>> {
        target
            .conditions()
            .iter()
            .map(|condition| {
                let condition_state = self.condition_state.get(condition.id());
                (
                    condition.id().clone(),
                    ConditionContext::new(
                        self.accepted_observation.as_ref(),
                        self.fixed_initial_baseline.as_ref(),
                        condition_state.and_then(|state| state.last_transition_value.as_ref()),
                        condition_state.is_some_and(|state| state.active),
                    ),
                )
            })
            .collect()
    }

    /// Returns the complete durable lifecycle state for public run and status reports.
    pub(crate) fn lifecycle_snapshot(&self) -> Result<LifecycleSnapshot, CoreError> {
        LifecycleSnapshot::new(
            self.source_health.snapshot()?,
            self.permanent_error_episode
                .as_ref()
                .map(PermanentErrorEpisode::snapshot)
                .transpose()?,
            self.integration_fault_episode
                .as_ref()
                .map(IntegrationFaultEpisode::snapshot)
                .transpose()?,
        )
    }

    /// Stages one source-suspect episode update and returns whether it just reached escalation.
    pub(crate) fn apply_source_suspect(
        &mut self,
        reason_class: SourceSuspectReason,
        details: DiagnosticDetail,
        now: &str,
        escalate_after: u32,
    ) -> Result<bool, CoreError> {
        self.source_health
            .apply_suspect(reason_class, details, now, escalate_after)
    }

    /// Stages one permanent contract-error episode and returns whether it just began.
    pub(crate) fn apply_permanent_error(
        &mut self,
        error_code: PermanentErrorCode,
        now: &str,
    ) -> Result<bool, CoreError> {
        require_timestamp("permanent-error timestamp", now)?;
        if self
            .permanent_error_episode
            .as_ref()
            .is_some_and(|episode| episode.error_code == error_code)
        {
            return Ok(false);
        }
        self.permanent_error_episode = Some(PermanentErrorEpisode {
            error_code,
            first_seen_at: now.to_owned(),
        });
        Ok(true)
    }

    /// Stages one integration-fault episode and returns whether it just began or changed code.
    pub(crate) fn apply_integration_fault(
        &mut self,
        integration_fault_code: IntegrationFaultCode,
        now: &str,
    ) -> Result<bool, CoreError> {
        require_timestamp("integration-fault timestamp", now)?;
        if self
            .integration_fault_episode
            .as_ref()
            .is_some_and(|episode| episode.integration_fault_code == integration_fault_code)
        {
            return Ok(false);
        }
        self.integration_fault_episode = Some(IntegrationFaultEpisode {
            integration_fault_code,
            first_seen_at: now.to_owned(),
        });
        Ok(true)
    }

    /// Applies all valid-observation temporal mutations after policy evaluation has staged.
    pub(crate) fn apply_valid_observation(
        &mut self,
        target: &TargetDocument,
        observation: Observation,
        evaluations: &[ConditionEvaluation],
        now: &str,
    ) -> Result<(), CoreError> {
        require_timestamp("condition-transition timestamp", now)?;
        if !target.observation_matches(&observation) {
            return Err(CoreError::contract(
                "accepted observation does not match the current target type contract",
            ));
        }
        let mut staged = BTreeMap::new();
        if evaluations.len() != target.conditions().len() {
            return Err(CoreError::contract(
                "policy staging did not produce one evaluation per target condition",
            ));
        }
        for condition in target.conditions() {
            let evaluation = evaluations
                .iter()
                .find(|evaluation| evaluation.condition_id() == condition.condition_id())
                .ok_or_else(|| {
                    CoreError::contract(
                        "policy staging did not produce one evaluation per target condition",
                    )
                })?;
            let prior = self.condition_state.get(condition.id());
            let result_changed = prior.is_none_or(|state| state.result != evaluation.outcome());
            let next = ConditionState {
                result: evaluation.outcome(),
                active: evaluation.active_after(),
                last_transition_at: if result_changed {
                    Some(now.to_owned())
                } else {
                    prior.and_then(|state| state.last_transition_at.clone())
                },
                last_transition_value: if result_changed {
                    Some(observation.clone())
                } else {
                    prior.and_then(|state| state.last_transition_value.clone())
                },
            };
            staged.insert(condition.id().clone(), next);
        }
        self.observation_seq = self
            .observation_seq
            .checked_add(1)
            .ok_or_else(|| CoreError::contract("observation sequence overflow"))?;
        if self.fixed_initial_baseline.is_none() {
            self.fixed_initial_baseline = Some(observation.clone());
        }
        self.accepted_observation = Some(observation);
        self.condition_state = staged;
        self.source_health = SourceHealth::healthy();
        self.permanent_error_episode = None;
        self.integration_fault_episode = None;
        self.validate_for_target(target)
    }

    /// Returns the owning target id.
    pub fn target_id(&self) -> &str {
        self.target_id.as_str()
    }
    /// Returns the captured contract digest.
    pub fn contract_digest_sha256(&self) -> &str {
        &self.contract_digest_sha256
    }
    /// Returns the accepted valid observation, when one exists.
    pub fn accepted_observation(&self) -> Option<&Observation> {
        self.accepted_observation.as_ref()
    }
    /// Returns the monotonic sequence assigned to accepted observations.
    pub const fn observation_seq(&self) -> u64 {
        self.observation_seq
    }

    pub(crate) fn condition_transition_at(&self, condition_id: &ConditionId) -> Option<&str> {
        self.condition_state
            .get(condition_id)
            .and_then(|state| state.last_transition_at.as_deref())
    }

    pub(crate) fn source_episode_started_at(&self, reason: SourceSuspectReason) -> Option<&str> {
        self.source_health.episode_started_at(reason)
    }

    pub(crate) fn permanent_episode_started_at(&self, code: PermanentErrorCode) -> Option<&str> {
        self.permanent_error_episode
            .as_ref()
            .filter(|episode| episode.error_code == code)
            .map(|episode| episode.first_seen_at.as_str())
    }

    pub(crate) fn integration_fault_episode_started_at(
        &self,
        code: IntegrationFaultCode,
    ) -> Option<&str> {
        self.integration_fault_episode
            .as_ref()
            .filter(|episode| episode.integration_fault_code == code)
            .map(|episode| episode.first_seen_at.as_str())
    }

    pub(crate) fn enqueue_outbox(
        &mut self,
        records: Vec<StagedOutboxRecord>,
        policy: &OutboxPolicy,
        commit_time: &str,
    ) -> Result<Vec<OutboxOverflow>, CoreError> {
        self.outbox.enqueue(
            records,
            policy.max_pending(),
            commit_time,
            &self.target_id,
            &self.contract_digest_sha256,
        )
    }

    pub(crate) fn next_due_outbox_record(
        &self,
        now: &str,
    ) -> Result<Option<PendingOutboxRecord>, CoreError> {
        self.outbox.first_due(now)
    }

    pub(crate) fn remove_outbox_record(
        &mut self,
        event_id: &str,
        route_id: &RouteId,
    ) -> Result<(), CoreError> {
        self.outbox.remove(event_id, route_id)
    }

    pub(crate) fn record_outbox_failure(
        &mut self,
        event_id: &str,
        route_id: &RouteId,
        error_detail: DiagnosticDetail,
        next_retry_at: String,
    ) -> Result<u32, CoreError> {
        self.outbox
            .record_failure(event_id, route_id, error_detail, next_retry_at)
    }
}

impl ConditionState {
    fn validate(&self) -> Result<(), CoreError> {
        match (&self.last_transition_at, &self.last_transition_value) {
            (None, None) => Ok(()),
            (Some(at), Some(value)) => {
                require_timestamp("condition transition timestamp", at)?;
                value.validate()
            }
            _ => Err(CoreError::contract(
                "condition transition timestamp and value must appear together",
            )),
        }
    }
}

impl PermanentErrorEpisode {
    fn validate(&self) -> Result<(), CoreError> {
        require_timestamp("permanent-error first-seen timestamp", &self.first_seen_at)
    }

    fn snapshot(&self) -> Result<PermanentErrorEpisodeSnapshot, CoreError> {
        PermanentErrorEpisodeSnapshot::new(self.error_code, self.first_seen_at.clone())
    }
}

impl IntegrationFaultEpisode {
    fn validate(&self) -> Result<(), CoreError> {
        require_timestamp(
            "integration-fault first-seen timestamp",
            &self.first_seen_at,
        )
    }

    fn snapshot(&self) -> Result<IntegrationFaultEpisodeSnapshot, CoreError> {
        IntegrationFaultEpisodeSnapshot::new(
            self.integration_fault_code,
            self.first_seen_at.clone(),
        )
    }
}

fn require_timestamp(field: &str, value: &str) -> Result<(), CoreError> {
    super::require_canonical_utc_rfc3339(field, value)
}

pub(super) fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

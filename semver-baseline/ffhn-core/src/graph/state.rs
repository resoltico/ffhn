//! Source-owned durable state. Measurement temporal state has a separate ownership boundary.

use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use url::Url;

use crate::CoreError;

use super::{
    GraphIntegrationFaultCode, HttpValidators, IntegrationFaultEpisode, OutboxOverflowFact,
    SourceAcquisitionHealth, SourceFetchFailure, SourceInstanceId,
};

#[path = "measurement_state.rs"]
mod measurement_state;

pub use measurement_state::{
    MEASUREMENT_STATE_SCHEMA_NAME, MEASUREMENT_STATE_SCHEMA_VERSION, MeasurementState,
};

/// Canonical source-state schema name.
pub const SOURCE_STATE_SCHEMA_NAME: &str = "ffhn.source_state";
/// Canonical source-state schema version.
pub const SOURCE_STATE_SCHEMA_VERSION: u32 = 1;

/// Per-source durable state whose generation is advanced only by normal commits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceState {
    schema_name: String,
    schema_version: u32,
    source_instance_id: SourceInstanceId,
    generation: u64,
    source_episode_seq: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    validators: Option<HttpValidators>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_url: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effective_base_url: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_content_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_source_representation_digest: Option<String>,
    source_health: SourceAcquisitionHealth,
    outbox_overflow: Vec<OutboxOverflowFact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_fault_episode: Option<IntegrationFaultEpisode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_cycle_completed_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_due_utc: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceStateWire {
    schema_name: String,
    schema_version: u32,
    source_instance_id: SourceInstanceId,
    generation: u64,
    source_episode_seq: u64,
    #[serde(default)]
    validators: Option<HttpValidators>,
    #[serde(default)]
    source_url: Option<Url>,
    #[serde(default)]
    effective_base_url: Option<Url>,
    #[serde(default)]
    file_content_sha256: Option<String>,
    #[serde(default)]
    last_source_representation_digest: Option<String>,
    #[serde(default = "SourceAcquisitionHealth::healthy")]
    source_health: SourceAcquisitionHealth,
    outbox_overflow: Vec<OutboxOverflowFact>,
    #[serde(default)]
    integration_fault_episode: Option<IntegrationFaultEpisode>,
    #[serde(default)]
    last_cycle_completed_utc: Option<String>,
    #[serde(default)]
    next_due_utc: Option<String>,
}

impl<'de> Deserialize<'de> for SourceState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = SourceStateWire::deserialize(deserializer)?;
        let state = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            source_instance_id: wire.source_instance_id,
            generation: wire.generation,
            source_episode_seq: wire.source_episode_seq,
            validators: wire.validators,
            source_url: wire.source_url,
            effective_base_url: wire.effective_base_url,
            file_content_sha256: wire.file_content_sha256,
            last_source_representation_digest: wire.last_source_representation_digest,
            source_health: wire.source_health,
            outbox_overflow: wire.outbox_overflow,
            integration_fault_episode: wire.integration_fault_episode,
            last_cycle_completed_utc: wire.last_cycle_completed_utc,
            next_due_utc: wire.next_due_utc,
        };
        state.validate().map_err(serde::de::Error::custom)?;
        Ok(state)
    }
}

impl SourceState {
    /// Creates the fresh state document for a new source lineage.
    pub fn fresh(source_instance_id: SourceInstanceId) -> Self {
        Self {
            schema_name: SOURCE_STATE_SCHEMA_NAME.to_owned(),
            schema_version: SOURCE_STATE_SCHEMA_VERSION,
            source_instance_id,
            generation: 1,
            source_episode_seq: 0,
            validators: None,
            source_url: None,
            effective_base_url: None,
            file_content_sha256: None,
            last_source_representation_digest: None,
            source_health: SourceAcquisitionHealth::healthy(),
            outbox_overflow: Vec::new(),
            integration_fault_episode: None,
            last_cycle_completed_utc: None,
            next_due_utc: None,
        }
    }

    /// Validates the closed source-state schema and its durable lineage facts.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_name != SOURCE_STATE_SCHEMA_NAME
            || self.schema_version != SOURCE_STATE_SCHEMA_VERSION
            || self.generation == 0
        {
            return Err(CoreError::contract(
                "source state is not a current FFHN source state",
            ));
        }
        self.source_instance_id.validate()?;
        for digest in [
            &self.file_content_sha256,
            &self.last_source_representation_digest,
        ]
        .into_iter()
        .flatten()
        {
            validate_sha256("source state digest", digest)?;
        }
        if let Some(validators) = &self.validators {
            validators.validate()?;
        }
        match (
            self.source_url.as_ref(),
            self.effective_base_url.as_ref(),
            self.file_content_sha256.as_ref(),
            self.last_source_representation_digest.as_ref(),
        ) {
            (None, None, None, None) if self.validators.is_none() => {}
            (Some(source_url), Some(effective), None, Some(_))
                if valid_http_url(source_url) && valid_http_url(effective) =>
            {
                if self.validators.as_ref().is_some_and(|validators| {
                    validators.issued_url != *source_url || effective != source_url
                }) {
                    return Err(CoreError::contract(
                        "source HTTP validators must belong to the direct stored source URL",
                    ));
                }
            }
            (None, None, Some(_), Some(_)) if self.validators.is_none() => {}
            _ => {
                return Err(CoreError::contract(
                    "source representation provenance must be coherently HTTP, file, or absent",
                ));
            }
        }
        self.source_health.validate()?;
        for overflow in &self.outbox_overflow {
            overflow.validate()?;
        }
        if let Some(episode) = &self.integration_fault_episode {
            episode.validate()?;
            if episode.code() != GraphIntegrationFaultCode::SecretUnavailable {
                return Err(CoreError::contract(
                    "source state integration fault must be secret_unavailable",
                ));
            }
        }
        match (
            self.last_cycle_completed_utc.as_deref(),
            self.next_due_utc.as_deref(),
        ) {
            (None, None) => {}
            (Some(completed), Some(next_due)) => {
                require_timestamp("source cycle completion time", completed)?;
                require_timestamp("source next due time", next_due)?;
                let completed = OffsetDateTime::parse(completed, &Rfc3339)?;
                let next_due = OffsetDateTime::parse(next_due, &Rfc3339)?;
                if next_due <= completed {
                    return Err(CoreError::contract(
                        "source next due time must be after cycle completion",
                    ));
                }
            }
            _ => {
                return Err(CoreError::contract(
                    "source cycle completion and next due timestamps must appear together",
                ));
            }
        }
        Ok(())
    }

    /// Returns the source lineage token stamped into this state document.
    pub fn source_instance_id(&self) -> &SourceInstanceId {
        &self.source_instance_id
    }

    /// Returns the currently installed normal-commit generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the current source-episode discriminator for deterministic source event keys.
    pub const fn source_episode_seq(&self) -> u64 {
        self.source_episode_seq
    }

    /// Returns the active source-scoped integration-fault code, if any.
    pub fn source_integration_fault_code(&self) -> Option<GraphIntegrationFaultCode> {
        self.integration_fault_episode
            .as_ref()
            .map(IntegrationFaultEpisode::code)
    }

    /// Returns the source acquisition-health episode owned by this source state.
    pub const fn source_health(&self) -> &SourceAcquisitionHealth {
        &self.source_health
    }

    /// Returns the active source integration-fault episode, if any.
    pub fn integration_fault_episode(&self) -> Option<&IntegrationFaultEpisode> {
        self.integration_fault_episode.as_ref()
    }

    /// Returns the latest committed source-owned overflow facts.
    pub fn outbox_overflow(&self) -> &[OutboxOverflowFact] {
        &self.outbox_overflow
    }

    /// Replaces source-owned overflow evidence for the generation being committed.
    pub fn set_outbox_overflow(
        &mut self,
        overflow: Vec<OutboxOverflowFact>,
    ) -> Result<(), CoreError> {
        for fact in &overflow {
            fact.validate()?;
        }
        self.outbox_overflow = overflow;
        Ok(())
    }

    /// Returns the successor state generation for one acquisition or delivery commit.
    pub fn next_generation(&self) -> Result<Self, CoreError> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| CoreError::contract("source generation overflowed"))?;
        Ok(Self {
            schema_name: self.schema_name.clone(),
            schema_version: self.schema_version,
            source_instance_id: self.source_instance_id.clone(),
            generation,
            source_episode_seq: self.source_episode_seq,
            validators: self.validators.clone(),
            source_url: self.source_url.clone(),
            effective_base_url: self.effective_base_url.clone(),
            file_content_sha256: self.file_content_sha256.clone(),
            last_source_representation_digest: self.last_source_representation_digest.clone(),
            source_health: self.source_health.clone(),
            outbox_overflow: self.outbox_overflow.clone(),
            integration_fault_episode: self.integration_fault_episode.clone(),
            last_cycle_completed_utc: self.last_cycle_completed_utc.clone(),
            next_due_utc: self.next_due_utc.clone(),
        })
    }

    /// Returns validator provenance from a direct accepted HTTP representation.
    pub fn validators(&self) -> Option<&HttpValidators> {
        self.validators.as_ref()
    }

    /// Returns the configured HTTP URL for the last accepted HTTP representation.
    pub fn source_url(&self) -> Option<&Url> {
        self.source_url.as_ref()
    }

    /// Returns the effective HTML base for the last accepted HTTP representation.
    pub fn effective_base_url(&self) -> Option<&Url> {
        self.effective_base_url.as_ref()
    }

    /// Returns the latest accepted file-content digest when the source is file-backed.
    pub fn file_content_sha256(&self) -> Option<&str> {
        self.file_content_sha256.as_deref()
    }

    /// Returns the SRD associated with the last accepted source representation.
    pub fn last_source_representation_digest(&self) -> Option<&str> {
        self.last_source_representation_digest.as_deref()
    }

    /// Returns the last completed source-cycle UTC time when scheduling has begun.
    pub fn last_cycle_completed_utc(&self) -> Option<&str> {
        self.last_cycle_completed_utc.as_deref()
    }

    /// Returns the next durable UTC due time when scheduling has begun.
    pub fn next_due_utc(&self) -> Option<&str> {
        self.next_due_utc.as_deref()
    }

    /// Returns whether this state records the exact file digest under the supplied source digest.
    pub fn matches_file_representation(
        &self,
        source_representation_digest: &str,
        file_content_sha256: &str,
    ) -> bool {
        self.last_source_representation_digest.as_deref() == Some(source_representation_digest)
            && self.file_content_sha256.as_deref() == Some(file_content_sha256)
    }

    /// Replaces representation facts while preserving the current lineage and generation.
    pub fn with_representation_facts(
        &self,
        validators: Option<HttpValidators>,
        source_url: Option<Url>,
        effective_base_url: Option<Url>,
        file_content_sha256: Option<String>,
        source_representation_digest: String,
    ) -> Result<Self, CoreError> {
        validate_sha256(
            "source representation digest",
            &source_representation_digest,
        )?;
        if let Some(digest) = &file_content_sha256 {
            validate_sha256("file content digest", digest)?;
        }
        let state = Self {
            schema_name: self.schema_name.clone(),
            schema_version: self.schema_version,
            source_instance_id: self.source_instance_id.clone(),
            generation: self.generation,
            source_episode_seq: self.source_episode_seq,
            validators,
            source_url,
            effective_base_url,
            file_content_sha256,
            last_source_representation_digest: Some(source_representation_digest),
            source_health: self.source_health.clone(),
            outbox_overflow: self.outbox_overflow.clone(),
            integration_fault_episode: self.integration_fault_episode.clone(),
            last_cycle_completed_utc: self.last_cycle_completed_utc.clone(),
            next_due_utc: self.next_due_utc.clone(),
        };
        state.validate()?;
        Ok(state)
    }

    /// Records one source acquisition failure and returns whether its health episode escalated.
    pub fn apply_acquisition_failure(
        &mut self,
        failure: SourceFetchFailure,
        now_utc: &str,
        escalate_after: u32,
    ) -> Result<bool, CoreError> {
        let episode_began = self.source_health.failure_kind() != Some(failure.kind);
        let mut source_health = self.source_health.clone();
        let escalated = source_health.observe(failure, now_utc, escalate_after)?;
        let source_episode_seq = if episode_began {
            self.source_episode_seq
                .checked_add(1)
                .ok_or_else(|| CoreError::contract("source episode sequence overflowed"))?
        } else {
            self.source_episode_seq
        };
        self.source_health = source_health;
        self.source_episode_seq = source_episode_seq;
        self.integration_fault_episode = None;
        self.validate()?;
        Ok(escalated)
    }

    /// Records the only source-scoped integration fault and returns whether its episode began.
    pub fn apply_source_integration_fault(
        &mut self,
        code: GraphIntegrationFaultCode,
        now_utc: String,
    ) -> Result<bool, CoreError> {
        if code != GraphIntegrationFaultCode::SecretUnavailable {
            return Err(CoreError::contract(
                "only secret_unavailable is a source-scoped integration fault",
            ));
        }
        if self
            .integration_fault_episode
            .as_ref()
            .is_some_and(|episode| episode.code() == code)
        {
            return Ok(false);
        }
        let source_episode_seq = self
            .source_episode_seq
            .checked_add(1)
            .ok_or_else(|| CoreError::contract("source episode sequence overflowed"))?;
        let episode = IntegrationFaultEpisode::new(code, now_utc)?;
        self.source_episode_seq = source_episode_seq;
        self.integration_fault_episode = Some(episode);
        self.validate()?;
        Ok(true)
    }

    /// Clears source acquisition-health and source integration-fault episodes after success.
    pub fn clear_transient_episodes(&mut self) {
        self.source_health.clear();
        self.integration_fault_episode = None;
    }

    /// Records a completed source cycle and its already-computed next UTC due time.
    pub fn with_cycle_schedule(
        &self,
        completed_at_utc: String,
        next_due_utc: String,
    ) -> Result<Self, CoreError> {
        require_timestamp("source cycle completion time", &completed_at_utc)?;
        require_timestamp("source next due time", &next_due_utc)?;
        let state = Self {
            schema_name: self.schema_name.clone(),
            schema_version: self.schema_version,
            source_instance_id: self.source_instance_id.clone(),
            generation: self.generation,
            source_episode_seq: self.source_episode_seq,
            validators: self.validators.clone(),
            source_url: self.source_url.clone(),
            effective_base_url: self.effective_base_url.clone(),
            file_content_sha256: self.file_content_sha256.clone(),
            last_source_representation_digest: self.last_source_representation_digest.clone(),
            source_health: self.source_health.clone(),
            outbox_overflow: self.outbox_overflow.clone(),
            integration_fault_episode: self.integration_fault_episode.clone(),
            last_cycle_completed_utc: Some(completed_at_utc),
            next_due_utc: Some(next_due_utc),
        };
        state.validate()?;
        Ok(state)
    }
}

pub(super) fn validate_sha256(field: &str, value: &str) -> Result<(), CoreError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(CoreError::contract(format!(
            "{field} must be lowercase SHA-256"
        )))
    }
}

fn require_timestamp(field: &str, value: &str) -> Result<(), CoreError> {
    crate::model::require_canonical_utc_rfc3339(field, value)
}

fn valid_http_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url.username().is_empty()
        && url.password().is_none()
}

#[cfg(test)]
#[path = "state/tests.rs"]
mod tests;

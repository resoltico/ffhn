//! Narrow read-only accessors for measurement-owned temporal state.

use super::super::super::{
    IntegrationFaultEpisode, MeasurementExtractionHealth, MeasurementInstanceId,
    OutboxOverflowFact, SourceInstanceId,
};
use super::MeasurementState;
use crate::{CoreError, Observation};

impl MeasurementState {
    /// Returns the source lineage token stamped into this measurement state.
    pub fn source_instance_id(&self) -> &SourceInstanceId {
        &self.source_instance_id
    }

    /// Returns the measurement lineage token stamped into this measurement state.
    pub fn measurement_instance_id(&self) -> &MeasurementInstanceId {
        &self.measurement_instance_id
    }

    /// Returns the current accepted typed observation, if initialized.
    pub fn accepted_observation(&self) -> Option<&Observation> {
        self.accepted_observation.as_ref()
    }

    /// Returns the monotonic accepted-observation sequence.
    pub const fn observation_seq(&self) -> u64 {
        self.observation_seq
    }

    /// Returns the value-contract digest for accepted observations.
    pub fn measurement_value_digest(&self) -> Option<&str> {
        self.measurement_value_digest.as_deref()
    }

    /// Returns the audit-only policy revision advanced on any per-condition rebase.
    pub const fn policy_revision(&self) -> u64 {
        self.policy_revision
    }

    /// Returns the current episode discriminator for measurement lifecycle event keys.
    pub const fn measurement_episode_seq(&self) -> u64 {
        self.measurement_episode_seq
    }

    /// Returns the measurement-owned extraction-health episode.
    pub const fn extraction_health(&self) -> &MeasurementExtractionHealth {
        &self.extraction_health
    }

    /// Returns the active measurement integration-fault episode, if any.
    pub fn integration_fault_episode(&self) -> Option<&IntegrationFaultEpisode> {
        self.integration_fault_episode.as_ref()
    }

    /// Returns the latest committed measurement-owned overflow facts.
    pub fn outbox_overflow(&self) -> &[OutboxOverflowFact] {
        &self.outbox_overflow
    }

    /// Replaces measurement-owned overflow evidence for the generation being committed.
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
}

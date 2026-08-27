//! Immutable event envelopes whose event ID is derived exclusively from their key.

use serde::{Deserialize, Deserializer, Serialize};

use crate::{ConditionId, CoreError};

use super::super::{GraphId, MeasurementId, MeasurementInstanceId, SourceId, SourceInstanceId};
use super::{EVENT_ENVELOPE_SCHEMA_NAME, EVENT_ENVELOPE_SCHEMA_VERSION, EventKey, EventKind};

/// Emitter identity carried explicitly in every v11 event envelope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEmitter {
    kind: EventEmitterKind,
    source_id: SourceId,
    #[serde(skip_serializing_if = "Option::is_none")]
    measurement_id: Option<MeasurementId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    measurement_instance_id: Option<MeasurementInstanceId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EventEmitterKind {
    Source,
    Measurement,
}

impl EventEmitter {
    /// Constructs the source emitter carried by a source-owned event.
    pub fn source(source_id: SourceId) -> Self {
        Self {
            kind: EventEmitterKind::Source,
            source_id,
            measurement_id: None,
            measurement_instance_id: None,
        }
    }

    /// Constructs the measurement emitter carried by a measurement-owned event.
    pub fn measurement(
        source_id: SourceId,
        measurement_id: MeasurementId,
        measurement_instance_id: MeasurementInstanceId,
    ) -> Self {
        Self {
            kind: EventEmitterKind::Measurement,
            source_id,
            measurement_id: Some(measurement_id),
            measurement_instance_id: Some(measurement_instance_id),
        }
    }

    pub(super) fn validate(&self) -> Result<(), CoreError> {
        match (
            self.kind,
            self.measurement_id.as_ref(),
            self.measurement_instance_id.as_ref(),
        ) {
            (EventEmitterKind::Source, None, None) => Ok(()),
            (EventEmitterKind::Measurement, Some(_), Some(instance)) => instance.validate(),
            _ => Err(CoreError::contract(
                "event emitter scope and measurement lineage facts disagree",
            )),
        }
    }
}

/// Observation evidence attached to one condition-scoped event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventObservation {
    canonical_value: String,
    seq: u64,
}

impl EventObservation {
    /// Creates canonical accepted-observation evidence for an event envelope.
    pub fn new(canonical_value: String, seq: u64) -> Result<Self, CoreError> {
        let value = Self {
            canonical_value,
            seq,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), CoreError> {
        if self.seq == 0 {
            Err(CoreError::contract(
                "event observation sequence must be positive",
            ))
        } else {
            Ok(())
        }
    }
}

/// Inputs for one immutable event envelope.
pub struct EventEnvelopeParts {
    /// Graph lineage root.
    pub graph_id: GraphId,
    /// Source lineage token.
    pub source_instance_id: SourceInstanceId,
    /// Key whose stable JSON derives the event identifier.
    pub event_key: EventKey,
    /// Public display name snapshot for the current source or measurement.
    pub display_name: String,
    /// Commit time, retained as evidence but structurally excluded from the key.
    pub committed_at_utc: String,
    /// Current accepted-observation evidence for condition events.
    pub observation: Option<EventObservation>,
    /// Human-safe, closed vocabulary evidence for lifecycle and episode events.
    pub lifecycle_fact: Option<String>,
    /// Current condition policy revision, for audit only and never part of the key.
    pub policy_revision: Option<u64>,
}

/// Immutable public payload staged before any route fan-out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope {
    schema_name: String,
    schema_version: u32,
    graph_id: GraphId,
    source_instance_id: SourceInstanceId,
    emitter: EventEmitter,
    event_kind: EventKind,
    event_id: String,
    event_key: EventKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    observation: Option<EventObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lifecycle_fact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    policy_revision: Option<u64>,
    display_name: String,
    committed_at_utc: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventEnvelopeWire {
    schema_name: String,
    schema_version: u32,
    graph_id: GraphId,
    source_instance_id: SourceInstanceId,
    emitter: EventEmitter,
    event_kind: EventKind,
    event_id: String,
    event_key: EventKey,
    #[serde(default)]
    observation: Option<EventObservation>,
    #[serde(default)]
    lifecycle_fact: Option<String>,
    #[serde(default)]
    policy_revision: Option<u64>,
    display_name: String,
    committed_at_utc: String,
}

impl<'de> Deserialize<'de> for EventEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = EventEnvelopeWire::deserialize(deserializer)?;
        let envelope = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            graph_id: wire.graph_id,
            source_instance_id: wire.source_instance_id,
            emitter: wire.emitter,
            event_kind: wire.event_kind,
            event_id: wire.event_id,
            event_key: wire.event_key,
            observation: wire.observation,
            lifecycle_fact: wire.lifecycle_fact,
            policy_revision: wire.policy_revision,
            display_name: wire.display_name,
            committed_at_utc: wire.committed_at_utc,
        };
        envelope.validate().map_err(serde::de::Error::custom)?;
        Ok(envelope)
    }
}

impl EventEnvelope {
    /// Creates an envelope, deriving its ID from the key and checking scope coherence.
    pub fn new(parts: EventEnvelopeParts) -> Result<Self, CoreError> {
        let event_kind = parts.event_key.event_kind();
        let emitter = emitter_for(&parts.event_key)?;
        let event_id = parts.event_key.event_id()?;
        let envelope = Self {
            schema_name: EVENT_ENVELOPE_SCHEMA_NAME.to_owned(),
            schema_version: EVENT_ENVELOPE_SCHEMA_VERSION,
            graph_id: parts.graph_id,
            source_instance_id: parts.source_instance_id,
            emitter,
            event_kind,
            event_id,
            event_key: parts.event_key,
            observation: parts.observation,
            lifecycle_fact: parts.lifecycle_fact,
            policy_revision: parts.policy_revision,
            display_name: parts.display_name,
            committed_at_utc: parts.committed_at_utc,
        };
        envelope.validate()?;
        Ok(envelope)
    }

    /// Returns the route-independent deterministic event ID.
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    /// Returns the stable event kind bound into the envelope's deterministic key.
    pub const fn event_kind(&self) -> EventKind {
        self.event_kind
    }

    /// Returns the source lineage stamp bound into both envelope and deterministic key.
    pub const fn source_instance_id(&self) -> &SourceInstanceId {
        &self.source_instance_id
    }

    /// Returns the source-directory identifier bound into the deterministic event key.
    pub const fn source_id(&self) -> &SourceId {
        self.event_key.source_id()
    }

    /// Returns measurement lineage when the event belongs to a measurement emitter.
    pub const fn measurement_lineage(&self) -> Option<(&MeasurementId, &MeasurementInstanceId)> {
        self.event_key.measurement_lineage()
    }

    /// Returns the immutable deterministic key.
    pub const fn event_key(&self) -> &EventKey {
        &self.event_key
    }

    /// Returns the condition identity carried by a condition-scoped event.
    pub const fn condition_id(&self) -> Option<&ConditionId> {
        match &self.event_key {
            EventKey::ConditionSatisfied { condition_id, .. }
            | EventKey::ConditionEvaluationIssue { condition_id, .. } => Some(condition_id),
            EventKey::MeasurementLifecycle { .. }
            | EventKey::MeasurementEpisode { .. }
            | EventKey::SourceLifecycle { .. }
            | EventKey::SourceEpisode { .. } => None,
        }
    }

    /// Validates every persisted envelope field and its derived identity.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_name != EVENT_ENVELOPE_SCHEMA_NAME
            || self.schema_version != EVENT_ENVELOPE_SCHEMA_VERSION
        {
            return Err(CoreError::contract(
                "event envelope is not a current FFHN event envelope",
            ));
        }
        self.graph_id.validate()?;
        self.source_instance_id.validate()?;
        self.emitter.validate()?;
        self.event_key.validate()?;
        if self.graph_id != *self.event_key.graph_id()
            || self.source_instance_id != *self.event_key.source_instance_id()
            || self.event_kind != self.event_key.event_kind()
            || self.event_id != self.event_key.event_id()?
        {
            return Err(CoreError::contract(
                "event envelope kind or identifier does not match its deterministic key",
            ));
        }
        if self.display_name.trim().is_empty() {
            return Err(CoreError::contract(
                "event envelope display name must not be blank",
            ));
        }
        crate::model::require_canonical_utc_rfc3339(
            "event envelope committed_at_utc",
            &self.committed_at_utc,
        )?;
        if self.event_key.measurement_lineage().is_none() && self.policy_revision.is_some() {
            return Err(CoreError::contract(
                "source event envelope must not carry a measurement policy revision",
            ));
        }
        match (&self.event_key, &self.observation, &self.lifecycle_fact) {
            (
                EventKey::ConditionSatisfied {
                    observation_seq, ..
                },
                Some(observation),
                None,
            )
            | (
                EventKey::ConditionEvaluationIssue {
                    observation_seq, ..
                },
                Some(observation),
                None,
            ) if observation.seq == *observation_seq => {}
            (
                EventKey::MeasurementLifecycle { .. } | EventKey::SourceLifecycle { .. },
                None,
                Some(fact),
            ) if fact == "initialized" => {}
            (
                EventKey::MeasurementEpisode { code, .. } | EventKey::SourceEpisode { code, .. },
                None,
                Some(fact),
            ) if fact == code => {}
            _ => {
                return Err(CoreError::contract(
                    "event envelope evidence does not match its deterministic event-key class",
                ));
            }
        }
        validate_emitter_matches_key(&self.emitter, &self.event_key)
    }
}

fn emitter_for(key: &EventKey) -> Result<EventEmitter, CoreError> {
    let value = match key {
        EventKey::ConditionSatisfied {
            source_id,
            measurement_id,
            measurement_instance_id,
            ..
        }
        | EventKey::ConditionEvaluationIssue {
            source_id,
            measurement_id,
            measurement_instance_id,
            ..
        }
        | EventKey::MeasurementLifecycle {
            source_id,
            measurement_id,
            measurement_instance_id,
            ..
        }
        | EventKey::MeasurementEpisode {
            source_id,
            measurement_id,
            measurement_instance_id,
            ..
        } => EventEmitter::measurement(
            source_id.clone(),
            measurement_id.clone(),
            measurement_instance_id.clone(),
        ),
        EventKey::SourceLifecycle { source_id, .. } | EventKey::SourceEpisode { source_id, .. } => {
            EventEmitter::source(source_id.clone())
        }
    };
    value.validate()?;
    Ok(value)
}

fn validate_emitter_matches_key(emitter: &EventEmitter, key: &EventKey) -> Result<(), CoreError> {
    let expected = emitter_for(key)?;
    if emitter == &expected {
        Ok(())
    } else {
        Err(CoreError::contract(
            "event envelope emitter does not match its deterministic key",
        ))
    }
}

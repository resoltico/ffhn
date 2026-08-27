//! Route-independent condition event materialization from already staged policy decisions.

use std::collections::BTreeMap;

use crate::{ConditionEvaluation, ConditionOutcome, CoreError};

use super::{
    EventEnvelope, EventEnvelopeParts, EventKey, EventKind, EventObservation, GraphId,
    MeasurementDocument, MeasurementId, MeasurementState, SourceId, SourceInstanceId,
};

/// Materializes condition-satisfied and condition-evaluation-issue envelopes in declaration order.
///
/// Delivery routing is deliberately absent: one event identity precedes all route fan-out and is
/// derived only from lineage, definition, kind, and observation sequence.
#[allow(clippy::too_many_arguments)]
pub fn materialize_condition_events(
    graph_id: GraphId,
    source_id: SourceId,
    source_instance_id: SourceInstanceId,
    measurement_id: MeasurementId,
    measurement: &MeasurementDocument,
    state: &MeasurementState,
    evaluations: &[ConditionEvaluation],
    committed_at_utc: String,
) -> Result<Vec<EventEnvelope>, CoreError> {
    state.validate_for_measurement(measurement)?;
    let observation = state.accepted_observation().ok_or_else(|| {
        CoreError::contract("condition events require an accepted measurement observation")
    })?;
    let digests = measurement
        .condition_definition_digests()?
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let mut results = evaluations
        .iter()
        .map(|evaluation| (evaluation.condition_id(), evaluation))
        .collect::<BTreeMap<_, _>>();
    let mut issues = Vec::new();
    let mut satisfactions = Vec::new();
    for condition in measurement.conditions() {
        let id = condition.condition_id();
        let evaluation = results.remove(id).ok_or_else(|| {
            CoreError::contract("policy staging omitted a declared condition evaluation")
        })?;
        let digest = condition_digest(&digests, id)?;
        let key = match evaluation.outcome() {
            ConditionOutcome::Satisfied if evaluation.trigger() => {
                Some(EventKey::ConditionSatisfied {
                    graph_id: graph_id.clone(),
                    source_id: source_id.clone(),
                    source_instance_id: source_instance_id.clone(),
                    measurement_id: measurement_id.clone(),
                    measurement_instance_id: state.measurement_instance_id().clone(),
                    condition_id: id.parse()?,
                    condition_defn_digest: digest.clone(),
                    observation_seq: state.observation_seq(),
                })
            }
            ConditionOutcome::Unavailable
            | ConditionOutcome::ArithmeticOverflow
            | ConditionOutcome::ZeroReference => Some(EventKey::ConditionEvaluationIssue {
                graph_id: graph_id.clone(),
                source_id: source_id.clone(),
                source_instance_id: source_instance_id.clone(),
                measurement_id: measurement_id.clone(),
                measurement_instance_id: state.measurement_instance_id().clone(),
                issue_code: evaluation.outcome().as_str().to_owned(),
                condition_id: id.parse()?,
                condition_defn_digest: digest.clone(),
                observation_seq: state.observation_seq(),
            }),
            ConditionOutcome::Satisfied | ConditionOutcome::NotSatisfied => None,
        };
        if let Some(event_key) = key {
            let issue = matches!(event_key, EventKey::ConditionEvaluationIssue { .. });
            let event = EventEnvelope::new(EventEnvelopeParts {
                graph_id: graph_id.clone(),
                source_instance_id: source_instance_id.clone(),
                event_key,
                display_name: measurement.display_name().to_owned(),
                committed_at_utc: committed_at_utc.clone(),
                observation: Some(EventObservation::new(
                    observation.canonical_value().to_owned(),
                    state.observation_seq(),
                )?),
                lifecycle_fact: None,
                policy_revision: Some(state.policy_revision()),
            })?;
            if issue {
                issues.push(event);
            } else {
                satisfactions.push(event);
            }
        }
    }
    if !results.is_empty() {
        return Err(CoreError::contract(
            "policy staging contains an undeclared condition evaluation",
        ));
    }
    issues.extend(satisfactions);
    Ok(issues)
}

pub(super) struct MeasurementEventParts<'a> {
    pub(super) graph_id: GraphId,
    pub(super) source_id: SourceId,
    pub(super) source_instance_id: SourceInstanceId,
    pub(super) measurement_id: MeasurementId,
    pub(super) measurement: &'a MeasurementDocument,
    pub(super) state: &'a MeasurementState,
    pub(super) event_kind: EventKind,
    pub(super) code: Option<String>,
    pub(super) measurement_value_digest: String,
    pub(super) committed_at_utc: String,
}

/// Materializes one measurement lifecycle or episode event before condition events are admitted.
pub(super) fn materialize_measurement_event(
    parts: MeasurementEventParts<'_>,
) -> Result<EventEnvelope, CoreError> {
    let MeasurementEventParts {
        graph_id,
        source_id,
        source_instance_id,
        measurement_id,
        measurement,
        state,
        event_kind,
        code,
        measurement_value_digest,
        committed_at_utc,
    } = parts;
    let event_key = match event_kind {
        EventKind::MeasurementInitialized => EventKey::MeasurementLifecycle {
            graph_id: graph_id.clone(),
            source_id: source_id.clone(),
            source_instance_id: source_instance_id.clone(),
            measurement_id: measurement_id.clone(),
            measurement_instance_id: state.measurement_instance_id().clone(),
            event_kind,
            measurement_value_digest,
        },
        EventKind::ExtractionEscalation | EventKind::MeasurementIntegrationFault => {
            EventKey::MeasurementEpisode {
                graph_id: graph_id.clone(),
                source_id: source_id.clone(),
                source_instance_id: source_instance_id.clone(),
                measurement_id: measurement_id.clone(),
                measurement_instance_id: state.measurement_instance_id().clone(),
                event_kind,
                code: code.clone().ok_or_else(|| {
                    CoreError::contract("measurement episode event requires code")
                })?,
                measurement_episode_seq: state.measurement_episode_seq(),
                measurement_value_digest,
            }
        }
        _ => {
            return Err(CoreError::contract(
                "measurement event materializer received a non-measurement event kind",
            ));
        }
    };
    EventEnvelope::new(EventEnvelopeParts {
        graph_id,
        source_instance_id,
        event_key,
        display_name: measurement.display_name().to_owned(),
        committed_at_utc,
        observation: None,
        lifecycle_fact: Some(code.unwrap_or_else(|| "initialized".to_owned())),
        policy_revision: Some(state.policy_revision()),
    })
}

fn condition_digest<'a>(
    digests: &'a BTreeMap<String, String>,
    condition_id: &str,
) -> Result<&'a String, CoreError> {
    digests.get(condition_id).ok_or_else(|| {
        CoreError::internal("condition digest calculation omitted a declared condition")
    })
}

pub(super) struct SourceEventParts<'a> {
    pub(super) graph_id: GraphId,
    pub(super) source_id: SourceId,
    pub(super) source_instance_id: SourceInstanceId,
    pub(super) source: &'a super::SourceDocument,
    pub(super) state: &'a super::SourceState,
    pub(super) event_kind: EventKind,
    pub(super) code: String,
    pub(super) committed_at_utc: String,
}

/// Materializes one source episode event before source-route admission.
pub(super) fn materialize_source_event(
    parts: SourceEventParts<'_>,
) -> Result<EventEnvelope, CoreError> {
    let SourceEventParts {
        graph_id,
        source_id,
        source_instance_id,
        source,
        state,
        event_kind,
        code,
        committed_at_utc,
    } = parts;
    if !matches!(
        event_kind,
        EventKind::SourceEscalation | EventKind::SourceIntegrationFault
    ) {
        return Err(CoreError::contract(
            "source event materializer received a non-source episode kind",
        ));
    }
    EventEnvelope::new(EventEnvelopeParts {
        graph_id: graph_id.clone(),
        source_instance_id: source_instance_id.clone(),
        event_key: EventKey::SourceEpisode {
            graph_id,
            source_id,
            source_instance_id,
            event_kind,
            code: code.clone(),
            source_episode_seq: state.source_episode_seq(),
            source_representation_digest: source.source_representation_digest()?,
        },
        display_name: source.display_name().to_owned(),
        committed_at_utc,
        observation: None,
        lifecycle_fact: Some(code),
        policy_revision: None,
    })
}

/// Materializes a source lifecycle event for a fresh source lineage.
pub(super) fn materialize_source_initialized(
    graph_id: GraphId,
    source_id: SourceId,
    source_instance_id: SourceInstanceId,
    source: &super::SourceDocument,
    committed_at_utc: String,
) -> Result<EventEnvelope, CoreError> {
    EventEnvelope::new(EventEnvelopeParts {
        graph_id: graph_id.clone(),
        source_instance_id: source_instance_id.clone(),
        event_key: EventKey::SourceLifecycle {
            graph_id,
            source_id,
            source_instance_id,
            event_kind: EventKind::SourceInitialized,
            source_representation_digest: source.source_representation_digest()?,
        },
        display_name: source.display_name().to_owned(),
        committed_at_utc,
        observation: None,
        lifecycle_fact: Some("initialized".to_owned()),
        policy_revision: None,
    })
}

#[cfg(test)]
#[path = "event_materialize/tests.rs"]
mod tests;

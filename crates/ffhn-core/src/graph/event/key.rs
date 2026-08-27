//! Event keys: all deterministic identity material and nothing observationally timed.

use serde::{Deserialize, Serialize};

use crate::{ConditionId, CoreError};

use super::super::{GraphId, MeasurementId, MeasurementInstanceId, SourceId, SourceInstanceId};

/// Stable kind of an immutable event envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// A condition satisfied its predicate-specific trigger rule.
    ConditionSatisfied,
    /// A condition could not complete an exact evaluation.
    ConditionEvaluationIssue,
    /// One measurement acquired its first accepted observation.
    MeasurementInitialized,
    /// A measurement extraction-health episode reached escalation.
    ExtractionEscalation,
    /// A measurement-scoped integration-fault episode began.
    MeasurementIntegrationFault,
    /// One source lineage was initialized.
    SourceInitialized,
    /// A source acquisition-health episode reached escalation.
    SourceEscalation,
    /// A source-scoped integration-fault episode began.
    SourceIntegrationFault,
}

/// All fields that determine a route-independent event identity.
///
/// The shape has no wall-clock member. Constructors force every condition event to bind its
/// definition digest and every episode event to bind its monotonic episode sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventKey {
    /// Predicate satisfaction at one accepted observation.
    ConditionSatisfied {
        /// Graph lineage root.
        graph_id: GraphId,
        /// Owning source directory identifier.
        source_id: SourceId,
        /// Source lineage token.
        source_instance_id: SourceInstanceId,
        /// Owning measurement identifier.
        measurement_id: MeasurementId,
        /// Measurement lineage token.
        measurement_instance_id: MeasurementInstanceId,
        /// Stable condition identifier.
        condition_id: ConditionId,
        /// Digest of the normalized condition definition.
        condition_defn_digest: String,
        /// Accepted-observation sequence.
        observation_seq: u64,
    },
    /// A condition evaluation issue at one accepted observation.
    ConditionEvaluationIssue {
        /// Graph lineage root.
        graph_id: GraphId,
        /// Owning source directory identifier.
        source_id: SourceId,
        /// Source lineage token.
        source_instance_id: SourceInstanceId,
        /// Owning measurement identifier.
        measurement_id: MeasurementId,
        /// Measurement lineage token.
        measurement_instance_id: MeasurementInstanceId,
        /// Closed issue code.
        issue_code: String,
        /// Stable condition identifier.
        condition_id: ConditionId,
        /// Digest of the normalized condition definition.
        condition_defn_digest: String,
        /// Accepted-observation sequence.
        observation_seq: u64,
    },
    /// One measurement lifecycle event, identified by its value contract.
    MeasurementLifecycle {
        /// Graph lineage root.
        graph_id: GraphId,
        /// Owning source directory identifier.
        source_id: SourceId,
        /// Source lineage token.
        source_instance_id: SourceInstanceId,
        /// Owning measurement identifier.
        measurement_id: MeasurementId,
        /// Measurement lineage token.
        measurement_instance_id: MeasurementInstanceId,
        /// Only `measurement_initialized`.
        event_kind: EventKind,
        /// Measurement value digest.
        measurement_value_digest: String,
    },
    /// One measurement episode event, identified by its code and episode sequence.
    MeasurementEpisode {
        /// Graph lineage root.
        graph_id: GraphId,
        /// Owning source directory identifier.
        source_id: SourceId,
        /// Source lineage token.
        source_instance_id: SourceInstanceId,
        /// Owning measurement identifier.
        measurement_id: MeasurementId,
        /// Measurement lineage token.
        measurement_instance_id: MeasurementInstanceId,
        /// Only `extraction_escalation` or `measurement_integration_fault`.
        event_kind: EventKind,
        /// Stable reason or fault code.
        code: String,
        /// Measurement episode sequence.
        measurement_episode_seq: u64,
        /// Measurement value digest.
        measurement_value_digest: String,
    },
    /// One source lifecycle event, identified by the source representation contract.
    SourceLifecycle {
        /// Graph lineage root.
        graph_id: GraphId,
        /// Owning source directory identifier.
        source_id: SourceId,
        /// Source lineage token.
        source_instance_id: SourceInstanceId,
        /// Only `source_initialized`.
        event_kind: EventKind,
        /// Source representation digest.
        source_representation_digest: String,
    },
    /// One source episode event, identified by its code and episode sequence.
    SourceEpisode {
        /// Graph lineage root.
        graph_id: GraphId,
        /// Owning source directory identifier.
        source_id: SourceId,
        /// Source lineage token.
        source_instance_id: SourceInstanceId,
        /// `source_escalation` or `source_integration_fault`.
        event_kind: EventKind,
        /// Stable reason, error, or fault code.
        code: String,
        /// Source episode sequence.
        source_episode_seq: u64,
        /// Source representation digest.
        source_representation_digest: String,
    },
}

impl EventKey {
    /// Returns the graph lineage root bound into this event identity.
    pub const fn graph_id(&self) -> &GraphId {
        match self {
            Self::ConditionSatisfied { graph_id, .. }
            | Self::ConditionEvaluationIssue { graph_id, .. }
            | Self::MeasurementLifecycle { graph_id, .. }
            | Self::MeasurementEpisode { graph_id, .. }
            | Self::SourceLifecycle { graph_id, .. }
            | Self::SourceEpisode { graph_id, .. } => graph_id,
        }
    }

    /// Returns the source-lineage token bound into this event identity.
    pub const fn source_instance_id(&self) -> &SourceInstanceId {
        match self {
            Self::ConditionSatisfied {
                source_instance_id, ..
            }
            | Self::ConditionEvaluationIssue {
                source_instance_id, ..
            }
            | Self::MeasurementLifecycle {
                source_instance_id, ..
            }
            | Self::MeasurementEpisode {
                source_instance_id, ..
            }
            | Self::SourceLifecycle {
                source_instance_id, ..
            }
            | Self::SourceEpisode {
                source_instance_id, ..
            } => source_instance_id,
        }
    }

    /// Returns the source-directory identifier bound into this event identity.
    pub const fn source_id(&self) -> &SourceId {
        match self {
            Self::ConditionSatisfied { source_id, .. }
            | Self::ConditionEvaluationIssue { source_id, .. }
            | Self::MeasurementLifecycle { source_id, .. }
            | Self::MeasurementEpisode { source_id, .. }
            | Self::SourceLifecycle { source_id, .. }
            | Self::SourceEpisode { source_id, .. } => source_id,
        }
    }

    /// Returns measurement identity when this key belongs to a measurement emitter.
    pub const fn measurement_lineage(&self) -> Option<(&MeasurementId, &MeasurementInstanceId)> {
        match self {
            Self::ConditionSatisfied {
                measurement_id,
                measurement_instance_id,
                ..
            }
            | Self::ConditionEvaluationIssue {
                measurement_id,
                measurement_instance_id,
                ..
            }
            | Self::MeasurementLifecycle {
                measurement_id,
                measurement_instance_id,
                ..
            }
            | Self::MeasurementEpisode {
                measurement_id,
                measurement_instance_id,
                ..
            } => Some((measurement_id, measurement_instance_id)),
            Self::SourceLifecycle { .. } | Self::SourceEpisode { .. } => None,
        }
    }

    /// Returns the kind structurally encoded by this identity key.
    pub const fn event_kind(&self) -> EventKind {
        match self {
            Self::ConditionSatisfied { .. } => EventKind::ConditionSatisfied,
            Self::ConditionEvaluationIssue { .. } => EventKind::ConditionEvaluationIssue,
            Self::MeasurementLifecycle { event_kind, .. }
            | Self::MeasurementEpisode { event_kind, .. }
            | Self::SourceLifecycle { event_kind, .. }
            | Self::SourceEpisode { event_kind, .. } => *event_kind,
        }
    }

    /// Computes the route-independent identifier from stable key JSON.
    pub fn event_id(&self) -> Result<String, CoreError> {
        self.validate()?;
        crate::stable_json::stable_digest(self)
    }

    /// Validates event-kind scope, UUID lineage, digest forms, and nonzero sequences.
    pub fn validate(&self) -> Result<(), CoreError> {
        match self {
            Self::ConditionSatisfied {
                graph_id,
                source_instance_id,
                measurement_instance_id,
                condition_id,
                condition_defn_digest,
                observation_seq,
                ..
            } => validate_measurement_condition(
                graph_id,
                source_instance_id,
                measurement_instance_id,
                condition_id,
                condition_defn_digest,
                *observation_seq,
            ),
            Self::ConditionEvaluationIssue {
                graph_id,
                source_instance_id,
                measurement_instance_id,
                issue_code,
                condition_id,
                condition_defn_digest,
                observation_seq,
                ..
            } => {
                validate_measurement_condition(
                    graph_id,
                    source_instance_id,
                    measurement_instance_id,
                    condition_id,
                    condition_defn_digest,
                    *observation_seq,
                )?;
                require_one_of(
                    "condition issue",
                    issue_code,
                    &["unavailable", "arithmetic_overflow", "zero_reference"],
                )
            }
            Self::MeasurementLifecycle {
                graph_id,
                source_instance_id,
                measurement_instance_id,
                event_kind,
                measurement_value_digest,
                ..
            } => {
                validate_measurement_ids(graph_id, source_instance_id, measurement_instance_id)?;
                if *event_kind != EventKind::MeasurementInitialized {
                    return Err(CoreError::contract(
                        "measurement lifecycle key has an invalid event kind",
                    ));
                }
                require_sha256("measurement value digest", measurement_value_digest)
            }
            Self::MeasurementEpisode {
                graph_id,
                source_instance_id,
                measurement_instance_id,
                event_kind,
                code,
                measurement_episode_seq,
                measurement_value_digest,
                ..
            } => {
                validate_measurement_ids(graph_id, source_instance_id, measurement_instance_id)?;
                if !matches!(
                    event_kind,
                    EventKind::ExtractionEscalation | EventKind::MeasurementIntegrationFault
                ) {
                    return Err(CoreError::contract(
                        "measurement episode key has an invalid event kind",
                    ));
                }
                if *event_kind == EventKind::ExtractionEscalation {
                    require_one_of(
                        "measurement extraction episode",
                        code,
                        &[
                            "json_malformed",
                            "json_missing_pointer_target",
                            "json_non_scalar_pointer_target",
                            "htmlcut_no_match",
                            "htmlcut_ambiguous_match",
                            "htmlcut_missing_attribute",
                            "htmlcut_match_index_out_of_range",
                            "value_unparseable",
                        ],
                    )?;
                } else {
                    require_one_of(
                        "measurement integration episode",
                        code,
                        &[
                            "htmlcut_internal_error",
                            "ffhn_boundary_invariant_violation",
                            "ffhn_policy_invariant_violation",
                        ],
                    )?;
                }
                require_positive("measurement episode sequence", *measurement_episode_seq)?;
                require_sha256("measurement value digest", measurement_value_digest)
            }
            Self::SourceLifecycle {
                graph_id,
                source_instance_id,
                event_kind,
                source_representation_digest,
                ..
            } => {
                validate_source_ids(graph_id, source_instance_id)?;
                if *event_kind != EventKind::SourceInitialized {
                    return Err(CoreError::contract(
                        "source lifecycle key has an invalid event kind",
                    ));
                }
                require_sha256("source representation digest", source_representation_digest)
            }
            Self::SourceEpisode {
                graph_id,
                source_instance_id,
                event_kind,
                code,
                source_episode_seq,
                source_representation_digest,
                ..
            } => {
                validate_source_ids(graph_id, source_instance_id)?;
                if !matches!(
                    event_kind,
                    EventKind::SourceEscalation | EventKind::SourceIntegrationFault
                ) {
                    return Err(CoreError::contract(
                        "source episode key has an invalid event kind",
                    ));
                }
                if *event_kind == EventKind::SourceEscalation {
                    require_one_of(
                        "source acquisition episode",
                        code,
                        &[
                            "dns_error",
                            "connect_failed",
                            "connect_timeout",
                            "read_timeout",
                            "connection_reset",
                            "total_timeout",
                            "tls_error",
                            "incomplete_body",
                            "too_many_redirects",
                            "redirect_loop",
                            "redirect_downgrade",
                            "content_length_exceeded",
                            "body_bytes_exceeded",
                            "invalid_utf8",
                            "http_status",
                            "http_success_not_representation",
                            "file_not_found",
                            "file_permission_denied",
                            "file_not_regular",
                            "file_read_error",
                            "io_unclassified",
                        ],
                    )?;
                } else {
                    require_one_of("source integration episode", code, &["secret_unavailable"])?;
                }
                require_positive("source episode sequence", *source_episode_seq)?;
                require_sha256("source representation digest", source_representation_digest)
            }
        }
    }
}

impl EventKind {
    /// Returns the sole route family permitted to receive this event kind.
    pub const fn route_family(self) -> super::super::GraphRouteFamily {
        match self {
            Self::ConditionSatisfied => super::super::GraphRouteFamily::OnCondition,
            Self::ConditionEvaluationIssue
            | Self::MeasurementInitialized
            | Self::ExtractionEscalation
            | Self::MeasurementIntegrationFault => super::super::GraphRouteFamily::OnMeasurement,
            Self::SourceInitialized | Self::SourceEscalation | Self::SourceIntegrationFault => {
                super::super::GraphRouteFamily::OnSource
            }
        }
    }
}

pub(super) fn validate_measurement_condition(
    graph_id: &GraphId,
    source_instance_id: &SourceInstanceId,
    measurement_instance_id: &MeasurementInstanceId,
    _condition_id: &ConditionId,
    condition_defn_digest: &str,
    observation_seq: u64,
) -> Result<(), CoreError> {
    validate_measurement_ids(graph_id, source_instance_id, measurement_instance_id)?;
    require_sha256("condition definition digest", condition_defn_digest)?;
    require_positive("observation sequence", observation_seq)
}

pub(super) fn validate_measurement_ids(
    graph_id: &GraphId,
    source_instance_id: &SourceInstanceId,
    measurement_instance_id: &MeasurementInstanceId,
) -> Result<(), CoreError> {
    validate_source_ids(graph_id, source_instance_id)?;
    measurement_instance_id.validate()
}

pub(super) fn validate_source_ids(
    graph_id: &GraphId,
    source_instance_id: &SourceInstanceId,
) -> Result<(), CoreError> {
    graph_id.validate()?;
    source_instance_id.validate()
}

fn require_positive(field: &str, value: u64) -> Result<(), CoreError> {
    if value == 0 {
        Err(CoreError::contract(format!("{field} must be positive")))
    } else {
        Ok(())
    }
}

pub(super) fn require_code(field: &str, value: &str) -> Result<(), CoreError> {
    let bytes = value.as_bytes();
    if bytes.first().is_none_or(|byte| !byte.is_ascii_lowercase())
        || bytes
            .last()
            .is_none_or(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit()))
        || bytes
            .iter()
            .any(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_'))
        || bytes.windows(2).any(|pair| pair == b"__")
    {
        Err(CoreError::contract(format!(
            "{field} must be a lowercase alphanumeric underscore code"
        )))
    } else {
        Ok(())
    }
}

fn require_one_of(field: &str, value: &str, allowed: &[&str]) -> Result<(), CoreError> {
    require_code(field, value)?;
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(CoreError::contract(format!(
            "{field} is outside its closed vocabulary"
        )))
    }
}

fn require_sha256(field: &str, value: &str) -> Result<(), CoreError> {
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

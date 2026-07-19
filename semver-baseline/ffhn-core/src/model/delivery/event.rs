use serde::{Deserialize, Serialize};

use crate::{
    ConditionId, CoreError, IntegrationFaultCode, PermanentErrorCode, SourceSuspectReason, TargetId,
};

use super::payload::{
    ProcessStdinPayload, require_canonical_timestamp, require_contract_digest,
    require_matching_condition_facts,
};

/// The route family selected by a staged event eligibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteFamily {
    /// Immediate lifecycle, health, permanent-error, or evaluation-issue routing.
    OnRun,
    /// Routing for a named condition's trigger.
    OnCondition,
}

impl RouteFamily {
    /// Returns the stable target-config and payload spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OnRun => "on_run",
            Self::OnCondition => "on_condition",
        }
    }
}

/// The closed domain vocabulary for every staged FFHN delivery event.
///
/// This is delivery-domain metadata, not a process-stdin implementation detail. The current
/// process-stdin adapter serializes it, while reports and durable outbox records retain the same
/// fact without decoding adapter payload bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryEventKind {
    /// One named condition satisfied its routing rule.
    ConditionSatisfied,
    /// A target accepted its first valid observation.
    Initialized,
    /// The target's owned state was blindly reset.
    Reset,
    /// A valid condition evaluation overflowed checked arithmetic.
    ArithmeticOverflow,
    /// A valid percentage condition encountered a zero runtime reference.
    ZeroReference,
    /// A source-suspect episode reached its escalation threshold.
    SourceSuspectEscalated,
    /// A permanent target-contract-error episode began.
    PermanentContractError,
    /// An FFHN integration-fault episode began.
    IntegrationFault,
}

impl DeliveryEventKind {
    /// Returns the stable serialized event-kind spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConditionSatisfied => "condition_satisfied",
            Self::Initialized => "initialized",
            Self::Reset => "reset",
            Self::ArithmeticOverflow => "arithmetic_overflow",
            Self::ZeroReference => "zero_reference",
            Self::SourceSuspectEscalated => "source_suspect_escalated",
            Self::PermanentContractError => "permanent_contract_error",
            Self::IntegrationFault => "integration_fault",
        }
    }
}

/// The complete, typed fact set from which one deterministic delivery-event identity is derived.
///
/// This is persisted alongside the rendered payload so a state load can recompute the event id
/// rather than trusting a syntactically valid digest supplied by the stored record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "key_kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ProcessStdinEventKey {
    /// An event-predicate condition became eligible at an accepted observation.
    ConditionEvent {
        /// The configured condition that became eligible.
        condition_id: ConditionId,
        /// The accepted-observation sequence that discriminates this event.
        observation_seq: u64,
    },
    /// A level condition entered its eligible state.
    ConditionLevel {
        /// The configured condition that entered its eligible state.
        condition_id: ConditionId,
        /// The persisted transition instant that discriminates this level entry.
        entry_at: String,
    },
    /// The target accepted its first valid observation under one measurement contract.
    Initialized {
        /// The measurement contract that defines this lifecycle event.
        contract_digest_sha256: String,
    },
    /// The target's owned state was explicitly reset under one measurement contract.
    Reset {
        /// The measurement contract that defines this lifecycle event.
        contract_digest_sha256: String,
    },
    /// A checked condition evaluation overflowed at an accepted observation.
    ArithmeticOverflow {
        /// The configured condition that overflowed.
        condition_id: ConditionId,
        /// The accepted-observation sequence that produced the overflow.
        observation_seq: u64,
    },
    /// A percentage condition encountered its valid zero runtime reference.
    ZeroReference {
        /// The configured condition that encountered a zero reference.
        condition_id: ConditionId,
        /// The accepted-observation sequence that produced the issue.
        observation_seq: u64,
    },
    /// A source-health episode reached its configured escalation threshold.
    SourceSuspectEscalated {
        /// The class of source-health episode.
        reason_class: SourceSuspectReason,
        /// The first unresolved instant of that episode.
        episode: String,
    },
    /// A permanent target-contract-error episode began.
    PermanentContractError {
        /// The measurement contract that produced the permanent error.
        contract_digest_sha256: String,
        /// The permanent error class.
        error_code: PermanentErrorCode,
        /// The first observed instant of this error episode.
        first_seen_at: String,
    },
    /// An FFHN integration-fault episode began.
    IntegrationFault {
        /// The measurement contract that defines this integration event.
        contract_digest_sha256: String,
        /// The closed adapter-boundary fault class.
        integration_fault_code: IntegrationFaultCode,
        /// The first observed instant of this integration-fault episode.
        first_seen_at: String,
    },
}

impl ProcessStdinEventKey {
    /// Returns the external event vocabulary represented by this identity key.
    pub(crate) const fn event_kind(&self) -> DeliveryEventKind {
        match self {
            Self::ConditionEvent { .. } | Self::ConditionLevel { .. } => {
                DeliveryEventKind::ConditionSatisfied
            }
            Self::Initialized { .. } => DeliveryEventKind::Initialized,
            Self::Reset { .. } => DeliveryEventKind::Reset,
            Self::ArithmeticOverflow { .. } => DeliveryEventKind::ArithmeticOverflow,
            Self::ZeroReference { .. } => DeliveryEventKind::ZeroReference,
            Self::SourceSuspectEscalated { .. } => DeliveryEventKind::SourceSuspectEscalated,
            Self::PermanentContractError { .. } => DeliveryEventKind::PermanentContractError,
            Self::IntegrationFault { .. } => DeliveryEventKind::IntegrationFault,
        }
    }

    /// Computes the deterministic event identity for one target-local route family.
    pub(crate) fn event_id(
        &self,
        target_id: &TargetId,
        route_family: RouteFamily,
    ) -> Result<String, CoreError> {
        crate::stable_json::stable_digest(&serde_json::json!({
            "target_id": target_id,
            "route_family": route_family,
            "key": self.identity_json(),
        }))
    }

    pub(super) fn identity_json(&self) -> serde_json::Value {
        match self {
            Self::ConditionEvent {
                condition_id,
                observation_seq,
            } => serde_json::json!({
                "condition_id": condition_id,
                "observation_seq": observation_seq,
            }),
            Self::ConditionLevel {
                condition_id,
                entry_at,
            } => serde_json::json!({
                "condition_id": condition_id,
                "entry_at": entry_at,
            }),
            Self::Initialized {
                contract_digest_sha256,
            } => serde_json::json!({
                "kind": "initialized",
                "contract_digest_sha256": contract_digest_sha256,
            }),
            Self::Reset {
                contract_digest_sha256,
            } => serde_json::json!({
                "kind": "reset",
                "contract_digest_sha256": contract_digest_sha256,
            }),
            Self::ArithmeticOverflow {
                condition_id,
                observation_seq,
            } => serde_json::json!({
                "condition_id": condition_id,
                "kind": "arithmetic_overflow",
                "observation_seq": observation_seq,
            }),
            Self::ZeroReference {
                condition_id,
                observation_seq,
            } => serde_json::json!({
                "condition_id": condition_id,
                "kind": "zero_reference",
                "observation_seq": observation_seq,
            }),
            Self::SourceSuspectEscalated {
                reason_class,
                episode,
            } => serde_json::json!({
                "reason_class": reason_class,
                "episode": episode,
            }),
            Self::PermanentContractError {
                contract_digest_sha256,
                error_code,
                first_seen_at,
            } => serde_json::json!({
                "contract_digest_sha256": contract_digest_sha256,
                "error_code": error_code,
                "first_seen_at": first_seen_at,
            }),
            Self::IntegrationFault {
                contract_digest_sha256,
                integration_fault_code,
                first_seen_at,
            } => serde_json::json!({
                "contract_digest_sha256": contract_digest_sha256,
                "integration_fault_code": integration_fault_code,
                "first_seen_at": first_seen_at,
            }),
        }
    }

    pub(super) fn validate(
        &self,
        expected_contract_digest_sha256: Option<&str>,
    ) -> Result<(), CoreError> {
        match self {
            Self::ConditionEvent {
                observation_seq, ..
            }
            | Self::ArithmeticOverflow {
                observation_seq, ..
            }
            | Self::ZeroReference {
                observation_seq, ..
            } if *observation_seq == 0 => Err(CoreError::contract(
                "outbox immutable_payload event_key observation_seq must be positive",
            )),
            Self::ConditionLevel { entry_at, .. } => {
                require_canonical_timestamp("outbox immutable_payload event_key entry_at", entry_at)
            }
            Self::Initialized {
                contract_digest_sha256: key_digest,
            }
            | Self::Reset {
                contract_digest_sha256: key_digest,
            } => require_contract_digest(key_digest, expected_contract_digest_sha256),
            Self::SourceSuspectEscalated { episode, .. } => {
                require_canonical_timestamp("outbox immutable_payload event_key episode", episode)
            }
            Self::PermanentContractError {
                contract_digest_sha256: key_digest,
                first_seen_at,
                ..
            } => {
                require_contract_digest(key_digest, expected_contract_digest_sha256)?;
                require_canonical_timestamp(
                    "outbox immutable_payload event_key first_seen_at",
                    first_seen_at,
                )
            }
            Self::IntegrationFault {
                contract_digest_sha256: key_digest,
                first_seen_at,
                ..
            } => {
                require_contract_digest(key_digest, expected_contract_digest_sha256)?;
                require_canonical_timestamp(
                    "outbox immutable_payload event_key first_seen_at",
                    first_seen_at,
                )
            }
            _ => Ok(()),
        }
    }

    pub(super) fn validate_payload_facts(
        &self,
        payload: &ProcessStdinPayload,
    ) -> Result<(), CoreError> {
        match self {
            Self::ConditionEvent {
                condition_id,
                observation_seq,
            }
            | Self::ArithmeticOverflow {
                condition_id,
                observation_seq,
            }
            | Self::ZeroReference {
                condition_id,
                observation_seq,
            } => require_matching_condition_facts(payload, condition_id, *observation_seq),
            Self::ConditionLevel { condition_id, .. } => {
                let payload_condition = payload.condition_id.as_ref().ok_or_else(|| {
                    CoreError::contract(
                        "outbox immutable_payload event_key condition_id must match payload facts",
                    )
                })?;
                if payload_condition != condition_id {
                    return Err(CoreError::contract(
                        "outbox immutable_payload event_key condition_id must match payload facts",
                    ));
                }
                Ok(())
            }
            Self::Initialized { .. } | Self::Reset { .. } => Ok(()),
            Self::SourceSuspectEscalated {
                reason_class,
                episode,
            } => {
                if payload.reason_class != Some(*reason_class)
                    || payload.episode_started_at.as_deref() != Some(episode)
                {
                    return Err(CoreError::contract(
                        "outbox immutable_payload event_key source episode must match payload facts",
                    ));
                }
                Ok(())
            }
            Self::PermanentContractError {
                error_code,
                first_seen_at,
                ..
            } => {
                if payload.error_code != Some(*error_code)
                    || payload.episode_started_at.as_deref() != Some(first_seen_at)
                {
                    return Err(CoreError::contract(
                        "outbox immutable_payload event_key permanent episode must match payload facts",
                    ));
                }
                Ok(())
            }
            Self::IntegrationFault {
                integration_fault_code,
                first_seen_at,
                ..
            } => {
                if payload.integration_fault_code != Some(*integration_fault_code)
                    || payload.episode_started_at.as_deref() != Some(first_seen_at)
                {
                    return Err(CoreError::contract(
                        "outbox immutable_payload event_key integration episode must match payload facts",
                    ));
                }
                Ok(())
            }
        }
    }
}

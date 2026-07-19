use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

use crate::{
    ConditionId, CoreError, IntegrationFaultCode, PermanentErrorCode, SourceSuspectReason, TargetId,
};

use super::{
    event::{DeliveryEventKind, ProcessStdinEventKey, RouteFamily},
    route::{RouteId, require_text},
};

/// Versioned, self-contained payload persisted for a process-stdin delivery record.
///
/// The byte representation is deliberately part of the state contract: a record is eligible for
/// delivery only when it decodes as this exact schema and canonically binds to its enclosing
/// target, event, and route identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProcessStdinPayload {
    pub(super) schema_name: String,
    pub(super) schema_version: u32,
    pub(super) event_id: String,
    pub(super) route_id: RouteId,
    pub(super) route_family: RouteFamily,
    pub(super) target_id: TargetId,
    pub(super) display_name: String,
    pub(super) event_key: ProcessStdinEventKey,
    pub(super) event_kind: DeliveryEventKind,
    pub(super) summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) condition_id: Option<ConditionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) observation_seq: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) canonical_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reason_class: Option<SourceSuspectReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error_code: Option<PermanentErrorCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) integration_fault_code: Option<IntegrationFaultCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) episode_started_at: Option<String>,
}

impl ProcessStdinPayload {
    /// Builds one version-three payload from already staged delivery facts.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        route_id: &RouteId,
        route_family: RouteFamily,
        target_id: &TargetId,
        display_name: &str,
        event_key: ProcessStdinEventKey,
        summary: &str,
        condition_id: Option<ConditionId>,
        observation_seq: Option<u64>,
        canonical_value: Option<String>,
        reason_class: Option<SourceSuspectReason>,
        error_code: Option<PermanentErrorCode>,
        integration_fault_code: Option<IntegrationFaultCode>,
        episode_started_at: Option<String>,
    ) -> Result<Self, CoreError> {
        event_key.validate(None)?;
        let event_id = event_key.event_id(target_id, route_family)?;
        Ok(Self {
            schema_name: "ffhn.process_stdin".to_owned(),
            schema_version: 4,
            event_id,
            route_id: route_id.clone(),
            route_family,
            target_id: target_id.clone(),
            display_name: display_name.to_owned(),
            event_kind: event_key.event_kind(),
            event_key,
            summary: summary.to_owned(),
            condition_id,
            observation_seq,
            canonical_value,
            reason_class,
            error_code,
            integration_fault_code,
            episode_started_at,
        })
    }

    /// Serializes this payload into the exact bytes retained in state and sent on delivery.
    pub(crate) fn immutable_bytes(&self) -> Result<Vec<u8>, CoreError> {
        Ok(crate::stable_json::stable_json(self)?.into_bytes())
    }

    pub(super) fn validate_for_record(
        &self,
        target_id: &TargetId,
        contract_digest_sha256: &str,
        event_id: &str,
        route_id: &RouteId,
        route_family: RouteFamily,
    ) -> Result<(), CoreError> {
        if self.schema_name != "ffhn.process_stdin" || self.schema_version != 4 {
            return Err(CoreError::contract(
                "outbox immutable_payload must be an ffhn.process_stdin version 4 payload",
            ));
        }
        if self.target_id != *target_id
            || self.event_id != event_id
            || self.route_id != *route_id
            || self.route_family != route_family
        {
            return Err(CoreError::contract(
                "outbox immutable_payload identity must match its enclosing pending record",
            ));
        }
        require_text("outbox immutable_payload display_name", &self.display_name)?;
        require_text("outbox immutable_payload summary", &self.summary)?;
        self.event_key.validate(Some(contract_digest_sha256))?;
        if self.event_kind != self.event_key.event_kind() {
            return Err(CoreError::contract(
                "outbox immutable_payload event_kind must match its event_key",
            ));
        }
        if self.event_key.event_id(target_id, route_family)? != event_id {
            return Err(CoreError::contract(
                "outbox event_id must be derived from its persisted event_key",
            ));
        }

        let has_condition_fact = self.condition_id.is_some()
            || self.observation_seq.is_some()
            || self.canonical_value.is_some();
        let has_episode_fact = self.reason_class.is_some()
            || self.error_code.is_some()
            || self.integration_fault_code.is_some()
            || self.episode_started_at.is_some();
        match self.event_kind {
            DeliveryEventKind::ConditionSatisfied => {
                require_condition_fact(self, "condition_satisfied")?;
                require_route_family(self, RouteFamily::OnCondition)?;
                if has_episode_fact {
                    return Err(CoreError::contract(
                        "condition_satisfied payload must not contain episode fields",
                    ));
                }
            }
            DeliveryEventKind::Initialized => {
                require_observation_fact(self, "initialized")?;
                require_route_family(self, RouteFamily::OnRun)?;
                if self.condition_id.is_some() || has_episode_fact {
                    return Err(CoreError::contract(
                        "initialized payload must contain only its observation facts",
                    ));
                }
            }
            DeliveryEventKind::Reset => {
                require_route_family(self, RouteFamily::OnRun)?;
                if has_condition_fact || has_episode_fact {
                    return Err(CoreError::contract(
                        "reset payload must not contain observation, condition, or episode facts",
                    ));
                }
            }
            DeliveryEventKind::ArithmeticOverflow | DeliveryEventKind::ZeroReference => {
                require_condition_fact(self, "condition-evaluation issue")?;
                require_route_family(self, RouteFamily::OnRun)?;
                if has_episode_fact {
                    return Err(CoreError::contract(
                        "condition-evaluation issue payload must not contain episode fields",
                    ));
                }
            }
            DeliveryEventKind::SourceSuspectEscalated => {
                require_route_family(self, RouteFamily::OnRun)?;
                if has_condition_fact
                    || self.reason_class.is_none()
                    || self.error_code.is_some()
                    || self.integration_fault_code.is_some()
                    || self.episode_started_at.is_none()
                {
                    return Err(CoreError::contract(
                        "source_suspect_escalated payload must contain only a source episode",
                    ));
                }
            }
            DeliveryEventKind::PermanentContractError => {
                require_route_family(self, RouteFamily::OnRun)?;
                if has_condition_fact
                    || self.reason_class.is_some()
                    || self.error_code.is_none()
                    || self.integration_fault_code.is_some()
                    || self.episode_started_at.is_none()
                {
                    return Err(CoreError::contract(
                        "permanent_contract_error payload must contain only a permanent-error episode",
                    ));
                }
            }
            DeliveryEventKind::IntegrationFault => {
                require_route_family(self, RouteFamily::OnRun)?;
                if has_condition_fact
                    || self.reason_class.is_some()
                    || self.error_code.is_some()
                    || self.integration_fault_code.is_none()
                    || self.episode_started_at.is_none()
                {
                    return Err(CoreError::contract(
                        "integration_fault payload must contain only an integration-fault episode",
                    ));
                }
            }
        }
        if let Some(episode_started_at) = &self.episode_started_at {
            require_canonical_timestamp(
                "outbox immutable_payload episode_started_at",
                episode_started_at,
            )?;
        }
        self.event_key.validate_payload_facts(self)?;
        if self.summary != self.expected_summary()? {
            return Err(CoreError::contract(
                "outbox immutable_payload summary must match its staged event facts",
            ));
        }
        Ok(())
    }

    /// Returns this payload's deterministic event identity.
    pub(crate) fn event_id(&self) -> &str {
        &self.event_id
    }

    /// Returns the closed domain event kind carried by this immutable adapter payload.
    pub(crate) const fn event_kind(&self) -> DeliveryEventKind {
        self.event_kind
    }

    /// Returns the condition identity carried by a condition-scoped event, when any.
    pub(crate) fn condition_id(&self) -> Option<&ConditionId> {
        self.condition_id.as_ref()
    }

    pub(super) fn expected_summary(&self) -> Result<String, CoreError> {
        let condition_id = || {
            self.condition_id.as_ref().ok_or_else(|| {
                CoreError::internal("validated payload lost its required condition id")
            })
        };
        let observation_seq = || {
            self.observation_seq.ok_or_else(|| {
                CoreError::internal("validated payload lost its required observation sequence")
            })
        };
        match self.event_kind {
            DeliveryEventKind::ConditionSatisfied => Ok(format!(
                "{}[condition={}]: satisfied at observation {} (value={})",
                self.display_name,
                condition_id()?,
                observation_seq()?,
                self.canonical_value.as_deref().ok_or_else(|| {
                    CoreError::internal("validated payload lost its required canonical value")
                })?,
            )),
            DeliveryEventKind::Initialized => Ok(format!("{}: initialized", self.display_name)),
            DeliveryEventKind::Reset => Ok(format!("{}: reset", self.display_name)),
            DeliveryEventKind::ArithmeticOverflow | DeliveryEventKind::ZeroReference => {
                Ok(format!(
                    "{}[condition={}]: {} at observation {}",
                    self.display_name,
                    condition_id()?,
                    self.event_kind.as_str(),
                    observation_seq()?,
                ))
            }
            DeliveryEventKind::SourceSuspectEscalated => Ok(format!(
                "{}: source health escalated ({})",
                self.display_name,
                self.reason_class
                    .ok_or_else(|| CoreError::internal("validated payload lost its source reason"))?
                    .as_str(),
            )),
            DeliveryEventKind::PermanentContractError => Ok(format!(
                "{}: permanent contract error ({})",
                self.display_name,
                self.error_code
                    .ok_or_else(|| CoreError::internal("validated payload lost its error code"))?
                    .as_str(),
            )),
            DeliveryEventKind::IntegrationFault => Ok(format!(
                "{}: integration fault ({})",
                self.display_name,
                self.integration_fault_code
                    .ok_or_else(|| {
                        CoreError::internal("validated payload lost its integration-fault code")
                    })?
                    .as_str(),
            )),
        }
    }
}

/// Decodes canonical persisted process-stdin bytes after validating their enclosing-record facts.
pub(crate) fn read_validated_process_stdin_payload_bytes(
    bytes: &[u8],
    target_id: &TargetId,
    contract_digest_sha256: &str,
    event_id: &str,
    route_id: &RouteId,
    route_family: RouteFamily,
) -> Result<ProcessStdinPayload, CoreError> {
    let payload: ProcessStdinPayload = serde_json::from_slice(bytes)?;
    payload.validate_for_record(
        target_id,
        contract_digest_sha256,
        event_id,
        route_id,
        route_family,
    )?;
    if payload.immutable_bytes()? != bytes {
        return Err(CoreError::contract(
            "outbox immutable_payload must use FFHN canonical stable JSON bytes",
        ));
    }
    Ok(payload)
}

pub(super) fn require_route_family(
    payload: &ProcessStdinPayload,
    expected: RouteFamily,
) -> Result<(), CoreError> {
    if payload.route_family != expected {
        return Err(CoreError::contract(
            "outbox immutable_payload event kind does not match its route family",
        ));
    }
    Ok(())
}

pub(super) fn require_observation_fact(
    payload: &ProcessStdinPayload,
    label: &str,
) -> Result<(), CoreError> {
    if payload.observation_seq.is_none_or(|sequence| sequence == 0)
        || payload.canonical_value.as_deref().is_none_or(str::is_empty)
    {
        return Err(CoreError::contract(format!(
            "{label} payload must contain a positive observation sequence and canonical value",
        )));
    }
    Ok(())
}

pub(super) fn require_condition_fact(
    payload: &ProcessStdinPayload,
    label: &str,
) -> Result<(), CoreError> {
    if payload.condition_id.is_none() {
        return Err(CoreError::contract(format!(
            "{label} payload must contain a condition id",
        )));
    }
    require_observation_fact(payload, label)
}

pub(super) fn require_canonical_timestamp(field: &str, value: &str) -> Result<(), CoreError> {
    let timestamp = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| CoreError::contract(format!("{field} must be RFC 3339")))?;
    let canonical = timestamp
        .format(&Rfc3339)
        .map_err(|_| CoreError::internal("could not format timestamp"))?;
    if timestamp.offset() != UtcOffset::UTC || value != canonical {
        return Err(CoreError::contract(format!(
            "{field} must be canonical UTC RFC 3339"
        )));
    }
    Ok(())
}

pub(super) fn require_contract_digest(
    key_digest: &str,
    expected_contract_digest_sha256: Option<&str>,
) -> Result<(), CoreError> {
    let valid_digest = key_digest.len() == 64
        && key_digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'));
    if !valid_digest {
        return Err(CoreError::contract(
            "outbox immutable_payload event_key contract_digest_sha256 must be lowercase SHA-256",
        ));
    }
    if expected_contract_digest_sha256.is_some_and(|expected| expected != key_digest) {
        return Err(CoreError::contract(
            "outbox immutable_payload event_key contract_digest_sha256 must match state",
        ));
    }
    Ok(())
}

pub(super) fn require_matching_condition_facts(
    payload: &ProcessStdinPayload,
    condition_id: &ConditionId,
    observation_seq: u64,
) -> Result<(), CoreError> {
    if payload.condition_id.as_ref() != Some(condition_id)
        || payload.observation_seq != Some(observation_seq)
    {
        return Err(CoreError::contract(
            "outbox immutable_payload event_key condition facts must match payload facts",
        ));
    }
    Ok(())
}

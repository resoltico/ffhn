use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

use crate::CoreError;

use super::{ConditionId, PermanentErrorCode, SourceSuspectReason, TargetId};

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

/// The event vocabulary carried by a version-two process-stdin payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProcessStdinEventKind {
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
}

impl ProcessStdinEventKind {
    /// Returns the stable serialized event-kind spelling.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ConditionSatisfied => "condition_satisfied",
            Self::Initialized => "initialized",
            Self::Reset => "reset",
            Self::ArithmeticOverflow => "arithmetic_overflow",
            Self::ZeroReference => "zero_reference",
            Self::SourceSuspectEscalated => "source_suspect_escalated",
            Self::PermanentContractError => "permanent_contract_error",
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
}

impl ProcessStdinEventKey {
    /// Returns the external event vocabulary represented by this identity key.
    pub(crate) const fn event_kind(&self) -> ProcessStdinEventKind {
        match self {
            Self::ConditionEvent { .. } | Self::ConditionLevel { .. } => {
                ProcessStdinEventKind::ConditionSatisfied
            }
            Self::Initialized { .. } => ProcessStdinEventKind::Initialized,
            Self::Reset { .. } => ProcessStdinEventKind::Reset,
            Self::ArithmeticOverflow { .. } => ProcessStdinEventKind::ArithmeticOverflow,
            Self::ZeroReference { .. } => ProcessStdinEventKind::ZeroReference,
            Self::SourceSuspectEscalated { .. } => ProcessStdinEventKind::SourceSuspectEscalated,
            Self::PermanentContractError { .. } => ProcessStdinEventKind::PermanentContractError,
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

    fn identity_json(&self) -> serde_json::Value {
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
        }
    }

    fn validate(&self, expected_contract_digest_sha256: Option<&str>) -> Result<(), CoreError> {
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
            _ => Ok(()),
        }
    }

    fn validate_payload_facts(&self, payload: &ProcessStdinPayload) -> Result<(), CoreError> {
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
        }
    }
}

/// Versioned, self-contained payload persisted for a process-stdin delivery record.
///
/// The byte representation is deliberately part of the state contract: a record is eligible for
/// delivery only when it decodes as this exact schema and canonically binds to its enclosing
/// target, event, and route identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProcessStdinPayload {
    schema_name: String,
    schema_version: u32,
    event_id: String,
    route_id: RouteId,
    route_family: RouteFamily,
    target_id: TargetId,
    display_name: String,
    event_key: ProcessStdinEventKey,
    event_kind: ProcessStdinEventKind,
    summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    condition_id: Option<ConditionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observation_seq: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason_class: Option<SourceSuspectReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<PermanentErrorCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    episode_started_at: Option<String>,
}

impl ProcessStdinPayload {
    /// Builds one version-two payload from already staged delivery facts.
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
        episode_started_at: Option<String>,
    ) -> Result<Self, CoreError> {
        event_key.validate(None)?;
        let event_id = event_key.event_id(target_id, route_family)?;
        Ok(Self {
            schema_name: "ffhn.process_stdin".to_owned(),
            schema_version: 2,
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
            episode_started_at,
        })
    }

    /// Serializes this payload into the exact bytes retained in state and sent on delivery.
    pub(crate) fn immutable_bytes(&self) -> Result<Vec<u8>, CoreError> {
        Ok(crate::stable_json::stable_json(self)?.into_bytes())
    }

    fn validate_for_record(
        &self,
        target_id: &TargetId,
        contract_digest_sha256: &str,
        event_id: &str,
        route_id: &RouteId,
        route_family: RouteFamily,
    ) -> Result<(), CoreError> {
        if self.schema_name != "ffhn.process_stdin" || self.schema_version != 2 {
            return Err(CoreError::contract(
                "outbox immutable_payload must be an ffhn.process_stdin version 2 payload",
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
            || self.episode_started_at.is_some();
        match self.event_kind {
            ProcessStdinEventKind::ConditionSatisfied => {
                require_condition_fact(self, "condition_satisfied")?;
                require_route_family(self, RouteFamily::OnCondition)?;
                if has_episode_fact {
                    return Err(CoreError::contract(
                        "condition_satisfied payload must not contain episode fields",
                    ));
                }
            }
            ProcessStdinEventKind::Initialized => {
                require_observation_fact(self, "initialized")?;
                require_route_family(self, RouteFamily::OnRun)?;
                if self.condition_id.is_some() || has_episode_fact {
                    return Err(CoreError::contract(
                        "initialized payload must contain only its observation facts",
                    ));
                }
            }
            ProcessStdinEventKind::Reset => {
                require_route_family(self, RouteFamily::OnRun)?;
                if has_condition_fact || has_episode_fact {
                    return Err(CoreError::contract(
                        "reset payload must not contain observation, condition, or episode facts",
                    ));
                }
            }
            ProcessStdinEventKind::ArithmeticOverflow | ProcessStdinEventKind::ZeroReference => {
                require_condition_fact(self, "condition-evaluation issue")?;
                require_route_family(self, RouteFamily::OnRun)?;
                if has_episode_fact {
                    return Err(CoreError::contract(
                        "condition-evaluation issue payload must not contain episode fields",
                    ));
                }
            }
            ProcessStdinEventKind::SourceSuspectEscalated => {
                require_route_family(self, RouteFamily::OnRun)?;
                if has_condition_fact
                    || self.reason_class.is_none()
                    || self.error_code.is_some()
                    || self.episode_started_at.is_none()
                {
                    return Err(CoreError::contract(
                        "source_suspect_escalated payload must contain only a source episode",
                    ));
                }
            }
            ProcessStdinEventKind::PermanentContractError => {
                require_route_family(self, RouteFamily::OnRun)?;
                if has_condition_fact
                    || self.reason_class.is_some()
                    || self.error_code.is_none()
                    || self.episode_started_at.is_none()
                {
                    return Err(CoreError::contract(
                        "permanent_contract_error payload must contain only a permanent-error episode",
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

    fn expected_summary(&self) -> Result<String, CoreError> {
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
            ProcessStdinEventKind::ConditionSatisfied => Ok(format!(
                "{}[condition={}]: satisfied at observation {} (value={})",
                self.display_name,
                condition_id()?,
                observation_seq()?,
                self.canonical_value.as_deref().ok_or_else(|| {
                    CoreError::internal("validated payload lost its required canonical value")
                })?,
            )),
            ProcessStdinEventKind::Initialized => Ok(format!("{}: initialized", self.display_name)),
            ProcessStdinEventKind::Reset => Ok(format!("{}: reset", self.display_name)),
            ProcessStdinEventKind::ArithmeticOverflow | ProcessStdinEventKind::ZeroReference => {
                Ok(format!(
                    "{}[condition={}]: {} at observation {}",
                    self.display_name,
                    condition_id()?,
                    self.event_kind.as_str(),
                    observation_seq()?,
                ))
            }
            ProcessStdinEventKind::SourceSuspectEscalated => Ok(format!(
                "{}: source health escalated ({})",
                self.display_name,
                self.reason_class
                    .ok_or_else(|| CoreError::internal("validated payload lost its source reason"))?
                    .as_str(),
            )),
            ProcessStdinEventKind::PermanentContractError => Ok(format!(
                "{}: permanent contract error ({})",
                self.display_name,
                self.error_code
                    .ok_or_else(|| CoreError::internal("validated payload lost its error code"))?
                    .as_str(),
            )),
        }
    }
}

/// Validates persisted process-stdin bytes before a pending record can be drained.
pub(crate) fn validate_process_stdin_payload_bytes(
    bytes: &[u8],
    target_id: &TargetId,
    contract_digest_sha256: &str,
    event_id: &str,
    route_id: &RouteId,
    route_family: RouteFamily,
) -> Result<(), CoreError> {
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
    Ok(())
}

fn require_route_family(
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

fn require_observation_fact(payload: &ProcessStdinPayload, label: &str) -> Result<(), CoreError> {
    if payload.observation_seq.is_none_or(|sequence| sequence == 0)
        || payload.canonical_value.as_deref().is_none_or(str::is_empty)
    {
        return Err(CoreError::contract(format!(
            "{label} payload must contain a positive observation sequence and canonical value",
        )));
    }
    Ok(())
}

fn require_condition_fact(payload: &ProcessStdinPayload, label: &str) -> Result<(), CoreError> {
    if payload.condition_id.is_none() {
        return Err(CoreError::contract(format!(
            "{label} payload must contain a condition id",
        )));
    }
    require_observation_fact(payload, label)
}

fn require_canonical_timestamp(field: &str, value: &str) -> Result<(), CoreError> {
    let timestamp = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|error| CoreError::contract(format!("{field} must be RFC 3339: {error}")))?;
    let canonical = timestamp
        .format(&Rfc3339)
        .map_err(|error| CoreError::internal(format!("could not format timestamp: {error}")))?;
    if timestamp.offset() != UtcOffset::UTC || value != canonical {
        return Err(CoreError::contract(format!(
            "{field} must be canonical UTC RFC 3339"
        )));
    }
    Ok(())
}

fn require_contract_digest(
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

fn require_matching_condition_facts(
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

/// A stable target-local identifier for one delivery route.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RouteId(String);

impl RouteId {
    /// Parses one stable target-local route identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        validate_route_id(&value)?;
        Ok(Self(value))
    }

    /// Returns the canonical route identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RouteId {
    type Error = CoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<RouteId> for String {
    fn from(value: RouteId) -> Self {
        value.0
    }
}

impl FromStr for RouteId {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl AsRef<str> for RouteId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for RouteId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Bounded operational settings for pending delivery records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutboxPolicy {
    max_pending: usize,
    max_attempts: u32,
    base_backoff_ms: u64,
    max_backoff_ms: u64,
}

impl Default for OutboxPolicy {
    fn default() -> Self {
        Self {
            max_pending: 100,
            max_attempts: 5,
            base_backoff_ms: 1_000,
            max_backoff_ms: 300_000,
        }
    }
}

impl OutboxPolicy {
    /// Validates the bounded deterministic retry policy.
    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        if !(1..=100_000).contains(&self.max_pending) {
            return Err(CoreError::contract(
                "outbox.max_pending must be in 1..=100000",
            ));
        }
        if !(1..=100).contains(&self.max_attempts) {
            return Err(CoreError::contract(
                "outbox.max_attempts must be in 1..=100",
            ));
        }
        if !(1..=86_400_000).contains(&self.base_backoff_ms) {
            return Err(CoreError::contract(
                "outbox.base_backoff_ms must be in 1..=86400000",
            ));
        }
        if !(self.base_backoff_ms..=604_800_000).contains(&self.max_backoff_ms) {
            return Err(CoreError::contract(
                "outbox.max_backoff_ms must be at least base_backoff_ms and at most 604800000",
            ));
        }
        Ok(())
    }

    /// Returns the maximum number of pending records retained for one target.
    pub const fn max_pending(&self) -> usize {
        self.max_pending
    }

    /// Returns the number of failed attempts that terminally dead-letters a record.
    pub const fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    /// Returns the deterministic initial retry delay in milliseconds.
    pub const fn base_backoff_ms(&self) -> u64 {
        self.base_backoff_ms
    }

    /// Returns the deterministic maximum retry delay in milliseconds.
    pub const fn max_backoff_ms(&self) -> u64 {
        self.max_backoff_ms
    }
}

/// One target-local delivery route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryRoute {
    route_id: RouteId,
    route_family: RouteFamily,
    adapter: DeliveryAdapter,
}

impl DeliveryRoute {
    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        self.adapter.validate()
    }

    /// Returns the stable target-local route identifier.
    pub fn route_id(&self) -> &str {
        self.route_id.as_str()
    }

    /// Returns the family of events accepted by this route.
    pub const fn route_family(&self) -> RouteFamily {
        self.route_family
    }

    pub(crate) const fn id(&self) -> &RouteId {
        &self.route_id
    }

    pub(crate) const fn adapter(&self) -> &DeliveryAdapter {
        &self.adapter
    }
}

/// The supported durable-delivery adapter vocabulary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeliveryAdapter {
    /// Writes one immutable structured JSON payload and a newline to a process's standard input.
    ProcessStdin {
        /// Absolute executable path.
        program: String,
        /// Exact argument vector supplied to the executable.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
        /// Maximum delivery-process lifetime in milliseconds.
        timeout_ms: u64,
    },
}

impl DeliveryAdapter {
    fn validate(&self) -> Result<(), CoreError> {
        match self {
            Self::ProcessStdin {
                program,
                args,
                timeout_ms,
            } => {
                require_text("routes.adapter.program", program)?;
                if !Path::new(program).is_absolute() {
                    return Err(CoreError::contract(
                        "routes.adapter.program must be an absolute path",
                    ));
                }
                for argument in args {
                    require_text("routes.adapter.args entry", argument)?;
                }
                if !(100..=60_000).contains(timeout_ms) {
                    return Err(CoreError::contract(
                        "routes.adapter.timeout_ms must be in 100..=60000",
                    ));
                }
                Ok(())
            }
        }
    }

    pub(crate) fn process_stdin(&self) -> (&str, &[String], u64) {
        match self {
            Self::ProcessStdin {
                program,
                args,
                timeout_ms,
            } => (program, args, *timeout_ms),
        }
    }
}

pub(crate) fn validate_routes(routes: &[DeliveryRoute]) -> Result<(), CoreError> {
    let mut previous: Option<&RouteId> = None;
    for route in routes {
        if let Some(previous) = previous
            && previous >= route.id()
        {
            return Err(CoreError::contract(
                "routes must be strictly ordered by route_id without duplicates",
            ));
        }
        route.validate()?;
        previous = Some(route.id());
    }
    Ok(())
}

fn validate_route_id(value: &str) -> Result<(), CoreError> {
    if value.is_empty()
        || value.len() > 64
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
        || !value.as_bytes()[0].is_ascii_lowercase() && !value.as_bytes()[0].is_ascii_digit()
        || value.ends_with(['-', '_'])
        || value.contains("--")
        || value.contains("__")
        || value.contains("-_")
        || value.contains("_-")
    {
        return Err(CoreError::contract(
            "route_id must start with [a-z0-9], stay within 64 chars, and only use single internal '-' or '_' separators",
        ));
    }
    Ok(())
}

fn require_text(field: &str, value: &str) -> Result<(), CoreError> {
    if value.trim().is_empty() {
        Err(CoreError::contract(format!("{field} must not be empty")))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::{CoreError, PermanentErrorCode, SourceSuspectReason, TargetId};

    const CONTRACT_DIGEST: &str =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const FIRST: &str = "2026-07-15T12:00:00Z";
    const SECOND: &str = "2026-07-15T13:00:00Z";

    struct EventKeyGolden {
        name: &'static str,
        event_key: ProcessStdinEventKey,
        route_family: RouteFamily,
        expected_identity: Value,
        expected_event_id: &'static str,
    }

    struct EventKeyMutation {
        name: &'static str,
        golden_index: usize,
        field: &'static str,
        replacement: Value,
        expected_rejection: ExpectedRejection,
    }

    #[derive(Clone, Copy)]
    enum ExpectedRejection {
        Contract(&'static str),
        Json,
    }

    fn contract_message(error: CoreError) -> Result<String, CoreError> {
        match error {
            CoreError::Contract(message) => Ok(message),
            other => Err(other),
        }
    }

    fn golden_event_keys() -> Vec<EventKeyGolden> {
        vec![
            EventKeyGolden {
                name: "condition event",
                event_key: ProcessStdinEventKey::ConditionEvent {
                    condition_id: "changed".parse().expect("condition id"),
                    observation_seq: 7,
                },
                route_family: RouteFamily::OnCondition,
                expected_identity: json!({
                    "condition_id": "changed",
                    "observation_seq": 7,
                }),
                expected_event_id: "e5a065277023e78d8547b8f6b4f5b688fe844441d64e000f6ef0023ce2bc1bd1",
            },
            EventKeyGolden {
                name: "condition level",
                event_key: ProcessStdinEventKey::ConditionLevel {
                    condition_id: "low".parse().expect("condition id"),
                    entry_at: FIRST.to_owned(),
                },
                route_family: RouteFamily::OnCondition,
                expected_identity: json!({
                    "condition_id": "low",
                    "entry_at": FIRST,
                }),
                expected_event_id: "76729aeaf7c2b9c510acb9ec7d66f288388911ec3dd62ad6fd53df7402ea62aa",
            },
            EventKeyGolden {
                name: "initialized",
                event_key: ProcessStdinEventKey::Initialized {
                    contract_digest_sha256: CONTRACT_DIGEST.to_owned(),
                },
                route_family: RouteFamily::OnRun,
                expected_identity: json!({
                    "kind": "initialized",
                    "contract_digest_sha256": CONTRACT_DIGEST,
                }),
                expected_event_id: "a2b4994f06b8959d56e9b280dcef69989bf50b97c923f2d61cb069f1de3b578a",
            },
            EventKeyGolden {
                name: "reset",
                event_key: ProcessStdinEventKey::Reset {
                    contract_digest_sha256: CONTRACT_DIGEST.to_owned(),
                },
                route_family: RouteFamily::OnRun,
                expected_identity: json!({
                    "kind": "reset",
                    "contract_digest_sha256": CONTRACT_DIGEST,
                }),
                expected_event_id: "79321abe28b96f48d93796ca62ac548025a6d305a3063e96d3f9a202e1cd3af7",
            },
            EventKeyGolden {
                name: "arithmetic overflow",
                event_key: ProcessStdinEventKey::ArithmeticOverflow {
                    condition_id: "changed".parse().expect("condition id"),
                    observation_seq: 7,
                },
                route_family: RouteFamily::OnRun,
                expected_identity: json!({
                    "condition_id": "changed",
                    "kind": "arithmetic_overflow",
                    "observation_seq": 7,
                }),
                expected_event_id: "c47e14b2645cbc248cbf7076752bb3df5020dcbc803c44d0460e98878ba90b94",
            },
            EventKeyGolden {
                name: "zero reference",
                event_key: ProcessStdinEventKey::ZeroReference {
                    condition_id: "low".parse().expect("condition id"),
                    observation_seq: 7,
                },
                route_family: RouteFamily::OnRun,
                expected_identity: json!({
                    "condition_id": "low",
                    "kind": "zero_reference",
                    "observation_seq": 7,
                }),
                expected_event_id: "0168719a74209a7bfd8feedec32a7f12cd398f488d5f592158c2716fdc138dce",
            },
            EventKeyGolden {
                name: "source suspect escalated",
                event_key: ProcessStdinEventKey::SourceSuspectEscalated {
                    reason_class: SourceSuspectReason::FetchFailed,
                    episode: FIRST.to_owned(),
                },
                route_family: RouteFamily::OnRun,
                expected_identity: json!({
                    "reason_class": "fetch_failed",
                    "episode": FIRST,
                }),
                expected_event_id: "7207d3b9286e32289e3f5b96697219660a028121b01db3baaef2a795f17e973d",
            },
            EventKeyGolden {
                name: "permanent contract error",
                event_key: ProcessStdinEventKey::PermanentContractError {
                    contract_digest_sha256: CONTRACT_DIGEST.to_owned(),
                    error_code: PermanentErrorCode::InvalidJsonPointer,
                    first_seen_at: FIRST.to_owned(),
                },
                route_family: RouteFamily::OnRun,
                expected_identity: json!({
                    "contract_digest_sha256": CONTRACT_DIGEST,
                    "error_code": "invalid_json_pointer",
                    "first_seen_at": FIRST,
                }),
                expected_event_id: "b0abfcb9a21fe6f12f1dbe3d7807387592f53e0f3953154da594ab133a3e50a9",
            },
        ]
    }

    fn payload_for_event_key(
        event_key: ProcessStdinEventKey,
        target_id: &TargetId,
    ) -> (RouteId, RouteFamily, ProcessStdinPayload) {
        let (
            route_id,
            route_family,
            summary,
            condition_id,
            observation_seq,
            canonical_value,
            reason_class,
            error_code,
            episode_started_at,
        ) = match &event_key {
            ProcessStdinEventKey::ConditionEvent {
                condition_id,
                observation_seq,
            } => (
                "condition",
                RouteFamily::OnCondition,
                format!(
                    "Demo[condition={condition_id}]: satisfied at observation {observation_seq} (value={observation_seq})"
                ),
                Some(condition_id.clone()),
                Some(*observation_seq),
                Some(observation_seq.to_string()),
                None,
                None,
                None,
            ),
            ProcessStdinEventKey::ConditionLevel { condition_id, .. } => (
                "condition",
                RouteFamily::OnCondition,
                format!("Demo[condition={condition_id}]: satisfied at observation 7 (value=7)"),
                Some(condition_id.clone()),
                Some(7),
                Some("7".to_owned()),
                None,
                None,
                None,
            ),
            ProcessStdinEventKey::Initialized { .. } => (
                "run",
                RouteFamily::OnRun,
                "Demo: initialized".to_owned(),
                None,
                Some(7),
                Some("7".to_owned()),
                None,
                None,
                None,
            ),
            ProcessStdinEventKey::Reset { .. } => (
                "run",
                RouteFamily::OnRun,
                "Demo: reset".to_owned(),
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            ProcessStdinEventKey::ArithmeticOverflow {
                condition_id,
                observation_seq,
            } => (
                "run",
                RouteFamily::OnRun,
                format!(
                    "Demo[condition={condition_id}]: arithmetic_overflow at observation {observation_seq}"
                ),
                Some(condition_id.clone()),
                Some(*observation_seq),
                Some(observation_seq.to_string()),
                None,
                None,
                None,
            ),
            ProcessStdinEventKey::ZeroReference {
                condition_id,
                observation_seq,
            } => (
                "run",
                RouteFamily::OnRun,
                format!(
                    "Demo[condition={condition_id}]: zero_reference at observation {observation_seq}"
                ),
                Some(condition_id.clone()),
                Some(*observation_seq),
                Some(observation_seq.to_string()),
                None,
                None,
                None,
            ),
            ProcessStdinEventKey::SourceSuspectEscalated {
                reason_class,
                episode,
            } => (
                "run",
                RouteFamily::OnRun,
                format!("Demo: source health escalated ({})", reason_class.as_str()),
                None,
                None,
                None,
                Some(*reason_class),
                None,
                Some(episode.clone()),
            ),
            ProcessStdinEventKey::PermanentContractError {
                error_code,
                first_seen_at,
                ..
            } => (
                "run",
                RouteFamily::OnRun,
                format!("Demo: permanent contract error ({})", error_code.as_str()),
                None,
                None,
                None,
                None,
                Some(*error_code),
                Some(first_seen_at.clone()),
            ),
        };
        let route_id = RouteId::new(route_id).expect("route id");
        let payload = ProcessStdinPayload::new(
            &route_id,
            route_family,
            target_id,
            "Demo",
            event_key,
            &summary,
            condition_id,
            observation_seq,
            canonical_value,
            reason_class,
            error_code,
            episode_started_at,
        )
        .expect("valid payload");
        (route_id, route_family, payload)
    }

    #[test]
    fn process_stdin_event_keys_keep_the_complete_exact_identity_contract() {
        let target_id = TargetId::new("demo").expect("target id");

        for golden in golden_event_keys() {
            let identity = golden.event_key.identity_json();
            assert_eq!(
                identity, golden.expected_identity,
                "{} must preserve its exact identity-key shape",
                golden.name
            );
            assert_eq!(
                golden
                    .event_key
                    .event_id(&target_id, golden.route_family)
                    .expect("event id"),
                golden.expected_event_id,
                "{} must preserve its exact deterministic event id",
                golden.name
            );
        }
    }

    #[test]
    fn delivery_payload_guards_reject_every_incoherent_fact_before_drain() {
        let target_id = TargetId::new("demo").expect("target id");
        let reject_record =
            |payload: &ProcessStdinPayload, route_id: &RouteId, route_family: RouteFamily| {
                assert!(
                    payload
                        .validate_for_record(
                            &target_id,
                            CONTRACT_DIGEST,
                            payload.event_id(),
                            route_id,
                            route_family,
                        )
                        .is_err()
                );
            };

        for (kind, spelling) in [
            (
                ProcessStdinEventKind::ConditionSatisfied,
                "condition_satisfied",
            ),
            (ProcessStdinEventKind::Initialized, "initialized"),
            (ProcessStdinEventKind::Reset, "reset"),
            (
                ProcessStdinEventKind::ArithmeticOverflow,
                "arithmetic_overflow",
            ),
            (ProcessStdinEventKind::ZeroReference, "zero_reference"),
            (
                ProcessStdinEventKind::SourceSuspectEscalated,
                "source_suspect_escalated",
            ),
            (
                ProcessStdinEventKind::PermanentContractError,
                "permanent_contract_error",
            ),
        ] {
            assert_eq!(kind.as_str(), spelling);
        }

        assert!(
            ProcessStdinEventKey::ConditionEvent {
                condition_id: "changed".parse().expect("condition id"),
                observation_seq: 0,
            }
            .validate(None)
            .is_err()
        );
        assert!(
            ProcessStdinEventKey::ConditionLevel {
                condition_id: "changed".parse().expect("condition id"),
                entry_at: "2026-07-15T00:00:00+00:00".to_owned(),
            }
            .validate(None)
            .is_err()
        );
        assert!(
            ProcessStdinEventKey::Initialized {
                contract_digest_sha256: "not-a-digest".to_owned(),
            }
            .validate(None)
            .is_err()
        );

        let condition_level = golden_event_keys().remove(1).event_key;
        let (_route, _family, mut payload) = payload_for_event_key(condition_level, &target_id);
        payload.condition_id = None;
        assert!(payload.event_key.validate_payload_facts(&payload).is_err());
        payload.condition_id = Some("other".parse().expect("condition id"));
        assert!(payload.event_key.validate_payload_facts(&payload).is_err());

        let source = golden_event_keys().remove(6).event_key;
        let (_route, _family, mut payload) = payload_for_event_key(source, &target_id);
        payload.reason_class = Some(SourceSuspectReason::JsonMalformed);
        assert!(payload.event_key.validate_payload_facts(&payload).is_err());
        payload.reason_class = Some(SourceSuspectReason::FetchFailed);
        payload.episode_started_at = Some(SECOND.to_owned());
        assert!(payload.event_key.validate_payload_facts(&payload).is_err());

        let permanent = golden_event_keys().remove(7).event_key;
        let (_route, _family, mut payload) = payload_for_event_key(permanent, &target_id);
        payload.error_code = Some(PermanentErrorCode::HtmlcutPlanInvalid);
        assert!(payload.event_key.validate_payload_facts(&payload).is_err());
        payload.error_code = Some(PermanentErrorCode::InvalidJsonPointer);
        payload.episode_started_at = Some(SECOND.to_owned());
        assert!(payload.event_key.validate_payload_facts(&payload).is_err());

        let condition = golden_event_keys().remove(0).event_key;
        let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
        payload.schema_version = 99;
        reject_record(&payload, &route, family);

        let condition = golden_event_keys().remove(0).event_key;
        let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
        payload.schema_name = "unexpected.schema".to_owned();
        reject_record(&payload, &route, family);

        let condition = golden_event_keys().remove(0).event_key;
        let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
        payload.target_id = TargetId::new("other").expect("target id");
        reject_record(&payload, &route, family);

        let condition = golden_event_keys().remove(0).event_key;
        let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
        payload.event_id = "other-event".to_owned();
        reject_record(&payload, &route, family);

        let condition = golden_event_keys().remove(0).event_key;
        let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
        payload.route_id = RouteId::new("other").expect("route id");
        reject_record(&payload, &route, family);

        let condition = golden_event_keys().remove(0).event_key;
        let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
        payload.route_family = RouteFamily::OnRun;
        reject_record(&payload, &route, family);

        let condition = golden_event_keys().remove(0).event_key;
        let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
        payload.event_kind = ProcessStdinEventKind::Reset;
        reject_record(&payload, &route, family);

        let condition = golden_event_keys().remove(0).event_key;
        let (route, family, mut payload) = payload_for_event_key(condition, &target_id);
        payload.reason_class = Some(SourceSuspectReason::FetchFailed);
        reject_record(&payload, &route, family);

        let initialized = golden_event_keys().remove(2).event_key;
        let (route, family, mut payload) = payload_for_event_key(initialized, &target_id);
        payload.condition_id = Some("changed".parse().expect("condition id"));
        reject_record(&payload, &route, family);

        let initialized = golden_event_keys().remove(2).event_key;
        let (route, family, mut payload) = payload_for_event_key(initialized, &target_id);
        payload.reason_class = Some(SourceSuspectReason::FetchFailed);
        reject_record(&payload, &route, family);

        let reset = golden_event_keys().remove(3).event_key;
        let (route, family, mut payload) = payload_for_event_key(reset, &target_id);
        payload.canonical_value = Some("7".to_owned());
        reject_record(&payload, &route, family);

        let reset = golden_event_keys().remove(3).event_key;
        let (route, family, mut payload) = payload_for_event_key(reset, &target_id);
        payload.episode_started_at = Some(FIRST.to_owned());
        reject_record(&payload, &route, family);

        let arithmetic = golden_event_keys().remove(4).event_key;
        let (route, family, mut payload) = payload_for_event_key(arithmetic, &target_id);
        payload.episode_started_at = Some(FIRST.to_owned());
        reject_record(&payload, &route, family);

        let source = golden_event_keys().remove(6).event_key;
        let (route, family, mut payload) = payload_for_event_key(source, &target_id);
        payload.error_code = Some(PermanentErrorCode::InvalidJsonPointer);
        reject_record(&payload, &route, family);

        let source = golden_event_keys().remove(6).event_key;
        let (route, family, mut payload) = payload_for_event_key(source, &target_id);
        payload.condition_id = Some("changed".parse().expect("condition id"));
        reject_record(&payload, &route, family);

        let source = golden_event_keys().remove(6).event_key;
        let (route, family, mut payload) = payload_for_event_key(source, &target_id);
        payload.reason_class = None;
        reject_record(&payload, &route, family);

        let source = golden_event_keys().remove(6).event_key;
        let (route, family, mut payload) = payload_for_event_key(source, &target_id);
        payload.episode_started_at = None;
        reject_record(&payload, &route, family);

        let permanent = golden_event_keys().remove(7).event_key;
        let (route, family, mut payload) = payload_for_event_key(permanent, &target_id);
        payload.reason_class = Some(SourceSuspectReason::FetchFailed);
        reject_record(&payload, &route, family);

        let permanent = golden_event_keys().remove(7).event_key;
        let (route, family, mut payload) = payload_for_event_key(permanent, &target_id);
        payload.condition_id = Some("changed".parse().expect("condition id"));
        reject_record(&payload, &route, family);

        let permanent = golden_event_keys().remove(7).event_key;
        let (route, family, mut payload) = payload_for_event_key(permanent, &target_id);
        payload.error_code = None;
        reject_record(&payload, &route, family);

        let permanent = golden_event_keys().remove(7).event_key;
        let (route, family, mut payload) = payload_for_event_key(permanent, &target_id);
        payload.episode_started_at = None;
        reject_record(&payload, &route, family);

        let condition = golden_event_keys().remove(0).event_key;
        let (_route, _family, payload) = payload_for_event_key(condition, &target_id);
        assert!(require_route_family(&payload, RouteFamily::OnRun).is_err());
        let mut no_observation = payload.clone();
        no_observation.observation_seq = None;
        assert!(require_observation_fact(&no_observation, "condition").is_err());
        let mut no_canonical_value = payload.clone();
        no_canonical_value.canonical_value = Some(String::new());
        assert!(require_observation_fact(&no_canonical_value, "condition").is_err());
        let mut no_condition = payload.clone();
        no_condition.condition_id = None;
        assert!(require_condition_fact(&no_condition, "condition").is_err());
        assert!(require_canonical_timestamp("timestamp", "2026-07-15T00:00:00+00:00").is_err());
        assert!(require_canonical_timestamp("timestamp", "2026-07-15T02:00:00+02:00").is_err());
        assert!(require_canonical_timestamp("timestamp", "2026-07-15T00:00:00.000Z").is_err());
        assert!(require_contract_digest("not-a-digest", None).is_err());
        assert!(require_contract_digest(&"g".repeat(64), None).is_err());
        let mut mismatched = payload.clone();
        mismatched.observation_seq = Some(8);
        assert!(
            mismatched
                .event_key
                .validate_payload_facts(&mismatched)
                .is_err()
        );
        let mut different_condition = payload.clone();
        different_condition.condition_id = Some("other".parse().expect("condition id"));
        assert!(
            different_condition
                .event_key
                .validate_payload_facts(&different_condition)
                .is_err()
        );

        let mut missing_condition = payload.clone();
        missing_condition.condition_id = None;
        assert!(missing_condition.expected_summary().is_err());
        let mut missing_sequence = payload.clone();
        missing_sequence.observation_seq = None;
        assert!(missing_sequence.expected_summary().is_err());
        let mut missing_value = payload;
        missing_value.canonical_value = None;
        assert!(missing_value.expected_summary().is_err());

        let reset = golden_event_keys().remove(3).event_key;
        let (route, family, mut payload) = payload_for_event_key(reset, &target_id);
        payload.summary = "incorrect summary".to_owned();
        reject_record(&payload, &route, family);
    }

    #[test]
    fn persisted_payload_rejects_mutation_of_every_event_key_fact() {
        let target_id = TargetId::new("demo").expect("target id");
        let goldens = golden_event_keys();
        let mutations = vec![
            EventKeyMutation {
                name: "condition event condition id",
                golden_index: 0,
                field: "condition_id",
                replacement: json!("recovered"),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox event_id must be derived from its persisted event_key",
                ),
            },
            EventKeyMutation {
                name: "condition event observation sequence",
                golden_index: 0,
                field: "observation_seq",
                replacement: json!(8),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox event_id must be derived from its persisted event_key",
                ),
            },
            EventKeyMutation {
                name: "condition level condition id",
                golden_index: 1,
                field: "condition_id",
                replacement: json!("high"),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox event_id must be derived from its persisted event_key",
                ),
            },
            EventKeyMutation {
                name: "condition level entry instant",
                golden_index: 1,
                field: "entry_at",
                replacement: json!(SECOND),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox event_id must be derived from its persisted event_key",
                ),
            },
            EventKeyMutation {
                name: "initialized contract digest",
                golden_index: 2,
                field: "contract_digest_sha256",
                replacement: json!(
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                ),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox immutable_payload event_key contract_digest_sha256 must match state",
                ),
            },
            EventKeyMutation {
                name: "reset contract digest",
                golden_index: 3,
                field: "contract_digest_sha256",
                replacement: json!(
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                ),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox immutable_payload event_key contract_digest_sha256 must match state",
                ),
            },
            EventKeyMutation {
                name: "arithmetic-overflow condition id",
                golden_index: 4,
                field: "condition_id",
                replacement: json!("recovered"),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox event_id must be derived from its persisted event_key",
                ),
            },
            EventKeyMutation {
                name: "arithmetic-overflow observation sequence",
                golden_index: 4,
                field: "observation_seq",
                replacement: json!(8),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox event_id must be derived from its persisted event_key",
                ),
            },
            EventKeyMutation {
                name: "zero-reference condition id",
                golden_index: 5,
                field: "condition_id",
                replacement: json!("high"),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox event_id must be derived from its persisted event_key",
                ),
            },
            EventKeyMutation {
                name: "zero-reference observation sequence",
                golden_index: 5,
                field: "observation_seq",
                replacement: json!(8),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox event_id must be derived from its persisted event_key",
                ),
            },
            EventKeyMutation {
                name: "source-suspect reason class",
                golden_index: 6,
                field: "reason_class",
                replacement: json!("json_malformed"),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox event_id must be derived from its persisted event_key",
                ),
            },
            EventKeyMutation {
                name: "source-suspect episode",
                golden_index: 6,
                field: "episode",
                replacement: json!(SECOND),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox event_id must be derived from its persisted event_key",
                ),
            },
            EventKeyMutation {
                name: "permanent-error contract digest",
                golden_index: 7,
                field: "contract_digest_sha256",
                replacement: json!(
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                ),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox immutable_payload event_key contract_digest_sha256 must match state",
                ),
            },
            EventKeyMutation {
                name: "permanent-error code",
                golden_index: 7,
                field: "error_code",
                replacement: json!("future_contract_error"),
                expected_rejection: ExpectedRejection::Json,
            },
            EventKeyMutation {
                name: "permanent-error first-seen instant",
                golden_index: 7,
                field: "first_seen_at",
                replacement: json!(SECOND),
                expected_rejection: ExpectedRejection::Contract(
                    "outbox event_id must be derived from its persisted event_key",
                ),
            },
        ];

        for mutation in mutations {
            let golden = &goldens[mutation.golden_index];
            let (route_id, route_family, payload) =
                payload_for_event_key(golden.event_key.clone(), &target_id);
            let event_id = payload.event_id().to_owned();
            let payload_bytes = payload.immutable_bytes().expect("canonical payload");
            validate_process_stdin_payload_bytes(
                &payload_bytes,
                &target_id,
                CONTRACT_DIGEST,
                &event_id,
                &route_id,
                route_family,
            )
            .expect("baseline payload");

            let mut wire: Value = serde_json::from_slice(&payload_bytes).expect("payload JSON");
            wire["event_key"][mutation.field] = mutation.replacement;
            let mutated_bytes = crate::stable_json::stable_json(&wire)
                .expect("canonical mutated payload")
                .into_bytes();
            let result = validate_process_stdin_payload_bytes(
                &mutated_bytes,
                &target_id,
                CONTRACT_DIGEST,
                &event_id,
                &route_id,
                route_family,
            );

            let error = result.expect_err("every event-key mutation must be rejected");
            match mutation.expected_rejection {
                ExpectedRejection::Contract(expected) => assert_eq!(
                    contract_message(error).expect("mutation must be a contract rejection"),
                    expected,
                    "{}",
                    mutation.name
                ),
                ExpectedRejection::Json => assert!(matches!(
                    contract_message(error).expect_err("mutation must be a JSON rejection"),
                    CoreError::Json(_)
                )),
            }
        }
    }
}

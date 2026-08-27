//! Immutable route snapshots and mutable retry bookkeeping for graph-owned outboxes.

use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{ConditionId, CoreError};

use super::{
    DeliveryPolicy, EventEnvelope, EventKind, GraphDeliveryAdapter, GraphRoute, GraphRouteFamily,
    GraphRouteId,
};

/// Canonical pending delivery-record schema name.
pub const DELIVERY_RECORD_SCHEMA_NAME: &str = "ffhn.delivery_record";
/// Canonical pending delivery-record schema version.
pub const DELIVERY_RECORD_SCHEMA_VERSION: u32 = 1;
/// Canonical terminal dead-letter schema name.
pub const DEAD_LETTER_SCHEMA_NAME: &str = "ffhn.dead_letter";
/// Canonical terminal dead-letter schema version.
pub const DEAD_LETTER_SCHEMA_VERSION: u32 = 1;

/// Immutable envelope and delivery snapshots plus append-only failure bookkeeping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryRecord {
    schema_name: String,
    schema_version: u32,
    envelope: EventEnvelope,
    route_id: GraphRouteId,
    route_family: GraphRouteFamily,
    adapter: GraphDeliveryAdapter,
    delivery_policy: DeliveryPolicy,
    attempts: Vec<DeliveryAttempt>,
    next_retry_at_utc: String,
}

/// Terminal durable record preserving the immutable envelope, snapshots, and every attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeadLetter {
    schema_name: String,
    schema_version: u32,
    record: DeliveryRecord,
    terminal_attempt: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeadLetterWire {
    schema_name: String,
    schema_version: u32,
    record: DeliveryRecordWire,
    terminal_attempt: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliveryRecordWire {
    schema_name: String,
    schema_version: u32,
    envelope: EventEnvelope,
    route_id: GraphRouteId,
    route_family: GraphRouteFamily,
    adapter: GraphDeliveryAdapter,
    delivery_policy: DeliveryPolicy,
    #[serde(default)]
    attempts: Vec<DeliveryAttempt>,
    next_retry_at_utc: String,
}

/// One observed failed delivery attempt appended to its immutable record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryAttempt {
    attempt: u32,
    attempted_at_utc: String,
    failure: DeliveryAttemptFailure,
}

/// Closed safe failure evidence for one delivery attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeliveryAttemptFailure {
    /// The route's environment-backed secret was unavailable.
    SecretUnavailable {
        /// Missing environment variable name, never its value.
        env: String,
    },
    /// Process creation, I/O, timeout, or exit was not successful.
    Process {
        /// Bounded human-safe process failure message.
        message: String,
    },
    /// HTTPS webhook transport or response status was not successful.
    HttpWebhook {
        /// Bounded human-safe webhook failure message.
        message: String,
        /// HTTP status when a response was received.
        status: Option<u16>,
    },
}

/// Durable fact for a new record refused because its emitter queue was already full.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutboxOverflowFact {
    event_id: String,
    event_kind: EventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    condition_id: Option<ConditionId>,
    route_id: GraphRouteId,
    route_family: GraphRouteFamily,
}

impl OutboxOverflowFact {
    /// Returns the route-independent event identity whose fan-out was refused.
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    /// Returns the typed event family whose record could not be admitted.
    pub const fn event_kind(&self) -> EventKind {
        self.event_kind
    }

    /// Returns the condition identity for a condition-scoped overflow.
    pub const fn condition_id(&self) -> Option<&ConditionId> {
        self.condition_id.as_ref()
    }

    /// Returns the configured route whose fan-out record could not be admitted.
    pub const fn route_id(&self) -> &GraphRouteId {
        &self.route_id
    }

    /// Validates the immutable event/route identity of one rejected admission.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.event_id.len() == 64
            && self
                .event_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            && self.route_family == self.event_kind.route_family()
            && (self.condition_id.is_some()
                == matches!(
                    self.event_kind,
                    EventKind::ConditionSatisfied | EventKind::ConditionEvaluationIssue
                ))
        {
            Ok(())
        } else {
            Err(CoreError::contract(
                "outbox overflow event, condition, and route-family facts disagree",
            ))
        }
    }
}

/// Deterministic admission result for one emitter generation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutboxAdmission {
    records: Vec<DeliveryRecord>,
    overflow: Vec<OutboxOverflowFact>,
}

impl<'de> Deserialize<'de> for DeliveryRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DeliveryRecordWire::deserialize(deserializer)?;
        Self::from_wire(wire, false).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for DeadLetter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DeadLetterWire::deserialize(deserializer)?;
        let letter = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            record: DeliveryRecord::from_wire(wire.record, true)
                .map_err(serde::de::Error::custom)?,
            terminal_attempt: wire.terminal_attempt,
        };
        letter.validate().map_err(serde::de::Error::custom)?;
        Ok(letter)
    }
}

impl DeliveryRecord {
    /// Returns the event identity that remains stable through every retry and route fan-out.
    pub fn event_id(&self) -> &str {
        self.envelope.event_id()
    }

    /// Returns the route-local record key's second component.
    pub fn route_id(&self) -> &GraphRouteId {
        &self.route_id
    }

    /// Returns the immutable route family snapshot.
    pub const fn route_family(&self) -> GraphRouteFamily {
        self.route_family
    }

    /// Returns the immutable event envelope sent to every adapter attempt.
    pub const fn envelope(&self) -> &EventEnvelope {
        &self.envelope
    }

    /// Returns the immutable copied adapter configuration.
    pub const fn adapter(&self) -> &GraphDeliveryAdapter {
        &self.adapter
    }

    /// Returns the immutable copied retry policy.
    pub const fn delivery_policy(&self) -> &DeliveryPolicy {
        &self.delivery_policy
    }

    /// Returns the append-only observed failure attempts.
    pub fn attempts(&self) -> &[DeliveryAttempt] {
        &self.attempts
    }

    /// Returns the earliest UTC instant at which this record may be attempted again.
    pub fn next_retry_at_utc(&self) -> &str {
        &self.next_retry_at_utc
    }

    /// Returns the validated deterministic storage filename for this `(event_id, route_id)` key.
    pub fn storage_file_name(&self) -> String {
        format!("{}--{}.json", self.event_id(), self.route_id.as_str())
    }

    pub(crate) fn is_single_attempt_successor_of(&self, prior: &Self) -> bool {
        self.envelope == prior.envelope
            && self.route_id == prior.route_id
            && self.route_family == prior.route_family
            && self.adapter == prior.adapter
            && self.delivery_policy == prior.delivery_policy
            && self.attempts.len() == prior.attempts.len() + 1
            && self.attempts.starts_with(&prior.attempts)
    }

    /// Appends one failed attempt and advances only mutable bookkeeping.
    pub fn record_failure(
        &mut self,
        attempted_at_utc: String,
        failure: DeliveryAttemptFailure,
        next_retry_at_utc: String,
    ) -> Result<(), CoreError> {
        self.validate()?;
        require_timestamp("delivery attempt time", &attempted_at_utc)?;
        require_timestamp("delivery next retry time", &next_retry_at_utc)?;
        if OffsetDateTime::parse(&next_retry_at_utc, &Rfc3339)?
            < OffsetDateTime::parse(&attempted_at_utc, &Rfc3339)?
        {
            return Err(CoreError::contract(
                "delivery next retry time must not precede its failed attempt",
            ));
        }
        let attempt = u32::try_from(self.attempts.len())
            .map_err(|_| CoreError::contract("delivery attempt count overflowed"))?
            .checked_add(1)
            .ok_or_else(|| CoreError::contract("delivery attempt count overflowed"))?;
        failure.validate()?;
        self.attempts.push(DeliveryAttempt {
            attempt,
            attempted_at_utc,
            failure,
        });
        self.next_retry_at_utc = next_retry_at_utc;
        self.validate_common()
    }

    /// Moves this record to its terminal dead-letter document exactly at its snapshot attempt cap.
    pub fn into_dead_letter(self) -> Result<DeadLetter, CoreError> {
        self.validate_common()?;
        let terminal_attempt = u32::try_from(self.attempts.len())
            .map_err(|_| CoreError::contract("delivery attempt count overflowed"))?;
        if terminal_attempt != self.delivery_policy.max_attempts() {
            return Err(CoreError::contract(
                "delivery record can dead-letter only at its immutable maximum attempt count",
            ));
        }
        let letter = DeadLetter {
            schema_name: DEAD_LETTER_SCHEMA_NAME.to_owned(),
            schema_version: DEAD_LETTER_SCHEMA_VERSION,
            record: self,
            terminal_attempt,
        };
        letter.validate()?;
        Ok(letter)
    }

    /// Validates immutable snapshot coherence and append-only bookkeeping sequence.
    pub fn validate(&self) -> Result<(), CoreError> {
        self.validate_common()?;
        if self.attempts.len()
            >= usize::try_from(self.delivery_policy.max_attempts()).unwrap_or(usize::MAX)
        {
            return Err(CoreError::contract(
                "pending delivery record has reached its terminal attempt bound",
            ));
        }
        Ok(())
    }

    fn validate_common(&self) -> Result<(), CoreError> {
        if self.schema_name != DELIVERY_RECORD_SCHEMA_NAME
            || self.schema_version != DELIVERY_RECORD_SCHEMA_VERSION
        {
            return Err(CoreError::contract(
                "delivery record is not a current FFHN delivery record",
            ));
        }
        self.envelope.validate()?;
        self.adapter.validate()?;
        self.delivery_policy.validate()?;
        if self.route_family != self.envelope.event_kind().route_family() {
            return Err(CoreError::contract(
                "delivery record route family does not accept its event kind",
            ));
        }
        require_timestamp("delivery next retry time", &self.next_retry_at_utc)?;
        if self.attempts.len()
            > usize::try_from(self.delivery_policy.max_attempts()).unwrap_or(usize::MAX)
        {
            return Err(CoreError::contract(
                "delivery attempt history exceeds its immutable terminal bound",
            ));
        }
        let mut previous = None;
        for (index, attempt) in self.attempts.iter().enumerate() {
            let expected = u32::try_from(index)
                .map_err(|_| CoreError::contract("delivery attempt count overflowed"))?
                .checked_add(1)
                .ok_or_else(|| CoreError::contract("delivery attempt count overflowed"))?;
            attempt.validate(expected)?;
            let attempted = OffsetDateTime::parse(&attempt.attempted_at_utc, &Rfc3339)?;
            if previous.is_some_and(|previous| attempted < previous) {
                return Err(CoreError::contract(
                    "delivery attempt timestamps must be monotonic",
                ));
            }
            previous = Some(attempted);
        }
        if previous.is_some_and(|attempted| {
            OffsetDateTime::parse(&self.next_retry_at_utc, &Rfc3339)
                .is_ok_and(|next| next < attempted)
        }) {
            return Err(CoreError::contract(
                "delivery next retry time must not precede its latest attempt",
            ));
        }
        Ok(())
    }

    fn from_wire(wire: DeliveryRecordWire, terminal: bool) -> Result<Self, CoreError> {
        let record = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            envelope: wire.envelope,
            route_id: wire.route_id,
            route_family: wire.route_family,
            adapter: wire.adapter,
            delivery_policy: wire.delivery_policy,
            attempts: wire.attempts,
            next_retry_at_utc: wire.next_retry_at_utc,
        };
        if terminal {
            record.validate_common()?;
        } else {
            record.validate()?;
        }
        Ok(record)
    }

    fn new(
        envelope: EventEnvelope,
        route: &GraphRoute,
        delivery_policy: &DeliveryPolicy,
        commit_time_utc: &str,
    ) -> Result<Self, CoreError> {
        require_timestamp("delivery admission time", commit_time_utc)?;
        let record = Self {
            schema_name: DELIVERY_RECORD_SCHEMA_NAME.to_owned(),
            schema_version: DELIVERY_RECORD_SCHEMA_VERSION,
            envelope,
            route_id: route.route_id().clone(),
            route_family: route.route_family(),
            adapter: route.adapter().clone(),
            delivery_policy: delivery_policy.clone(),
            attempts: Vec::new(),
            next_retry_at_utc: commit_time_utc.to_owned(),
        };
        if record.route_family != record.envelope.event_kind().route_family() {
            return Err(CoreError::contract(
                "delivery record route family does not accept its event kind",
            ));
        }
        record.validate()?;
        Ok(record)
    }
}

impl DeadLetter {
    /// Returns the immutable original record and its complete attempt history.
    pub const fn record(&self) -> &DeliveryRecord {
        &self.record
    }

    /// Validates terminality against the record's immutable policy snapshot.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_name != DEAD_LETTER_SCHEMA_NAME
            || self.schema_version != DEAD_LETTER_SCHEMA_VERSION
        {
            return Err(CoreError::contract(
                "dead letter is not a current FFHN dead-letter document",
            ));
        }
        self.record.validate_common()?;
        let attempts = u32::try_from(self.record.attempts.len())
            .map_err(|_| CoreError::contract("delivery attempt count overflowed"))?;
        if attempts != self.terminal_attempt
            || attempts != self.record.delivery_policy.max_attempts()
        {
            return Err(CoreError::contract(
                "dead letter terminal attempt does not match its immutable delivery policy",
            ));
        }
        Ok(())
    }
}

impl DeliveryAttempt {
    fn validate(&self, expected_attempt: u32) -> Result<(), CoreError> {
        if self.attempt != expected_attempt {
            return Err(CoreError::contract(
                "delivery attempts must be an append-only contiguous sequence",
            ));
        }
        require_timestamp("delivery attempt time", &self.attempted_at_utc)?;
        self.failure.validate()
    }
}

impl DeliveryAttemptFailure {
    fn validate(&self) -> Result<(), CoreError> {
        match self {
            Self::SecretUnavailable { env } if !env.trim().is_empty() => Ok(()),
            Self::Process { message } if valid_message(message) => Ok(()),
            Self::HttpWebhook { message, status }
                if valid_message(message)
                    && status.is_none_or(|status| !(200..300).contains(&status)) =>
            {
                Ok(())
            }
            _ => Err(CoreError::contract(
                "delivery attempt failure evidence is incomplete or invalid",
            )),
        }
    }
}

impl OutboxAdmission {
    /// Admits route fan-out in caller-supplied deterministic event and route declaration order.
    /// Existing `(event_id, route_id)` records are skipped; no event is re-ordered by hash.
    pub fn admit(
        existing: &[DeliveryRecord],
        envelopes: impl IntoIterator<Item = EventEnvelope>,
        routes: &[GraphRoute],
        delivery_policy: Option<&DeliveryPolicy>,
        commit_time_utc: &str,
    ) -> Result<Self, CoreError> {
        require_timestamp("delivery admission time", commit_time_utc)?;
        let Some(policy) = delivery_policy else {
            if routes.is_empty() {
                return Ok(Self::default());
            }
            return Err(CoreError::contract(
                "routes cannot admit records without a delivery policy",
            ));
        };
        policy.validate()?;
        for record in existing {
            record.validate()?;
        }
        let mut occupied = existing
            .iter()
            .map(|record| (record.event_id().to_owned(), record.route_id().clone()))
            .collect::<BTreeSet<_>>();
        let mut records = Vec::new();
        let mut overflow = Vec::new();
        for envelope in envelopes {
            envelope.validate()?;
            for route in routes {
                if route.route_family() != envelope.event_kind().route_family() {
                    continue;
                }
                let key = (envelope.event_id().to_owned(), route.route_id().clone());
                if !occupied.insert(key.clone()) {
                    continue;
                }
                if existing.len() + records.len() >= policy.max_pending() {
                    overflow.push(OutboxOverflowFact {
                        event_id: key.0,
                        event_kind: envelope.event_kind(),
                        condition_id: envelope.condition_id().cloned(),
                        route_id: key.1,
                        route_family: route.route_family(),
                    });
                    continue;
                }
                records.push(DeliveryRecord::new(
                    envelope.clone(),
                    route,
                    policy,
                    commit_time_utc,
                )?);
            }
        }
        Ok(Self { records, overflow })
    }

    /// Returns newly admitted immutable delivery records in admission order.
    pub fn records(&self) -> &[DeliveryRecord] {
        &self.records
    }

    /// Returns durable overflow facts in the same deterministic admission order.
    pub fn overflow(&self) -> &[OutboxOverflowFact] {
        &self.overflow
    }
}

fn require_timestamp(field: &str, value: &str) -> Result<(), CoreError> {
    crate::model::require_canonical_utc_rfc3339(field, value)
}

fn valid_message(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 2_048 && !value.contains(['\n', '\r'])
}

#[cfg(test)]
#[path = "outbox/tests.rs"]
mod tests;

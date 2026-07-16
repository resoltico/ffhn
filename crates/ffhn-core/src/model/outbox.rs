use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::CoreError;

use super::{RouteFamily, RouteId, TargetId, validate_process_stdin_payload_bytes};

/// One immutable delivery record staged before the state/outbox commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StagedOutboxRecord {
    pub(crate) event_id: String,
    pub(crate) route_id: RouteId,
    pub(crate) route_family: RouteFamily,
    pub(crate) immutable_payload: Vec<u8>,
}

/// One pending durable outbox record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PendingOutboxRecord {
    event_id: String,
    route_id: RouteId,
    route_family: RouteFamily,
    immutable_payload: Vec<u8>,
    attempt_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
    next_retry_at: String,
}

/// Overflow evidence for a newly staged record intentionally not admitted to a full queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutboxOverflow {
    event_id: String,
    route_id: String,
}

impl OutboxOverflow {
    pub(crate) fn new(event_id: String, route_id: RouteId) -> Self {
        Self {
            event_id,
            route_id: route_id.into(),
        }
    }

    /// Returns the event identity that could not enter the full queue.
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    /// Returns the target-local route identity that could not enter the full queue.
    pub fn route_id(&self) -> &str {
        &self.route_id
    }
}

/// The pending-only outbox stored within one target state document.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Outbox(Vec<PendingOutboxRecord>);

impl Outbox {
    pub(crate) fn validate(
        &self,
        target_id: &TargetId,
        contract_digest_sha256: &str,
    ) -> Result<(), CoreError> {
        let mut previous: Option<(&str, &RouteId)> = None;
        for record in &self.0 {
            record.validate(target_id, contract_digest_sha256)?;
            if let Some((event_id, route_id)) = previous
                && (event_id, route_id) >= (record.event_id(), record.route_id())
            {
                return Err(CoreError::contract(
                    "outbox records must be strictly ordered by event_id and route_id",
                ));
            }
            previous = Some((record.event_id(), record.route_id()));
        }
        Ok(())
    }

    pub(crate) fn records(&self) -> &[PendingOutboxRecord] {
        &self.0
    }

    pub(crate) fn enqueue(
        &mut self,
        staged: Vec<StagedOutboxRecord>,
        max_pending: usize,
        commit_time: &str,
        target_id: &TargetId,
        contract_digest_sha256: &str,
    ) -> Result<Vec<OutboxOverflow>, CoreError> {
        require_timestamp("outbox commit timestamp", commit_time)?;
        let existing = self
            .0
            .iter()
            .map(|record| (record.event_id.clone(), record.route_id.clone()))
            .collect::<BTreeSet<_>>();
        let mut seen = existing;
        let mut staged = staged;
        staged.sort_unstable_by(|left, right| {
            (&left.event_id, &left.route_id).cmp(&(&right.event_id, &right.route_id))
        });

        let mut overflow = Vec::new();
        for record in staged {
            let key = (record.event_id.clone(), record.route_id.clone());
            if !seen.insert(key) {
                continue;
            }
            if self.0.len() >= max_pending {
                overflow.push(OutboxOverflow::new(record.event_id, record.route_id));
                continue;
            }
            self.0.push(PendingOutboxRecord {
                event_id: record.event_id,
                route_id: record.route_id,
                route_family: record.route_family,
                immutable_payload: record.immutable_payload,
                attempt_count: 0,
                last_error: None,
                next_retry_at: commit_time.to_owned(),
            });
        }
        self.0.sort_unstable_by(|left, right| {
            (left.event_id(), left.route_id()).cmp(&(right.event_id(), right.route_id()))
        });
        self.validate(target_id, contract_digest_sha256)?;
        Ok(overflow)
    }

    pub(crate) fn first_due(&self, now: &str) -> Result<Option<PendingOutboxRecord>, CoreError> {
        require_timestamp("outbox drain timestamp", now)?;
        let now = parse_timestamp(now)?;
        for record in &self.0 {
            if parse_timestamp(&record.next_retry_at)? <= now {
                return Ok(Some(record.clone()));
            }
        }
        Ok(None)
    }

    pub(crate) fn remove(&mut self, event_id: &str, route_id: &RouteId) -> Result<(), CoreError> {
        let index = self
            .0
            .iter()
            .position(|record| record.event_id == event_id && record.route_id == *route_id)
            .ok_or_else(|| {
                CoreError::internal("outbox record disappeared before delivery update")
            })?;
        self.0.remove(index);
        Ok(())
    }

    pub(crate) fn record_failure(
        &mut self,
        event_id: &str,
        route_id: &RouteId,
        error: String,
        next_retry_at: String,
    ) -> Result<u32, CoreError> {
        require_timestamp("outbox next-retry timestamp", &next_retry_at)?;
        let record = self
            .0
            .iter_mut()
            .find(|record| record.event_id == event_id && record.route_id == *route_id)
            .ok_or_else(|| {
                CoreError::internal("outbox record disappeared before delivery update")
            })?;
        record.attempt_count = record
            .attempt_count
            .checked_add(1)
            .ok_or_else(|| CoreError::contract("outbox attempt count overflow"))?;
        record.last_error = Some(bound_error(error));
        record.next_retry_at = next_retry_at;
        Ok(record.attempt_count)
    }
}

impl PendingOutboxRecord {
    fn validate(
        &self,
        target_id: &TargetId,
        contract_digest_sha256: &str,
    ) -> Result<(), CoreError> {
        if !is_sha256(&self.event_id) {
            return Err(CoreError::contract(
                "outbox event_id must be lowercase SHA-256",
            ));
        }
        if self.immutable_payload.is_empty() {
            return Err(CoreError::contract(
                "outbox immutable_payload must not be empty",
            ));
        }
        validate_process_stdin_payload_bytes(
            &self.immutable_payload,
            target_id,
            contract_digest_sha256,
            &self.event_id,
            &self.route_id,
            self.route_family,
        )?;
        if self
            .last_error
            .as_deref()
            .is_some_and(|error| error.is_empty() || error.len() > 4_096)
        {
            return Err(CoreError::contract(
                "outbox last_error must be non-empty and at most 4096 bytes",
            ));
        }
        if (self.attempt_count == 0) != self.last_error.is_none() {
            return Err(CoreError::contract(
                "outbox last_error must exist exactly when a delivery attempt has failed",
            ));
        }
        require_timestamp("outbox next_retry_at", &self.next_retry_at)
    }

    pub(crate) fn event_id(&self) -> &str {
        &self.event_id
    }

    pub(crate) fn route_id(&self) -> &RouteId {
        &self.route_id
    }

    pub(crate) const fn route_family(&self) -> RouteFamily {
        self.route_family
    }

    pub(crate) fn immutable_payload(&self) -> &[u8] {
        &self.immutable_payload
    }

    pub(crate) const fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    pub(crate) fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
}

fn bound_error(mut error: String) -> String {
    const LIMIT: usize = 4_096;
    if error.len() <= LIMIT {
        return error;
    }
    let mut cutoff = LIMIT - " [truncated]".len();
    while !error.is_char_boundary(cutoff) {
        cutoff -= 1;
    }
    error.truncate(cutoff);
    error.push_str(" [truncated]");
    error
}

fn is_sha256(value: &str) -> bool {
    if value.len() != 64 {
        return false;
    }
    for byte in value.bytes() {
        if !(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
            return false;
        }
    }
    true
}

fn parse_timestamp(value: &str) -> Result<OffsetDateTime, CoreError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|error| CoreError::contract(format!("outbox timestamp must be RFC 3339: {error}")))
}

fn require_timestamp(field: &str, value: &str) -> Result<(), CoreError> {
    let timestamp = parse_timestamp(value)?;
    let canonical = timestamp
        .format(&Rfc3339)
        .map_err(|error| CoreError::internal(format!("could not format timestamp: {error}")))?;
    if timestamp.offset() != time::UtcOffset::UTC || value != canonical {
        return Err(CoreError::contract(format!(
            "{field} must be canonical UTC RFC 3339"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProcessStdinEventKey, ProcessStdinPayload};

    const TIME: &str = "2026-07-15T12:00:00Z";

    fn route(value: &str) -> RouteId {
        RouteId::new(value).expect("route id")
    }

    fn target_id() -> TargetId {
        TargetId::new("demo").expect("target id")
    }

    fn payload(observation_seq: u64, route_id: &RouteId) -> ProcessStdinPayload {
        ProcessStdinPayload::new(
            route_id,
            RouteFamily::OnCondition,
            &target_id(),
            "Demo",
            ProcessStdinEventKey::ConditionEvent {
                condition_id: "changed".parse().expect("condition id"),
                observation_seq,
            },
            &format!(
                "Demo[condition=changed]: satisfied at observation {observation_seq} (value=1)"
            ),
            Some("changed".parse().expect("condition id")),
            Some(observation_seq),
            Some("1".to_owned()),
            None,
            None,
            None,
        )
        .expect("payload")
    }

    fn event_id(observation_seq: u64) -> String {
        let route_id = route("identity");
        payload(observation_seq, &route_id).event_id().to_owned()
    }

    fn staged(observation_seq: u64, route_id: &str) -> StagedOutboxRecord {
        let route_id = route(route_id);
        let payload = payload(observation_seq, &route_id);
        StagedOutboxRecord {
            event_id: payload.event_id().to_owned(),
            immutable_payload: payload.immutable_bytes().expect("payload bytes"),
            route_id,
            route_family: RouteFamily::OnCondition,
        }
    }

    fn pending(
        observation_seq: u64,
        route_id: &str,
        attempt_count: u32,
        last_error: Option<&str>,
        next_retry_at: &str,
    ) -> PendingOutboxRecord {
        let route_id = route(route_id);
        let payload = payload(observation_seq, &route_id);
        PendingOutboxRecord {
            immutable_payload: payload.immutable_bytes().expect("payload bytes"),
            event_id: payload.event_id().to_owned(),
            route_id,
            route_family: RouteFamily::OnCondition,
            attempt_count,
            last_error: last_error.map(str::to_owned),
            next_retry_at: next_retry_at.to_owned(),
        }
    }

    #[test]
    fn sha256_identity_requires_lowercase_hex() {
        assert!(is_sha256(&"a".repeat(64)));
        assert!(is_sha256(&"1".repeat(64)));
        assert!(!is_sha256(&"A".repeat(64)));
        assert!(!is_sha256("short"));
    }

    #[test]
    fn queue_orders_records_deduplicates_and_never_evicts_existing_records() {
        let mut outbox = Outbox::default();
        let overflow = outbox
            .enqueue(
                vec![staged(2, "beta"), staged(1, "zeta"), staged(1, "zeta")],
                2,
                TIME,
                &target_id(),
                &"a".repeat(64),
            )
            .expect("initial records");
        assert!(overflow.is_empty());
        assert!(
            outbox
                .records()
                .iter()
                .map(|record| (record.event_id(), record.route_id().as_str()))
                .collect::<Vec<_>>()
                .windows(2)
                .all(|window| window[0] < window[1])
        );
        assert_eq!(outbox.records()[0].route_family(), RouteFamily::OnCondition);
        assert!(
            std::str::from_utf8(outbox.records()[0].immutable_payload())
                .expect("payload text")
                .contains("\"event_kind\":\"condition_satisfied\"")
        );
        assert_eq!(outbox.records()[0].attempt_count(), 0);
        assert_eq!(outbox.records()[0].last_error(), None);

        let duplicate_and_overflow = outbox
            .enqueue(
                vec![staged(2, "beta"), staged(3, "gamma")],
                2,
                TIME,
                &target_id(),
                &"a".repeat(64),
            )
            .expect("duplicate and overflow");
        assert_eq!(duplicate_and_overflow.len(), 1);
        assert_eq!(duplicate_and_overflow[0].event_id(), event_id(3));
        assert_eq!(duplicate_and_overflow[0].route_id(), "gamma");
        assert_eq!(outbox.records().len(), 2);
    }

    #[test]
    fn queue_tracks_due_records_failures_and_removal() {
        let mut outbox = Outbox::default();
        outbox
            .enqueue(
                vec![staged(1, "alpha")],
                1,
                TIME,
                &target_id(),
                &"a".repeat(64),
            )
            .expect("queued record");
        assert!(
            outbox
                .first_due("2026-07-15T11:59:59Z")
                .expect("not due")
                .is_none()
        );
        let due = outbox.first_due(TIME).expect("due").expect("record");
        assert_eq!(due.event_id(), event_id(1));
        assert_eq!(due.route_id().as_str(), "alpha");

        let attempt = outbox
            .record_failure(
                &event_id(1),
                &route("alpha"),
                "temporary delivery failure".to_owned(),
                "2026-07-15T12:00:01Z".to_owned(),
            )
            .expect("record failure");
        assert_eq!(attempt, 1);
        assert_eq!(
            outbox.records()[0].last_error(),
            Some("temporary delivery failure")
        );
        assert!(
            outbox
                .first_due(TIME)
                .expect("not due after failure")
                .is_none()
        );
        outbox
            .remove(&event_id(1), &route("alpha"))
            .expect("remove record");
        assert!(outbox.records().is_empty());
        assert!(outbox.remove(&event_id(1), &route("alpha")).is_err());
        assert!(
            outbox
                .record_failure(
                    &event_id(1),
                    &route("alpha"),
                    "missing".to_owned(),
                    TIME.to_owned(),
                )
                .is_err()
        );
        outbox
            .enqueue(
                vec![staged(2, "beta")],
                1,
                TIME,
                &target_id(),
                &"a".repeat(64),
            )
            .expect("replacement record");
        assert!(outbox.remove(&event_id(3), &route("beta")).is_err());
        assert!(outbox.remove(&event_id(2), &route("alpha")).is_err());
        assert!(
            outbox
                .record_failure(
                    &event_id(3),
                    &route("beta"),
                    "missing event".to_owned(),
                    TIME.to_owned(),
                )
                .is_err()
        );
        assert!(
            outbox
                .record_failure(
                    &event_id(2),
                    &route("alpha"),
                    "missing route".to_owned(),
                    TIME.to_owned(),
                )
                .is_err()
        );
    }

    #[test]
    fn queue_rejects_invalid_persistence_and_timestamps() {
        assert!(
            Outbox::default()
                .enqueue(
                    vec![],
                    1,
                    "2026-07-15T12:00:00+01:00",
                    &target_id(),
                    &"a".repeat(64),
                )
                .is_err()
        );
        assert!(Outbox::default().first_due("not-a-time").is_err());

        for record in [
            PendingOutboxRecord {
                event_id: "not-a-digest".to_owned(),
                ..pending(1, "alpha", 0, None, TIME)
            },
            PendingOutboxRecord {
                event_id: "A".repeat(64),
                ..pending(1, "alpha", 0, None, TIME)
            },
            PendingOutboxRecord {
                immutable_payload: Vec::new(),
                ..pending(1, "alpha", 0, None, TIME)
            },
            pending(1, "alpha", 0, Some("unexpected"), TIME),
            pending(1, "alpha", 1, None, TIME),
            pending(1, "alpha", 1, Some(""), TIME),
            pending(1, "alpha", 1, Some(&"x".repeat(4_097)), TIME),
            pending(1, "alpha", 0, None, "2026-07-15T12:00:00+01:00"),
            pending(1, "alpha", 0, None, "2026-07-15T12:00:00.000Z"),
        ] {
            assert!(
                Outbox(vec![record])
                    .validate(&target_id(), &"a".repeat(64))
                    .is_err()
            );
        }
        assert!(
            Outbox(vec![
                pending(2, "alpha", 0, None, TIME),
                pending(1, "alpha", 0, None, TIME),
            ])
            .validate(&target_id(), &"a".repeat(64))
            .is_err()
        );
    }

    #[test]
    fn bounded_errors_preserve_utf8_and_record_failure_detects_overflow() {
        let error = bound_error("€".repeat(2_000));
        assert!(error.len() <= 4_096);
        assert!(error.ends_with(" [truncated]"));
        assert!(std::str::from_utf8(error.as_bytes()).is_ok());
        assert_eq!(bound_error("small".to_owned()), "small");

        let mut outbox = Outbox(vec![pending(1, "alpha", u32::MAX, Some("old"), TIME)]);
        assert!(
            outbox
                .record_failure(
                    &event_id(1),
                    &route("alpha"),
                    "again".to_owned(),
                    "2026-07-15T12:00:01Z".to_owned(),
                )
                .is_err()
        );
    }
}

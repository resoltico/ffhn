use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{CoreError, DiagnosticDetail};

use super::{
    ConditionId, DeliveryEventKind, RouteFamily, RouteId, TargetId,
    read_validated_process_stdin_payload_bytes,
};

mod validation;

use validation::{parse_timestamp, require_timestamp};

/// One immutable delivery record staged before the state/outbox commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StagedOutboxRecord {
    pub(crate) event_id: String,
    pub(crate) route_id: RouteId,
    pub(crate) route_family: RouteFamily,
    pub(crate) event_kind: DeliveryEventKind,
    pub(crate) condition_id: Option<ConditionId>,
    pub(crate) immutable_payload: Vec<u8>,
}

/// One pending durable outbox record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PendingOutboxRecord {
    event_id: String,
    route_id: RouteId,
    route_family: RouteFamily,
    event_kind: DeliveryEventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    condition_id: Option<ConditionId>,
    immutable_payload: Vec<u8>,
    attempt_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_error_detail: Option<DiagnosticDetail>,
    next_retry_at: String,
}

/// Overflow evidence for a newly staged record intentionally not admitted to a full queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutboxOverflow {
    event_id: String,
    route_id: String,
    event_kind: DeliveryEventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    condition_id: Option<String>,
}

impl OutboxOverflow {
    pub(crate) fn new(
        event_id: String,
        route_id: RouteId,
        event_kind: DeliveryEventKind,
        condition_id: Option<ConditionId>,
    ) -> Self {
        Self {
            event_id,
            route_id: route_id.into(),
            event_kind,
            condition_id: condition_id.map(Into::into),
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

    /// Returns the event kind that could not enter the full queue.
    pub const fn event_kind(&self) -> DeliveryEventKind {
        self.event_kind
    }

    /// Returns the condition identity for a condition-scoped overflow, when any.
    pub fn condition_id(&self) -> Option<&str> {
        self.condition_id.as_deref()
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
        let mut overflow = Vec::new();
        for record in staged {
            let key = (record.event_id.clone(), record.route_id.clone());
            if !seen.insert(key) {
                continue;
            }
            if self.0.len() >= max_pending {
                overflow.push(OutboxOverflow::new(
                    record.event_id,
                    record.route_id,
                    record.event_kind,
                    record.condition_id,
                ));
                continue;
            }
            self.0.push(PendingOutboxRecord {
                event_id: record.event_id,
                route_id: record.route_id,
                route_family: record.route_family,
                event_kind: record.event_kind,
                condition_id: record.condition_id,
                immutable_payload: record.immutable_payload,
                attempt_count: 0,
                last_error_detail: None,
                next_retry_at: commit_time.to_owned(),
            });
        }
        // Admission priority is the caller's declaration order.  Only the durable representation
        // is canonicalized below; sorting candidates before this loop would make a hash-derived
        // event id silently decide which new event survives a bounded queue.
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
        error_detail: DiagnosticDetail,
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
        record.last_error_detail = Some(error_detail.fit_durable_delivery_failure()?);
        record.next_retry_at = next_retry_at;
        Ok(record.attempt_count)
    }
}

impl PendingOutboxRecord {
    pub(crate) fn event_id(&self) -> &str {
        &self.event_id
    }

    pub(crate) fn route_id(&self) -> &RouteId {
        &self.route_id
    }

    pub(crate) const fn route_family(&self) -> RouteFamily {
        self.route_family
    }

    pub(crate) const fn event_kind(&self) -> DeliveryEventKind {
        self.event_kind
    }

    pub(crate) fn condition_id(&self) -> Option<&ConditionId> {
        self.condition_id.as_ref()
    }

    pub(crate) fn immutable_payload(&self) -> &[u8] {
        &self.immutable_payload
    }

    pub(crate) const fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    pub(crate) fn last_error_detail(&self) -> Option<&DiagnosticDetail> {
        self.last_error_detail.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProcessStdinEventKey, ProcessStdinPayload};
    use crate::{
        DeliveryFailurePrimary, DeliveryProcessAttempt, ExactByteCount, StderrCapture,
        StderrOutcome, TerminalOutcome, WriterOutcome,
    };

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
            event_kind: DeliveryEventKind::ConditionSatisfied,
            condition_id: Some("changed".parse().expect("condition id")),
        }
    }

    fn pending(
        observation_seq: u64,
        route_id: &str,
        attempt_count: u32,
        last_error_detail: Option<DiagnosticDetail>,
        next_retry_at: &str,
    ) -> PendingOutboxRecord {
        let route_id = route(route_id);
        let payload = payload(observation_seq, &route_id);
        PendingOutboxRecord {
            immutable_payload: payload.immutable_bytes().expect("payload bytes"),
            event_id: payload.event_id().to_owned(),
            route_id,
            route_family: RouteFamily::OnCondition,
            event_kind: DeliveryEventKind::ConditionSatisfied,
            condition_id: Some("changed".parse().expect("condition id")),
            attempt_count,
            last_error_detail,
            next_retry_at: next_retry_at.to_owned(),
        }
    }

    fn failed_detail() -> DiagnosticDetail {
        crate::model::delivery_failure_detail(DeliveryProcessAttempt::new(
            TerminalOutcome::Exited { exit_code: Some(1) },
            WriterOutcome::Completed,
            StderrOutcome::captured(
                StderrCapture::from_bytes(Vec::new(), ExactByteCount::zero())
                    .expect("empty capture"),
            ),
        ))
        .expect("valid delivery failure")
    }

    #[test]
    fn sha256_identity_requires_lowercase_hex() {
        assert!(validation::is_sha256(&"a".repeat(64)));
        assert!(validation::is_sha256(&"1".repeat(64)));
        assert!(!validation::is_sha256(&"A".repeat(64)));
        assert!(!validation::is_sha256("short"));
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
        assert_eq!(outbox.records()[0].last_error_detail(), None);

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
    fn queue_admits_new_records_in_staging_order_not_hash_order() {
        let mut outbox = Outbox::default();
        let first = staged(2, "alpha");
        let second = staged(1, "zeta");
        assert!(
            first.event_id > second.event_id,
            "the fixture must stage the higher hash first to prove admission is not hash-sorted"
        );

        let overflow = outbox
            .enqueue(
                vec![first.clone(), second.clone()],
                1,
                TIME,
                &target_id(),
                &"a".repeat(64),
            )
            .expect("bounded admission");

        assert_eq!(outbox.records().len(), 1);
        assert_eq!(outbox.records()[0].event_id(), first.event_id);
        assert_eq!(outbox.records()[0].route_id(), &first.route_id);
        assert_eq!(overflow.len(), 1);
        assert_eq!(overflow[0].event_id(), second.event_id);
        assert_eq!(overflow[0].route_id(), second.route_id.as_str());
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
                failed_detail(),
                "2026-07-15T12:00:01Z".to_owned(),
            )
            .expect("record failure");
        assert_eq!(attempt, 1);
        assert_eq!(
            outbox.records()[0]
                .last_error_detail()
                .and_then(DiagnosticDetail::delivery_failure_primary),
            Some(DeliveryFailurePrimary::UnsuccessfulExit)
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
                    failed_detail(),
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
                    failed_detail(),
                    TIME.to_owned(),
                )
                .is_err()
        );
        assert!(
            outbox
                .record_failure(
                    &event_id(2),
                    &route("alpha"),
                    failed_detail(),
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
            PendingOutboxRecord {
                event_kind: DeliveryEventKind::Reset,
                ..pending(1, "alpha", 0, None, TIME)
            },
            PendingOutboxRecord {
                condition_id: None,
                ..pending(1, "alpha", 0, None, TIME)
            },
            pending(1, "alpha", 0, Some(failed_detail()), TIME),
            pending(1, "alpha", 1, None, TIME),
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
    fn durable_failure_detail_survives_recording_and_attempt_overflow_is_rejected() {
        let mut outbox = Outbox(vec![pending(
            1,
            "alpha",
            u32::MAX,
            Some(failed_detail()),
            TIME,
        )]);
        assert!(
            outbox
                .record_failure(
                    &event_id(1),
                    &route("alpha"),
                    failed_detail(),
                    "2026-07-15T12:00:01Z".to_owned(),
                )
                .is_err()
        );
    }
}

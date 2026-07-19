//! Validation for one persisted pending outbox record.

use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::CoreError;

use super::{PendingOutboxRecord, TargetId, read_validated_process_stdin_payload_bytes};

impl PendingOutboxRecord {
    pub(super) fn validate(
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
        let payload = read_validated_process_stdin_payload_bytes(
            &self.immutable_payload,
            target_id,
            contract_digest_sha256,
            &self.event_id,
            &self.route_id,
            self.route_family,
        )?;
        if payload.event_kind() != self.event_kind
            || payload.condition_id() != self.condition_id.as_ref()
        {
            return Err(CoreError::contract(
                "outbox event metadata must match its immutable process payload",
            ));
        }
        if (self.attempt_count == 0) != self.last_error_detail.is_none() {
            return Err(CoreError::contract(
                "outbox last_error_detail must exist exactly when a delivery attempt has failed",
            ));
        }
        if let Some(detail) = &self.last_error_detail {
            detail.validate_durable_delivery_failure()?;
        }
        require_timestamp("outbox next_retry_at", &self.next_retry_at)
    }
}

pub(super) fn parse_timestamp(value: &str) -> Result<OffsetDateTime, CoreError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| CoreError::contract("outbox timestamp must be RFC 3339"))
}

pub(super) fn require_timestamp(field: &str, value: &str) -> Result<(), CoreError> {
    let timestamp = parse_timestamp(value)?;
    let canonical = timestamp
        .format(&Rfc3339)
        .map_err(|_| CoreError::internal("could not format timestamp"))?;
    if timestamp.offset() != time::UtcOffset::UTC || value != canonical {
        return Err(CoreError::contract(format!(
            "{field} must be canonical UTC RFC 3339"
        )));
    }
    Ok(())
}

pub(super) fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

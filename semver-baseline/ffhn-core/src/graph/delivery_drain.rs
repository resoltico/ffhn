//! Due snapshot delivery drain under the owning source writer lease.

use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::CoreError;

use super::{
    DeliveryExecution, MeasurementId, MeasurementLineage, OutboxOwner, SourceId, SourceLineage,
    SourceTurnEntry, TrustedGraphRoot, commit_delivery_result, execute_delivery_attempt,
};

/// Result of draining the first due source-owned record, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrainResult {
    /// Another writer holds the source lock, so this drain capability was not attempted.
    Locked,
    /// No source-owned pending record was due at this clock instant.
    Idle,
    /// One pending record was successfully delivered and removed.
    Delivered,
    /// One record failed and remains pending with append-only retry evidence.
    Retrying,
    /// One record reached terminal attempts and was dead-lettered.
    DeadLettered,
    /// Source lineage or a manifest made all delivery records unreachable.
    Unreachable,
}

impl DrainResult {
    /// Returns the stable report spelling.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Locked => "skipped_locked",
            Self::Idle => "idle",
            Self::Delivered => "delivered",
            Self::Retrying => "retrying",
            Self::DeadLettered => "dead_lettered",
            Self::Unreachable => "unreachable",
        }
    }
}

/// Drains at most one due measurement-owned record under the same source writer lease.
pub fn drain_measurement_outbox_once(
    graph: &TrustedGraphRoot,
    source_id: SourceId,
    measurement_id: MeasurementId,
    attempted_at_utc: String,
) -> Result<DrainResult, CoreError> {
    graph.validate_graph_documents()?;
    let attempted_at = OffsetDateTime::parse(&attempted_at_utc, &Rfc3339)?;
    let source = graph.open_source(source_id)?;
    let Some(_lease) = source.try_acquire_write_lease()? else {
        return Ok(DrainResult::Locked);
    };
    if !matches!(source.recover_turn_entry()?, SourceTurnEntry::Ready) {
        return Ok(DrainResult::Unreachable);
    }
    let inspection = source.inspect_lineage([measurement_id.clone()])?;
    let ready = match inspection.source() {
        SourceLineage::Ready(ready) => ready,
        SourceLineage::NeedsInitialization => return Ok(DrainResult::Idle),
        SourceLineage::Refused(_) => return Ok(DrainResult::Unreachable),
    };
    match inspection.measurement(&measurement_id) {
        Some(MeasurementLineage::Ready(_)) => {}
        Some(MeasurementLineage::NeverInitialized) | None => return Ok(DrainResult::Idle),
        Some(MeasurementLineage::Held(_)) => return Ok(DrainResult::Unreachable),
    }
    let storage = source.open_storage()?;
    let Some(record) = first_due(
        storage.read_measurement_delivery_records(&measurement_id)?,
        attempted_at,
    )?
    else {
        return Ok(DrainResult::Idle);
    };
    let execution = execute_delivery_attempt(record.clone(), attempted_at_utc)?;
    let result = result_for(&execution);
    commit_delivery_result(
        &source,
        &storage,
        ready.state(),
        &OutboxOwner::Measurement(measurement_id),
        &record,
        execution,
    )?;
    Ok(result)
}

/// Drains at most one due source-owned pending record under the exclusive source lock.
pub fn drain_source_outbox_once(
    graph: &TrustedGraphRoot,
    source_id: SourceId,
    attempted_at_utc: String,
) -> Result<DrainResult, CoreError> {
    graph.validate_graph_documents()?;
    let attempted_at = OffsetDateTime::parse(&attempted_at_utc, &Rfc3339)?;
    let source = graph.open_source(source_id)?;
    let Some(_lease) = source.try_acquire_write_lease()? else {
        return Ok(DrainResult::Locked);
    };
    if !matches!(source.recover_turn_entry()?, SourceTurnEntry::Ready) {
        return Ok(DrainResult::Unreachable);
    }
    let inspection = source.inspect_lineage([])?;
    let ready = match inspection.source() {
        SourceLineage::Ready(ready) => ready,
        SourceLineage::NeedsInitialization => return Ok(DrainResult::Idle),
        SourceLineage::Refused(_) => return Ok(DrainResult::Unreachable),
    };
    let storage = source.open_storage()?;
    let Some(record) = first_due(storage.read_source_delivery_records()?, attempted_at)? else {
        return Ok(DrainResult::Idle);
    };
    let execution = execute_delivery_attempt(record.clone(), attempted_at_utc)?;
    let result = result_for(&execution);
    commit_delivery_result(
        &source,
        &storage,
        ready.state(),
        &OutboxOwner::Source,
        &record,
        execution,
    )?;
    Ok(result)
}

fn first_due(
    records: Vec<super::DeliveryRecord>,
    attempted_at: OffsetDateTime,
) -> Result<Option<super::DeliveryRecord>, CoreError> {
    for record in records {
        if OffsetDateTime::parse(record.next_retry_at_utc(), &Rfc3339)? <= attempted_at {
            return Ok(Some(record));
        }
    }
    Ok(None)
}

const fn result_for(execution: &DeliveryExecution) -> DrainResult {
    match execution {
        DeliveryExecution::Delivered => DrainResult::Delivered,
        DeliveryExecution::Retry(_) => DrainResult::Retrying,
        DeliveryExecution::DeadLetter(_) => DrainResult::DeadLettered,
    }
}

#[cfg(test)]
#[path = "delivery_drain/tests.rs"]
mod tests;

//! Pure fail-soft wake-time calculation over durable schedule/retry facts and deferrals.

use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::CoreError;

use super::super::TrustedGraphRoot;
use super::{AgentWorker, SourceDeferrals};

const IDLE_WAKE_MS: i64 = 1_000;

pub(super) fn next_wake_at(
    worker: &AgentWorker,
    graph: &TrustedGraphRoot,
    now: OffsetDateTime,
) -> Result<OffsetDateTime, CoreError> {
    let mut candidates = Vec::new();
    let mut source_ids = graph.source_ids()?;
    source_ids.sort_unstable();
    for source_id in source_ids {
        candidates.extend(source_wake_candidates(worker, graph, source_id, now));
    }
    Ok(candidates
        .into_iter()
        .min()
        .unwrap_or(now + Duration::milliseconds(IDLE_WAKE_MS)))
}

pub(super) fn source_wake_candidates(
    worker: &AgentWorker,
    graph: &TrustedGraphRoot,
    source_id: super::super::SourceId,
    now: OffsetDateTime,
) -> Vec<OffsetDateTime> {
    source_wake_candidates_with(
        worker,
        graph,
        source_id,
        now,
        |source| source.inspect_lineage([]),
        super::super::TrustedSourceDir::open_storage,
    )
}

pub(super) fn source_wake_candidates_with(
    worker: &AgentWorker,
    graph: &TrustedGraphRoot,
    source_id: super::super::SourceId,
    now: OffsetDateTime,
    inspect: impl FnOnce(
        &super::super::TrustedSourceDir,
    ) -> Result<super::super::LineageInspection, CoreError>,
    open_storage: impl FnOnce(
        &super::super::TrustedSourceDir,
    ) -> Result<super::super::TrustedStorageDir, CoreError>,
) -> Vec<OffsetDateTime> {
    let mut candidates = Vec::new();
    let deferrals = worker.deferrals.get(&source_id);
    let fallback = deferred_fallback(deferrals, now);
    let Some(source) = or_fallback(graph.open_source(source_id), &mut candidates, fallback) else {
        return candidates;
    };
    let Some(inspection) = or_fallback(inspect(&source), &mut candidates, fallback) else {
        return candidates;
    };
    let source_due = inspection
        .source()
        .as_ready_state()
        .and_then(|state| state.next_due_utc())
        .and_then(parse_utc)
        .unwrap_or(now);
    let source_config = source.read_source_document();
    if source_config.as_ref().is_ok_and(|config| config.enabled())
        || deferrals.is_some_and(|state| state.acquisition_until.is_some())
    {
        candidates.push(max_with_source_defer(
            source_due,
            deferrals.and_then(|state| state.acquisition_until),
        ));
    }
    if inspection.source().as_ready_state().is_none() {
        return candidates;
    }
    let Some(storage) = or_fallback(open_storage(&source), &mut candidates, fallback) else {
        return candidates;
    };
    for record in records_or_fallback(
        storage.read_source_delivery_records(),
        &mut candidates,
        fallback,
    ) {
        push_source_retry(
            &mut candidates,
            parse_utc(record.next_retry_at_utc()),
            deferrals,
        );
    }
    for measurement_id in inspection.measurements().keys() {
        for record in records_or_fallback(
            storage.read_measurement_delivery_records(measurement_id),
            &mut candidates,
            fallback,
        ) {
            push_measurement_retry(
                &mut candidates,
                parse_utc(record.next_retry_at_utc()),
                deferrals,
                measurement_id,
            );
        }
    }
    candidates
}

pub(super) fn push_source_retry(
    candidates: &mut Vec<OffsetDateTime>,
    retry: Option<OffsetDateTime>,
    deferrals: Option<&SourceDeferrals>,
) {
    if let Some(retry) = retry {
        candidates.push(max_with_source_defer(
            retry,
            deferrals.and_then(|state| state.drain_until),
        ));
    }
}

pub(super) fn push_measurement_retry(
    candidates: &mut Vec<OffsetDateTime>,
    retry: Option<OffsetDateTime>,
    deferrals: Option<&SourceDeferrals>,
    measurement_id: &super::super::MeasurementId,
) {
    if let Some(retry) = retry {
        let source_limited =
            max_with_source_defer(retry, deferrals.and_then(|state| state.drain_until));
        let measurement_limited = deferrals
            .and_then(|state| state.measurement_drain_until.get(measurement_id))
            .copied()
            .unwrap_or(source_limited);
        candidates.push(source_limited.max(measurement_limited));
    }
}

pub(super) fn or_fallback<T>(
    result: Result<T, CoreError>,
    candidates: &mut Vec<OffsetDateTime>,
    fallback: OffsetDateTime,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(_) => {
            candidates.push(fallback);
            None
        }
    }
}

pub(super) fn records_or_fallback<T>(
    result: Result<Vec<T>, CoreError>,
    candidates: &mut Vec<OffsetDateTime>,
    fallback: OffsetDateTime,
) -> Vec<T> {
    or_fallback(result, candidates, fallback).unwrap_or_default()
}

pub(super) fn deferred_fallback(
    deferrals: Option<&SourceDeferrals>,
    now: OffsetDateTime,
) -> OffsetDateTime {
    deferrals
        .into_iter()
        .flat_map(|state| {
            state
                .acquisition_until
                .into_iter()
                .chain(state.drain_until)
                .chain(state.measurement_drain_until.values().copied())
        })
        .min()
        .unwrap_or(now + Duration::milliseconds(IDLE_WAKE_MS))
}

pub(super) fn max_with_source_defer(
    candidate: OffsetDateTime,
    deferred: Option<OffsetDateTime>,
) -> OffsetDateTime {
    candidate.max(deferred.unwrap_or(candidate))
}

pub(super) fn parse_utc(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339).ok()
}

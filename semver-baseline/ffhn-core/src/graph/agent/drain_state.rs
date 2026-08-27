use time::{Duration, OffsetDateTime};

use crate::CoreError;

use super::super::{DrainResult, MeasurementId};
use super::{CONTENDED_DEFER_MS, DeferralReason, SourceDeferrals};

pub(super) fn drain_measurements(
    measurement_ids: Vec<MeasurementId>,
    deferrals: &mut SourceDeferrals,
    now: OffsetDateTime,
    source_interval_ms: i64,
    mut drain: impl FnMut(&MeasurementId) -> Result<DrainResult, CoreError>,
) -> (Vec<(String, DrainResult)>, Option<String>) {
    let mut outcomes = Vec::with_capacity(measurement_ids.len());
    for measurement_id in measurement_ids {
        if !deferrals.measurement_drain_permitted(&measurement_id, now) {
            continue;
        }
        match record_measurement_drain_result(
            deferrals,
            &measurement_id,
            now,
            source_interval_ms,
            drain(&measurement_id),
            &mut outcomes,
        ) {
            Ok(true) => break,
            Ok(false) => {}
            Err(error) => return (outcomes, Some(error)),
        }
    }
    (outcomes, None)
}

pub(super) fn finish_source_drain(
    deferrals: &mut SourceDeferrals,
    source_drain: DrainResult,
    measurement_drains: Vec<(String, DrainResult)>,
    measurement_error: Option<String>,
) -> (
    Option<DrainResult>,
    Option<String>,
    Vec<(String, DrainResult)>,
) {
    if let Some(error) = measurement_error {
        return (Some(source_drain), Some(error), measurement_drains);
    }
    if source_drain != DrainResult::Unreachable {
        deferrals.drain_until = None;
        deferrals.drain_reason = None;
    }
    (Some(source_drain), None, measurement_drains)
}

pub(super) fn record_measurement_drain_result(
    deferrals: &mut SourceDeferrals,
    measurement_id: &MeasurementId,
    now: OffsetDateTime,
    source_interval_ms: i64,
    result: Result<DrainResult, CoreError>,
    outcomes: &mut Vec<(String, DrainResult)>,
) -> Result<bool, String> {
    match result {
        Ok(DrainResult::Locked) => {
            deferrals.defer_drain(now, CONTENDED_DEFER_MS, DeferralReason::LockContention);
            outcomes.push((measurement_id.as_str().to_owned(), DrainResult::Locked));
            Ok(true)
        }
        Ok(DrainResult::Unreachable) => {
            deferrals.measurement_drain_until.insert(
                measurement_id.clone(),
                now + Duration::milliseconds(source_interval_ms),
            );
            deferrals
                .measurement_drain_reason
                .insert(measurement_id.clone(), DeferralReason::DeliveryUnreachable);
            outcomes.push((measurement_id.as_str().to_owned(), DrainResult::Unreachable));
            Ok(false)
        }
        Ok(result) => {
            deferrals.measurement_drain_until.remove(measurement_id);
            deferrals.measurement_drain_reason.remove(measurement_id);
            outcomes.push((measurement_id.as_str().to_owned(), result));
            Ok(false)
        }
        Err(error) => {
            deferrals.measurement_drain_until.insert(
                measurement_id.clone(),
                now + Duration::milliseconds(source_interval_ms),
            );
            deferrals
                .measurement_drain_reason
                .insert(measurement_id.clone(), DeferralReason::Unreadable);
            Err(error.to_string())
        }
    }
}

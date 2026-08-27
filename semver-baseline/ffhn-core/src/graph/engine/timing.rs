//! UTC scheduling helpers shared by live acquisition and atomic document processing.

use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::CoreError;

use super::super::{SourceDocument, SourceState};

pub(super) fn scheduled(
    state: &SourceState,
    source: &SourceDocument,
    now: &str,
) -> Result<SourceState, CoreError> {
    let completed = OffsetDateTime::parse(now, &Rfc3339)?;
    let milliseconds = i64::try_from(source.schedule().interval_ms())
        .map_err(|_| CoreError::contract("source interval overflows UTC clock range"))?;
    let due = completed
        .checked_add(Duration::milliseconds(milliseconds))
        .ok_or_else(|| CoreError::contract("source next due time overflows UTC clock range"))?
        .format(&Rfc3339)?;
    state
        .with_cycle_schedule(now.to_owned(), due)?
        .next_generation()
}

pub(super) fn now() -> Result<String, CoreError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(CoreError::from)
}

use std::time::{Duration, Instant};

pub(crate) fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

pub(crate) fn elapsed_ms(started: &Instant) -> u64 {
    duration_ms(started.elapsed())
}

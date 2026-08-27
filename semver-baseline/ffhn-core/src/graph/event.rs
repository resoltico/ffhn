//! Immutable, route-independent v11 event-envelope ownership.

#[path = "event/key.rs"]
mod key;

#[path = "event/envelope.rs"]
mod envelope;

pub use envelope::{EventEmitter, EventEnvelope, EventEnvelopeParts, EventObservation};
pub use key::{EventKey, EventKind};

/// Canonical event-envelope schema name.
pub const EVENT_ENVELOPE_SCHEMA_NAME: &str = "ffhn.event_envelope";
/// Canonical event-envelope schema version.
pub const EVENT_ENVELOPE_SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
#[path = "event/tests.rs"]
mod tests;

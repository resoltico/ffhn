//! Durable delivery contracts decomposed by route, event identity, and wire payload.

mod event;
mod payload;
mod route;

pub(crate) use event::ProcessStdinEventKey;
pub use event::{DeliveryEventKind, RouteFamily};
pub(crate) use payload::{ProcessStdinPayload, read_validated_process_stdin_payload_bytes};
pub(crate) use route::validate_routes;
pub use route::{DeliveryAdapter, DeliveryRoute, OutboxPolicy, RouteId};

#[cfg(test)]
mod tests;

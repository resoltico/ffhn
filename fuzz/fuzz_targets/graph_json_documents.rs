#![no_main]

use ffhn_core::graph::{
    CommitManifest, DeadLetter, DeliveryRecord, EventEnvelope, GraphIdentity, LineageManifest,
    MeasurementState, SourceIdentity, SourceState,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = serde_json::from_slice::<GraphIdentity>(data);
    let _ = serde_json::from_slice::<SourceIdentity>(data);
    let _ = serde_json::from_slice::<SourceState>(data);
    let _ = serde_json::from_slice::<MeasurementState>(data);
    let _ = serde_json::from_slice::<CommitManifest>(data);
    let _ = serde_json::from_slice::<LineageManifest>(data);
    let _ = serde_json::from_slice::<EventEnvelope>(data);
    let _ = serde_json::from_slice::<DeliveryRecord>(data);
    let _ = serde_json::from_slice::<DeadLetter>(data);
});

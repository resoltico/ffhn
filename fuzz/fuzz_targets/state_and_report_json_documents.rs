#![no_main]

use ffhn_core::{BatchRunReport, ResetReport, RunReport, StateDocument, StatusReport};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(state) = serde_json::from_slice::<StateDocument>(data) {
        let _ = state.validate();
    }
    let _ = serde_json::from_slice::<RunReport>(data);
    let _ = serde_json::from_slice::<StatusReport>(data);
    let _ = serde_json::from_slice::<BatchRunReport>(data);
    let _ = serde_json::from_slice::<ResetReport>(data);
});

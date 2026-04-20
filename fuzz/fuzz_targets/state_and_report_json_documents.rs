#![no_main]

use ffhn_core::{BatchRunReport, RunReport, StateDocument, StatusReport};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(state) = serde_json::from_slice::<StateDocument>(data) {
        let _ = state.validate();
    }
    if let Ok(report) = serde_json::from_slice::<RunReport>(data) {
        let _ = report.validate();
    }
    if let Ok(report) = serde_json::from_slice::<StatusReport>(data) {
        let _ = report.validate();
    }
    if let Ok(report) = serde_json::from_slice::<BatchRunReport>(data) {
        let _ = report.validate();
    }
});

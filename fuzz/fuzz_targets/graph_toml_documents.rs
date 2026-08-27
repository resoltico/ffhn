#![no_main]

use ffhn_core::graph::{AgentDocument, MeasurementDocument, SourceDocument};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = toml::from_str::<AgentDocument>(text);
        let _ = toml::from_str::<SourceDocument>(text);
        let _ = toml::from_str::<MeasurementDocument>(text);
    }
});

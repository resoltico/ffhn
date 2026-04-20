#![no_main]

use ffhn_core::TargetDocument;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data)
        && let Ok(target) = toml::from_str::<TargetDocument>(text)
    {
        let _ = target.validate();
    }
});

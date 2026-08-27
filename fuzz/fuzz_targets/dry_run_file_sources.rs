#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use ffhn_core::graph::{
    GraphPaths, MeasurementDocument, SourceDocument, SourceId, TrustedGraphRoot,
    measure_source_dry_run,
};
use libfuzzer_sys::fuzz_target;
use tempfile::tempdir;

#[derive(Debug)]
struct DryRunInput {
    json: String,
    source_id: String,
    measurement_id: String,
    pointer_token: String,
}

impl<'a> Arbitrary<'a> for DryRunInput {
    fn arbitrary(input: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            json: input.arbitrary()?,
            source_id: input.arbitrary()?,
            measurement_id: input.arbitrary()?,
            pointer_token: input.arbitrary()?,
        })
    }
}

fuzz_target!(|input: DryRunInput| {
    let temporary = tempdir().expect("temporary directory");
    let graph_root = temporary.path().join("graph");
    let source_id = safe_id(&input.source_id, "source");
    let measurement_id = safe_id(&input.measurement_id, "measurement");
    let source_file = temporary.path().join("source.json");
    std::fs::write(&source_file, normalized_json(&input.json)).expect("source JSON");

    let graph = TrustedGraphRoot::initialize(
        GraphPaths::new(graph_root),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("graph initialization");
    let source: SourceDocument =
        toml::from_str(&source_toml(&source_id, &source_file)).expect("source document");
    let source_dir = graph
        .create_source_document(&source)
        .expect("source creation");
    let measurement: MeasurementDocument = toml::from_str(&measurement_toml(
        &measurement_id,
        &json_pointer(&input.pointer_token),
    ))
    .expect("measurement document");
    source_dir
        .create_measurement_document(&measurement)
        .expect("measurement creation");

    let _ = measure_source_dry_run(
        &graph,
        SourceId::new(source_id).expect("normalized source id"),
    );
});

fn safe_id(raw: &str, fallback: &str) -> String {
    let mut result = raw
        .chars()
        .filter_map(|character| {
            let lower = character.to_ascii_lowercase();
            if lower.is_ascii_lowercase() || lower.is_ascii_digit() {
                Some(lower)
            } else if matches!(lower, '-' | '_') {
                Some('-')
            } else {
                None
            }
        })
        .collect::<String>();
    result.truncate(32);
    while result.ends_with('-') {
        result.pop();
    }
    if result.is_empty() {
        fallback.to_owned()
    } else {
        result
    }
}

fn normalized_json(raw: &str) -> String {
    if serde_json::from_str::<serde_json::Value>(raw).is_ok() {
        raw.to_owned()
    } else {
        r#"{"value":1}"#.to_owned()
    }
}

fn json_pointer(raw: &str) -> String {
    format!("/{}", raw.replace('~', "~0").replace('/', "~1"))
}

fn source_toml(source_id: &str, source_file: &std::path::Path) -> String {
    format!(
        "schema_name = \"ffhn.source\"\nschema_version = 1\nsource_id = {source_id:?}\ndisplay_name = \"Fuzz source\"\nenabled = true\nescalate_after = 3\n[fetch]\nengine = \"file\"\nfile_path = {:?}\nmax_bytes = 2000000\n[conditional]\nenabled = false\n[schedule]\ninterval_ms = 1000\nmin_interval_ms = 1000\n",
        source_file.to_string_lossy(),
    )
}

fn measurement_toml(measurement_id: &str, pointer: &str) -> String {
    format!(
        "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = {measurement_id:?}\ndisplay_name = \"Fuzz measurement\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"text\"\nconditions = []\n[projection]\nkind = \"json_pointer\"\npointer = {pointer:?}\n"
    )
}

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use ffhn_core::{TargetPaths, run_once_dry_run};
use libfuzzer_sys::fuzz_target;
use tempfile::tempdir;

#[derive(Debug)]
struct DryRunInput {
    json: String,
    target_id: String,
    pointer: String,
}

impl<'a> Arbitrary<'a> for DryRunInput {
    fn arbitrary(input: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            json: input.arbitrary()?,
            target_id: input.arbitrary()?,
            pointer: input.arbitrary()?,
        })
    }
}

fuzz_target!(|input: DryRunInput| {
    let temporary = tempdir().expect("temporary directory");
    let target_id = safe_target_id(&input.target_id);
    let paths = TargetPaths::try_new(temporary.path(), target_id.clone())
        .expect("normalized target id");
    std::fs::create_dir_all(paths.target_dir()).expect("target directory");
    let source_path = paths.target_dir().join("source.json");
    std::fs::write(&source_path, normalized_json(&input.json)).expect("source JSON");
    std::fs::write(
        paths.target_file(),
        target_toml(&target_id, &source_path.to_string_lossy(), &input.pointer),
    )
    .expect("target TOML");

    let _ = run_once_dry_run(&paths);
});

fn safe_target_id(raw: &str) -> String {
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
    if result.is_empty() || !result.starts_with(|character: char| character.is_ascii_alphanumeric()) {
        result.insert(0, 'd');
    }
    result.truncate(32);
    while result.ends_with(['-', '_']) {
        result.pop();
    }
    if result.is_empty() {
        "demo".to_owned()
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

fn target_toml(target_id: &str, source_path: &str, pointer: &str) -> String {
    format!(
        "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"{target_id}\"\ndisplay_name = \"Fuzz\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 2000000\n\n[projection]\nkind = \"json_pointer\"\npointer = {pointer:?}\n"
    )
}

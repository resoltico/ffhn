pub(super) use super::super::delivery::validate_routes;
pub(super) use super::super::observation::parse::{
    decode_json_pointer_token, json_scalar_value, normalize_decimal_input,
    normalize_grouped_decimal, normalized_raw_token, parse_canonical_value, parse_datetime,
    parse_json_array_index, parse_offset, select_json_pointer_child, select_json_scalar_token,
};
pub(super) use super::super::policy::POLICY_EVALUATION_SEMANTICS_VERSION;
pub(super) use super::super::state::is_sha256;
pub(super) use super::super::target::{
    default_follow_redirects, default_max_bytes, default_timeout_ms,
    permanent_code_for_htmlcut_failure, require_text, validate_json_pointer, validate_max_bytes,
    validate_type_params,
};
pub(super) use super::super::*;
pub(super) use crate::model::{htmlcut_detail, io_detail, plain_detail};

pub(super) fn target(declared_type: &str, type_params: &str) -> TargetDocument {
    let source_path = crate::test_support::absolute_file_path("source.json");
    let document: TargetDocument = toml::from_str(&format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"{declared_type}\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n{type_params}\n"
        ))
        .expect("target toml");
    document.validate().expect("valid target");
    document
}

pub(super) fn mutate_target(
    document: &TargetDocument,
    mutate: impl FnOnce(&mut serde_json::Value),
) -> TargetDocument {
    let mut wire = serde_json::to_value(document).expect("target JSON");
    mutate(&mut wire);
    serde_json::from_value(wire).expect("target JSON remains structurally valid")
}

pub(super) fn http_target() -> TargetDocument {
    let document: TargetDocument = toml::from_str(
        "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"http\"\ndisplay_name = \"HTTP\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"http\"\nsource_url = \"https://example.test/value\"\n\n[fetch]\nengine = \"http\"\nmethod = \"GET\"\ntimeout_ms = 1000\nmax_bytes = 1024\nuser_agent = \"ffhn-test\"\nfollow_redirects = false\naccept = \"application/json\"\n\n[fetch.headers]\nX-Test = \"yes\"\n\n[projection]\nkind = \"json_pointer\"\npointer = \"\"\n",
    )
    .expect("HTTP target TOML");
    document.validate().expect("valid HTTP target");
    document
}

pub(super) fn html_target(
    projection_kind: &str,
    selector: &str,
    attribute_name: Option<&str>,
    declared_type: &str,
    type_params: &str,
) -> TargetDocument {
    let source_path = crate::test_support::absolute_file_path("source.html");
    let attribute = attribute_name
        .map(|name| format!("name = {name:?}\n"))
        .unwrap_or_default();
    let document: TargetDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"html\"\ndisplay_name = \"HTML\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"{declared_type}\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"{projection_kind}\"\n{attribute}\n[projection.selection.strategy]\nkind = \"css_selector\"\nselector = {selector:?}\n\n[projection.selection.selection]\nmode = \"single\"\n\n[projection.selection.rendering]\nwhitespace = \"rendered\"\nrewrite_urls = false\n{type_params}\n"
    ))
    .expect("HTML target TOML");
    document.validate().expect("valid HTML target");
    document
}

pub(super) fn mutate_state(
    document: &StateDocument,
    mutate: impl FnOnce(&mut serde_json::Value),
) -> StateDocument {
    let mut wire = serde_json::to_value(document).expect("state JSON");
    mutate(&mut wire);
    StateDocument::from_unvalidated_wire_for_test(wire)
}

pub(super) fn immutable_payload_bytes(value: &serde_json::Value) -> Vec<u8> {
    value
        .as_array()
        .expect("payload byte array")
        .iter()
        .map(|value| u8::try_from(value.as_u64().expect("payload byte")).expect("u8"))
        .collect()
}

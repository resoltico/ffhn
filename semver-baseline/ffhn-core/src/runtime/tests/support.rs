#[cfg(test)]
pub(super) use super::super::acquire::{
    acquire_json_scalar, fetch_http_response, fetch_http_source, fetch_source, html_input,
    read_file_source,
};
#[cfg(test)]
pub(super) use super::super::execution::{run_batch as run_batch_internal, run_once_with_mode};
#[cfg(all(test, unix))]
pub(super) use super::super::lock::LockError;
#[cfg(test)]
pub(super) use super::super::lock::lock_exclusive;
#[cfg(all(test, unix))]
pub(super) use super::super::lock::lock_shared;
#[cfg(test)]
pub(super) use super::super::report::detail_from_error_for_operation;
#[cfg(test)]
pub(super) use super::super::storage::{blind_remove_storage_root, load_state, load_target};
#[cfg(test)]
pub(super) use crate::*;
#[cfg(test)]
pub(super) use std::fs;
#[cfg(test)]
pub(super) use std::io::{Read, Write};
#[cfg(all(test, unix))]
pub(super) use std::os::unix::fs::PermissionsExt;
#[cfg(test)]
pub(super) use std::thread;

pub(super) use std::collections::BTreeMap;
pub(super) use std::net::TcpListener;
pub(super) use std::sync::mpsc;
pub(super) use tempfile::tempdir;

pub(super) fn write_target(
    paths: &TargetPaths,
    declared_type: &str,
    type_params: &str,
    pointer: &str,
) {
    write_target_with_conditions(
        paths,
        declared_type,
        type_params,
        pointer,
        "conditions = []",
    );
}

pub(super) fn write_target_with_conditions(
    paths: &TargetPaths,
    declared_type: &str,
    type_params: &str,
    pointer: &str,
    conditions: &str,
) {
    let source_path = format!(
        "{:?}",
        paths.target_dir().join("source.json").to_string_lossy()
    );
    fs::create_dir_all(paths.target_dir()).expect("create target directory");
    fs::write(
            paths.target_file(),
            format!(
                "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"{}\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"{declared_type}\"\n{conditions}\n\n[target]\nkind = \"file\"\nfile_path = {source_path}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"{pointer}\"\n{type_params}\n",
                paths.target_id(),
            ),
        )
        .expect("write target");
}

pub(super) fn write_html_target(
    paths: &TargetPaths,
    projection_kind: &str,
    selector: &str,
    attribute_name: Option<&str>,
    declared_type: &str,
    type_params: &str,
) {
    let source_path = format!(
        "{:?}",
        paths.target_dir().join("source.html").to_string_lossy()
    );
    let attribute = attribute_name
        .map(|name| format!("name = {name:?}\n"))
        .unwrap_or_default();
    fs::create_dir_all(paths.target_dir()).expect("create target directory");
    fs::write(
        paths.target_file(),
        format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"{}\"\ndisplay_name = \"HTML\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"{declared_type}\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"{projection_kind}\"\n{attribute}\n[projection.selection.strategy]\nkind = \"css_selector\"\nselector = {selector:?}\n\n[projection.selection.selection]\nmode = \"single\"\n\n[projection.selection.rendering]\nwhitespace = \"rendered\"\nrewrite_urls = false\n{type_params}\n",
            paths.target_id(),
        ),
    )
    .expect("write HTML target");
}

#[cfg(unix)]
pub(super) fn write_delivery_target(
    paths: &TargetPaths,
    route_args: &str,
    max_pending: usize,
    max_attempts: u32,
    conditions: &str,
) {
    fs::create_dir_all(paths.target_dir()).expect("create target directory");
    fs::write(
        paths.target_file(),
        format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"{}\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\n{conditions}\n\n[target]\nkind = \"file\"\nfile_path = \"{}\"\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n\n[outbox]\nmax_pending = {max_pending}\nmax_attempts = {max_attempts}\nbase_backoff_ms = 10\nmax_backoff_ms = 20\n\n[[routes]]\nroute_id = \"condition\"\nroute_family = \"on_condition\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = \"/bin/sh\"\nargs = {route_args}\ntimeout_ms = 1000\n",
            paths.target_id(),
            paths.target_dir().join("source.json").display(),
        ),
    )
    .expect("write delivery target");
}

pub(super) fn write_portable_delivery_target(
    paths: &TargetPaths,
    mode: &str,
    output: Option<&std::path::Path>,
    max_pending: usize,
    max_attempts: u32,
    conditions: &str,
) {
    let (program, args) = super::super::delivery::process::test_process_command(mode, output);
    let arguments = toml::Value::Array(
        args.into_iter()
            .map(toml::Value::String)
            .collect::<Vec<_>>(),
    )
    .to_string();
    let program = toml::Value::String(program.display().to_string()).to_string();
    let source_path = format!(
        "{:?}",
        paths.target_dir().join("source.json").to_string_lossy()
    );
    fs::create_dir_all(paths.target_dir()).expect("create target directory");
    fs::write(
        paths.target_file(),
        format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"{}\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\n{conditions}\n\n[target]\nkind = \"file\"\nfile_path = {source_path}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n\n[outbox]\nmax_pending = {max_pending}\nmax_attempts = {max_attempts}\nbase_backoff_ms = 10\nmax_backoff_ms = 20\n\n[[routes]]\nroute_id = \"condition\"\nroute_family = \"on_condition\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = {program}\nargs = {arguments}\ntimeout_ms = 1000\n",
            paths.target_id(),
        ),
    )
    .expect("write portable delivery target");
}

pub(super) fn make_pending_outbox_due(paths: &TargetPaths) {
    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("state bytes"))
            .expect("state JSON");
    state["outbox"][0]["next_retry_at"] = serde_json::json!("1970-01-01T00:00:00Z");
    fs::write(
        paths.state_file(),
        serde_json::to_vec(&state).expect("state JSON"),
    )
    .expect("make pending record due");
}

pub(super) fn fixture_paths() -> (tempfile::TempDir, TargetPaths) {
    let temporary = tempdir().expect("temporary directory");
    let watch_root = temporary.path().join("watchlist");
    fs::create_dir_all(&watch_root).expect("create watch root");
    let paths = TargetPaths::try_new(&watch_root, "demo").expect("target paths");
    (temporary, paths)
}

pub(super) fn assert_htmlcut_preflight_permanent_episode(paths: &TargetPaths) {
    let first = run_once(paths).expect("first permanent HTMLCut preflight error");
    assert_eq!(first.outcome(), RunOutcome::ConfigInvalid);
    assert!(first.state_persisted());
    let failure = first
        .error_detail()
        .and_then(DiagnosticDetail::htmlcut_failure)
        .cloned()
        .expect("HTMLCut preflight evidence");
    assert_eq!(failure.error_class(), crate::HtmlcutErrorClass::PlanInvalid);
    assert_eq!(failure.candidate_count(), None);
    assert_eq!(failure.plan_digest_sha256().len(), 64);
    assert!(
        failure
            .plan_digest_sha256()
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    assert!(failure.diagnostics().is_empty());

    let first_state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("first state"))
            .expect("first state JSON");
    assert_eq!(
        first_state["permanent_error_episode"]["error_code"],
        "htmlcut_plan_invalid"
    );
    assert_eq!(first_state["source_health"]["state"], "healthy");
    assert!(first_state["accepted_observation"].is_null());

    let second = run_once(paths).expect("repeated permanent HTMLCut preflight error");
    assert_eq!(second.outcome(), RunOutcome::ConfigInvalid);
    assert_eq!(
        second
            .error_detail()
            .and_then(DiagnosticDetail::htmlcut_failure),
        Some(&failure)
    );
    let second_state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("second state"))
            .expect("second state JSON");
    assert_eq!(
        second_state["permanent_error_episode"]["first_seen_at"],
        first_state["permanent_error_episode"]["first_seen_at"]
    );
    assert_eq!(second_state["source_health"]["state"], "healthy");
}

pub(super) fn replace_state_contract_digest(paths: &TargetPaths, contract_digest_sha256: String) {
    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("state bytes"))
            .expect("state JSON");
    state["contract_digest_sha256"] = serde_json::Value::String(contract_digest_sha256);
    fs::write(
        paths.state_file(),
        serde_json::to_vec(&state).expect("state JSON"),
    )
    .expect("replace state contract digest");
}

pub(super) fn assert_policy_semantics_change_requires_reset(paths: &TargetPaths) {
    assert_eq!(
        run_once(paths)
            .expect("initialize current semantics")
            .outcome(),
        RunOutcome::Initialized
    );
    let target = load_target(paths).expect("target");
    let changed_digest = target
        .contract_digest_sha256_with_semantics_versions_for_test(
            crate::model::POLICY_EVALUATION_SEMANTICS_VERSION + 1,
            htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION,
        )
        .expect("changed policy semantics digest");
    assert_ne!(
        target.contract_digest_sha256().expect("current digest"),
        changed_digest
    );
    replace_state_contract_digest(paths, changed_digest);
    let state_under_changed_semantics = fs::read(paths.state_file()).expect("changed state");

    assert_eq!(
        run_once(paths)
            .expect("refused changed semantics")
            .outcome(),
        RunOutcome::RefusedContractDigest
    );
    assert_eq!(
        fs::read(paths.state_file()).expect("state remains unchanged"),
        state_under_changed_semantics
    );

    let reset_report = reset(paths).expect("blind reset");
    assert!(
        serde_json::to_value(reset_report).expect("reset JSON")["storage_cleared"]
            .as_bool()
            .expect("storage_cleared boolean")
    );
    assert!(!paths.storage_root().exists());
    assert_eq!(
        run_once(paths).expect("initialize after reset").outcome(),
        RunOutcome::Initialized
    );
}

pub(super) fn fetch_http_from_once(
    response: &str,
    max_bytes: usize,
) -> Result<String, DiagnosticDetail> {
    fetch_http_bytes_from_once(response.as_bytes().to_vec(), max_bytes)
}

pub(super) fn fetch_http_bytes_from_once(
    response: Vec<u8>,
    max_bytes: usize,
) -> Result<String, DiagnosticDetail> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let (ready, received) = mpsc::channel();
    let worker = thread::spawn(move || {
        ready.send(()).expect("ready");
        let (mut stream, _) = listener.accept().expect("request");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request).expect("read request");
        stream.write_all(&response).expect("response");
    });
    received.recv().expect("server ready");
    let mut headers = BTreeMap::new();
    headers.insert("X-FFHN-Test".to_owned(), "typed".to_owned());
    let result = fetch_http_source(
        &url::Url::parse(&format!("http://{address}/value")).expect("URL"),
        1_000,
        max_bytes,
        "ffhn-test",
        false,
        "application/json",
        &headers,
    );
    worker.join().expect("server worker");
    result
}

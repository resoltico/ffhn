#[cfg(test)]
use super::acquire::{
    acquire_json_scalar, fetch_http_response, fetch_http_source, fetch_source, html_input,
    read_file_source,
};
#[cfg(test)]
use super::execution::{run_batch as run_batch_internal, run_once_with_mode};
#[cfg(test)]
use super::lock::lock_exclusive;
#[cfg(all(test, unix))]
use super::lock::{LockError, lock_shared};
#[cfg(test)]
use super::report::detail_from_error;
#[cfg(test)]
use super::storage::{blind_remove_storage_root, load_state, load_target};
#[cfg(test)]
use crate::*;
#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::io::{Read, Write};
#[cfg(all(test, unix))]
use std::os::unix::fs::PermissionsExt;
#[cfg(test)]
use std::thread;
#[cfg(test)]
use std::time::Duration;

use std::collections::BTreeMap;
use std::net::TcpListener;
use std::sync::mpsc;
use tempfile::tempdir;

fn write_target(paths: &TargetPaths, declared_type: &str, type_params: &str, pointer: &str) {
    write_target_with_conditions(
        paths,
        declared_type,
        type_params,
        pointer,
        "conditions = []",
    );
}

fn write_target_with_conditions(
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
                "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"{}\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"{declared_type}\"\n{conditions}\n\n[target]\nkind = \"file\"\nfile_path = {source_path}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"{pointer}\"\n{type_params}\n",
                paths.target_id(),
            ),
        )
        .expect("write target");
}

fn write_html_target(
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
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"{}\"\ndisplay_name = \"HTML\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"{declared_type}\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"{projection_kind}\"\n{attribute}\n[projection.selection.strategy]\nkind = \"css_selector\"\nselector = {selector:?}\n\n[projection.selection.selection]\nmode = \"single\"\n\n[projection.selection.rendering]\nwhitespace = \"rendered\"\nrewrite_urls = false\n{type_params}\n",
            paths.target_id(),
        ),
    )
    .expect("write HTML target");
}

#[cfg(unix)]
fn write_delivery_target(
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
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"{}\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\n{conditions}\n\n[target]\nkind = \"file\"\nfile_path = \"{}\"\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n\n[outbox]\nmax_pending = {max_pending}\nmax_attempts = {max_attempts}\nbase_backoff_ms = 10\nmax_backoff_ms = 20\n\n[[routes]]\nroute_id = \"condition\"\nroute_family = \"on_condition\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = \"/bin/sh\"\nargs = {route_args}\ntimeout_ms = 1000\n",
            paths.target_id(),
            paths.target_dir().join("source.json").display(),
        ),
    )
    .expect("write delivery target");
}

fn write_portable_delivery_target(
    paths: &TargetPaths,
    mode: &str,
    output: Option<&std::path::Path>,
    max_pending: usize,
    max_attempts: u32,
    conditions: &str,
) {
    let (program, args) = super::delivery::process::test_process_command(mode, output);
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
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"{}\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\n{conditions}\n\n[target]\nkind = \"file\"\nfile_path = {source_path}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n\n[outbox]\nmax_pending = {max_pending}\nmax_attempts = {max_attempts}\nbase_backoff_ms = 10\nmax_backoff_ms = 20\n\n[[routes]]\nroute_id = \"condition\"\nroute_family = \"on_condition\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = {program}\nargs = {arguments}\ntimeout_ms = 1000\n",
            paths.target_id(),
        ),
    )
    .expect("write portable delivery target");
}

fn make_pending_outbox_due(paths: &TargetPaths) {
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

fn fixture_paths() -> (tempfile::TempDir, TargetPaths) {
    let temporary = tempdir().expect("temporary directory");
    let watch_root = temporary.path().join("watchlist");
    fs::create_dir_all(&watch_root).expect("create watch root");
    let paths = TargetPaths::try_new(&watch_root, "demo").expect("target paths");
    (temporary, paths)
}

fn assert_htmlcut_preflight_permanent_episode(paths: &TargetPaths) {
    let first = run_once(paths).expect("first permanent HTMLCut preflight error");
    assert_eq!(first.outcome(), RunOutcome::ConfigInvalid);
    assert!(first.state_persisted());
    let failure = first
        .error_detail()
        .and_then(ProcessErrorDetail::htmlcut_failure)
        .cloned()
        .expect("HTMLCut preflight evidence");
    assert_eq!(failure.reason(), "plan_invalid");
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
            .and_then(ProcessErrorDetail::htmlcut_failure),
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

#[test]
fn owned_storage_without_a_state_file_loads_as_pending() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.storage_root()).expect("create owned storage");
    assert!(
        load_state(&paths)
            .expect("missing owned state is pending")
            .is_none()
    );
}

#[test]
fn reset_of_a_valid_target_without_on_run_routes_has_no_delivery_evidence() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    let report = reset(&paths).expect("reset report");
    assert!(report.delivery_outcomes().is_empty());
    assert!(report.outbox_overflow().is_empty());
    assert!(report.outbox_error().is_none());
}

#[test]
fn html_text_and_attribute_acquisition_persist_original_evidence_and_htmlcut_identity() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(
        paths.target_dir().join("source.html"),
        "<main><span class=\"price\">1.00</span></main>",
    )
    .expect("HTML source");
    write_html_target(&paths, "html_text", ".price", None, "decimal", "");

    let report = run_once(&paths).expect("HTML text run");
    let observation = report.observation().expect("HTML text observation");
    assert_eq!(observation.acquisition_kind(), AcquisitionKind::HtmlText);
    assert_eq!(observation.raw_selected(), "1.00");
    assert_eq!(observation.comparison_projection(), "1.00");
    assert_eq!(observation.canonical_value(), "1");
    assert_eq!(
        observation.htmlcut_semantics_version(),
        Some(htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION)
    );
    assert_eq!(observation.plan_digest_sha256().map(str::len), Some(64));
    assert_eq!(observation.htmlcut_candidate_count(), Some(1));
    assert!(observation.htmlcut_diagnostics().is_empty());
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("persisted HTML state"))
            .expect("state JSON");
    assert_eq!(
        state["accepted_observation"]["acquisition_kind"],
        "html_text"
    );
    assert_eq!(
        state["accepted_observation"]["htmlcut_semantics_version"],
        htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION
    );

    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(
        paths.target_dir().join("source.html"),
        "<meta id=\"price\" content=\"12.00\"><time id=\"published\" datetime=\"2026-07-15T12:34:56+02:00\">ignored</time>",
    )
    .expect("HTML source");
    write_html_target(
        &paths,
        "html_attribute",
        "meta#price",
        Some("content"),
        "decimal",
        "",
    );
    let report = run_once(&paths).expect("meta content run");
    let observation = report.observation().expect("meta content observation");
    assert_eq!(
        observation.acquisition_kind(),
        AcquisitionKind::HtmlAttribute
    );
    assert_eq!(observation.raw_selected(), "12.00");
    assert_eq!(observation.comparison_projection(), "12.00");
    assert_eq!(observation.canonical_value(), "12");

    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(
        paths.target_dir().join("source.html"),
        "<time id=\"published\" datetime=\"2026-07-15T12:34:56+02:00\">ignored</time>",
    )
    .expect("HTML source");
    write_html_target(
        &paths,
        "html_attribute",
        "time#published",
        Some("datetime"),
        "datetime",
        "[type_params]\nformat = \"rfc3339\"\n",
    );
    let report = run_once(&paths).expect("time datetime run");
    let observation = report.observation().expect("time datetime observation");
    assert_eq!(observation.raw_selected(), "2026-07-15T12:34:56+02:00");
    assert_eq!(observation.canonical_value(), "2026-07-15T10:34:56Z");
}

#[test]
fn html_text_dom_canonicalization_parses_clone_text_while_persisting_original_evidence() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(
        paths.target_dir().join("source.html"),
        "<article class=\"price\"><a href=\"/offer\">1.00</a></article>",
    )
    .expect("HTML source");
    write_html_target(&paths, "html_text", "article.price a", None, "decimal", "");
    let mut target = fs::read_to_string(paths.target_file()).expect("HTML target");
    target.push_str(
        "\n[projection.selection.dom_canonicalization]\nignore_attributes = [\"href\"]\nstrip_whitespace_nodes = false\n",
    );
    fs::write(paths.target_file(), target).expect("canonicalized HTML target");

    let report = run_once(&paths).expect("canonicalized HTML run");
    assert_eq!(report.outcome(), RunOutcome::Initialized);
    let observation = report.observation().expect("HTML observation");
    assert_eq!(observation.raw_selected(), "1.00 [/offer]");
    assert_eq!(observation.comparison_projection(), "1.00");
    assert_eq!(observation.canonical_value(), "1");
    assert_eq!(observation.htmlcut_candidate_count(), Some(1));

    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("persisted HTML state"))
            .expect("state JSON");
    assert_eq!(
        state["accepted_observation"]["raw_selected"],
        "1.00 [/offer]"
    );
    assert_eq!(
        state["accepted_observation"]["comparison_projection"],
        "1.00"
    );
}

#[test]
fn htmlcut_failures_keep_candidate_counts_and_reasons_in_the_source_health_detail() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(paths.target_dir().join("source.html"), "<main>empty</main>").expect("HTML source");
    write_html_target(&paths, "html_text", ".price", None, "integer", "");
    let report = run_once(&paths).expect("no-match run");
    assert_eq!(report.outcome(), RunOutcome::AcquisitionFailed);
    let failure = report
        .error_detail()
        .and_then(ProcessErrorDetail::htmlcut_failure)
        .expect("HTMLCut failure evidence");
    assert_eq!(failure.reason(), "NO_MATCH");
    assert_eq!(failure.candidate_count(), Some(0));
    assert_eq!(failure.plan_digest_sha256().len(), 64);
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("persisted suspect state"))
            .expect("state JSON");
    assert_eq!(state["source_health"]["reason_class"], "htmlcut_no_match");
    assert_eq!(
        state["source_health"]["last_details"]["htmlcut_failure"]["candidate_count"],
        0
    );

    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(
        paths.target_dir().join("source.html"),
        "<meta id=\"price\">",
    )
    .expect("HTML source");
    write_html_target(
        &paths,
        "html_attribute",
        "meta#price",
        Some("content"),
        "integer",
        "",
    );
    let report = run_once(&paths).expect("missing-attribute run");
    assert_eq!(report.outcome(), RunOutcome::AcquisitionFailed);
    let failure = report
        .error_detail()
        .and_then(ProcessErrorDetail::htmlcut_failure)
        .expect("missing-attribute detail");
    assert_eq!(failure.reason(), "MISSING_ATTRIBUTE");
    assert_eq!(failure.candidate_count(), Some(1));
}

#[test]
fn htmlcut_invalid_selector_begins_a_permanent_contract_error_episode() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(paths.target_dir().join("source.html"), "<main>value</main>").expect("HTML source");
    write_html_target(&paths, "html_text", "[", None, "integer", "");
    let report = run_once(&paths).expect("invalid selector run");
    assert_eq!(report.outcome(), RunOutcome::ConfigInvalid);
    let failure = report
        .error_detail()
        .and_then(ProcessErrorDetail::htmlcut_failure)
        .expect("invalid-selector detail");
    assert_eq!(failure.reason(), "INVALID_SELECTOR");
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("permanent-error state"))
            .expect("state JSON");
    assert_eq!(
        state["permanent_error_episode"]["error_code"],
        "htmlcut_invalid_selector"
    );
}

#[test]
fn htmlcut_canonicalization_preflight_rejections_retain_evidence_for_the_full_lifecycle() {
    let (_temporary, paths) = fixture_paths();
    write_html_target(&paths, "html_text", "article", None, "integer", "");
    let target = fs::read_to_string(paths.target_file()).expect("HTML text target");
    let target = target.replace(
        "kind = \"css_selector\"\nselector = \"article\"",
        "kind = \"delimiter_pair\"\nstart = \"<article>\"\nend = \"</article>\"\nmode = \"literal\"\nboundary_retention = \"exclude_both\"",
    );
    fs::write(
        paths.target_file(),
        format!(
            "{target}\n[projection.selection.dom_canonicalization]\nignore_attributes = []\nstrip_whitespace_nodes = false\n"
        ),
    )
    .expect("non-CSS canonicalized target");
    assert_htmlcut_preflight_permanent_episode(&paths);

    let (_temporary, paths) = fixture_paths();
    write_html_target(
        &paths,
        "html_attribute",
        "meta#price",
        Some("content"),
        "decimal",
        "",
    );
    let target = fs::read_to_string(paths.target_file()).expect("HTML attribute target");
    fs::write(
        paths.target_file(),
        format!(
            "{target}\n[projection.selection.dom_canonicalization]\nignore_attributes = [\"content\"]\nstrip_whitespace_nodes = false\n"
        ),
    )
    .expect("canonicalized attribute target");
    assert_htmlcut_preflight_permanent_episode(&paths);
}

#[test]
fn html_http_input_uses_the_final_response_url_as_its_htmlcut_base() {
    let target: TargetDocument = toml::from_str(
        "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"html-http\"\ndisplay_name = \"HTML HTTP\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"http\"\nsource_url = \"https://configured.example/request\"\n\n[fetch]\nengine = \"http\"\nuser_agent = \"ffhn-test\"\naccept = \"text/html\"\n\n[projection]\nkind = \"html_text\"\n\n[projection.selection.strategy]\nkind = \"css_selector\"\nselector = \"main\"\n\n[projection.selection.selection]\nmode = \"single\"\n\n[projection.selection.rendering]\nwhitespace = \"rendered\"\nrewrite_urls = true\n",
    )
    .expect("HTML HTTP target");
    target.validate().expect("valid HTML HTTP target");
    let effective =
        url::Url::parse("https://redirected.example/content/page.html").expect("effective URL");

    let input = html_input(&target, "<main>7</main>", Some(&effective)).expect("HTML input");
    assert_eq!(
        input
            .input_base_url
            .as_ref()
            .expect("HTMLCut base URL")
            .as_fetch_str(),
        effective.as_str()
    );
}

#[test]
fn http_fetch_retains_the_final_redirect_url_for_html_input_construction() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let worker = thread::spawn(move || {
        for response in [
            format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{address}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .into_bytes(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\n<main>7</main>".to_vec(),
        ] {
            let (mut stream, _) = listener.accept().expect("request");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).expect("read request");
            stream.write_all(&response).expect("response");
        }
    });
    let headers = BTreeMap::new();
    let source = fetch_http_response(
        &url::Url::parse(&format!("http://{address}/start")).expect("URL"),
        1_000,
        1024,
        "ffhn-test",
        true,
        "text/html",
        &headers,
    )
    .expect("redirected response");
    worker.join().expect("server worker");

    assert_eq!(source.body, "<main>7</main>");
    assert_eq!(
        source
            .effective_http_url
            .as_ref()
            .expect("effective response URL")
            .as_str(),
        format!("http://{address}/final")
    );
}

#[test]
fn reset_remains_blind_when_the_remaining_target_is_semantically_invalid() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    let invalid_target = fs::read_to_string(paths.target_file())
        .expect("target")
        .replacen("max_bytes = 1024", "max_bytes = 0", 1);
    fs::write(paths.target_file(), invalid_target).expect("invalid target");
    fs::create_dir_all(paths.storage_root()).expect("create storage root");
    fs::write(paths.state_file(), "malformed prior state").expect("write prior state");

    let report = reset(&paths).expect("blind reset report");

    assert!(report.storage_cleared());
    assert!(report.delivery_outcomes().is_empty());
    assert!(report.outbox_overflow().is_empty());
    assert!(report.outbox_error().is_none());
    assert!(!paths.storage_root().exists());
}

#[test]
fn reset_does_not_materialize_delivery_for_a_permanent_projection_contract_error() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "not-an-rfc-6901-pointer");
    fs::create_dir_all(paths.storage_root()).expect("create storage root");
    fs::write(paths.state_file(), "malformed prior state").expect("write prior state");

    let report = reset(&paths).expect("blind reset report");

    assert!(report.storage_cleared());
    assert!(report.delivery_outcomes().is_empty());
    assert!(report.outbox_overflow().is_empty());
    assert!(report.outbox_error().is_none());
    assert!(!paths.storage_root().exists());
}

#[test]
fn run_and_status_distinguish_semantic_target_and_target_relative_state_failures() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    let target_text = fs::read_to_string(paths.target_file()).expect("target text");
    fs::write(
        paths.target_file(),
        target_text.replacen("max_bytes = 1024", "max_bytes = 0", 1),
    )
    .expect("invalid semantic target");
    assert_eq!(
        status(&paths).expect("invalid target status").kind(),
        StatusKind::InvalidConfig
    );

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":10}"#).expect("source value");
    run_once(&paths).expect("initial persisted state");
    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("state bytes"))
            .expect("state JSON");
    state["accepted_observation"]["declared_type"] = serde_json::json!("decimal");
    state["fixed_initial_baseline"]["declared_type"] = serde_json::json!("decimal");
    fs::write(
        paths.state_file(),
        serde_json::to_vec(&state).expect("mutated state JSON"),
    )
    .expect("mutated state");
    assert_eq!(
        run_once(&paths)
            .expect("target-relative invalid state")
            .outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&paths)
            .expect("target-relative invalid state status")
            .kind(),
        StatusKind::InvalidState
    );

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    let target = load_target(&paths).expect("target");
    let empty = StateDocument::new(
        TargetId::new(paths.target_id()).expect("target id"),
        target.contract_digest_sha256().expect("digest"),
    );
    fs::create_dir_all(paths.storage_root()).expect("owned storage");
    fs::write(
        paths.state_file(),
        serde_json::to_vec(&empty).expect("empty state JSON"),
    )
    .expect("empty state");
    assert_eq!(
        status(&paths).expect("empty persisted state status").kind(),
        StatusKind::Pending
    );
}

#[cfg(unix)]
#[test]
fn source_and_permanent_failures_preserve_persist_failure_reports() {
    fn refuse_state_writes(paths: &TargetPaths) {
        fs::create_dir_all(paths.storage_root()).expect("owned storage");
        fs::set_permissions(paths.storage_root(), fs::Permissions::from_mode(0o500))
            .expect("read-only storage");
    }

    fn restore_state_writes(paths: &TargetPaths) {
        fs::set_permissions(paths.storage_root(), fs::Permissions::from_mode(0o700))
            .expect("restore storage permissions");
    }

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    refuse_state_writes(&paths);
    let report = run_once(&paths).expect("source failure report");
    restore_state_writes(&paths);
    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
    assert!(!report.state_persisted());

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "not-an-rfc6901-pointer");
    refuse_state_writes(&paths);
    let report = run_once(&paths).expect("permanent-error report");
    restore_state_writes(&paths);
    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
    assert!(!report.state_persisted());
}

#[test]
fn durable_outbox_retries_stored_bytes_terminally_fails_and_never_evicts_pending_records() {
    let (_temporary, paths) = fixture_paths();
    let source = paths.target_dir().join("source.json");
    let log = paths.target_dir().join("deliveries.jsonl");
    let changed_condition = "[[conditions]]\ncondition_id = \"changed\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"";
    write_portable_delivery_target(&paths, "write", Some(&log), 2, 2, changed_condition);
    fs::write(&source, r#"{"value":10}"#).expect("baseline");
    let initialized = run_once(&paths).expect("initialized");
    assert!(initialized.delivery_outcomes().is_empty());

    fs::write(&source, r#"{"value":20}"#).expect("first condition event");
    let delivered = run_once(&paths).expect("delivered condition event");
    assert_eq!(delivered.delivery_outcomes().len(), 1);
    assert_eq!(
        delivered.delivery_outcomes()[0].status(),
        DeliveryStatus::Delivered
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(paths.state_file()).expect("state"))
            .expect("state JSON")["outbox"],
        serde_json::json!([])
    );

    write_portable_delivery_target(&paths, "fail", None, 2, 2, changed_condition);
    fs::write(&source, r#"{"value":30}"#).expect("retryable event");
    let retried = run_once(&paths).expect("retry scheduled");
    assert_eq!(retried.delivery_outcomes().len(), 1);
    assert_eq!(
        retried.delivery_outcomes()[0].status(),
        DeliveryStatus::RetryScheduled
    );
    let retry_state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("retry state"))
            .expect("retry state JSON");
    let event_id = retry_state["outbox"][0]["event_id"]
        .as_str()
        .expect("event id")
        .to_owned();
    let stored_payload = retry_state["outbox"][0]["immutable_payload"]
        .as_array()
        .expect("payload bytes")
        .iter()
        .map(|value| {
            u8::try_from(value.as_u64().expect("byte")).expect("payload entry within byte range")
        })
        .collect::<Vec<_>>();

    make_pending_outbox_due(&paths);
    write_portable_delivery_target(&paths, "write", Some(&log), 2, 2, changed_condition);
    let retry_delivered = run_once(&paths).expect("retry delivery");
    assert_eq!(retry_delivered.delivery_outcomes().len(), 1);
    assert_eq!(
        retry_delivered.delivery_outcomes()[0].status(),
        DeliveryStatus::Delivered
    );
    assert_eq!(retry_delivered.delivery_outcomes()[0].event_id(), event_id);
    let log_lines = fs::read_to_string(&log)
        .expect("delivery log")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(log_lines.len(), 2);
    assert_eq!(log_lines[1].as_bytes(), stored_payload.as_slice());

    write_portable_delivery_target(&paths, "fail", None, 2, 1, changed_condition);
    fs::write(&source, r#"{"value":40}"#).expect("terminal event");
    let terminal = run_once(&paths).expect("terminal delivery");
    assert_eq!(terminal.delivery_outcomes().len(), 1);
    assert_eq!(
        terminal.delivery_outcomes()[0].status(),
        DeliveryStatus::DeadLettered
    );
    assert!(terminal.has_delivery_failure());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(paths.state_file()).expect("state"))
            .expect("state JSON")["outbox"],
        serde_json::json!([])
    );

    let two_conditions = "[[conditions]]\ncondition_id = \"first\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"\n\n[[conditions]]\ncondition_id = \"second\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"";
    let (_temporary, overflow_paths) = fixture_paths();
    let overflow_source = overflow_paths.target_dir().join("source.json");
    write_portable_delivery_target(&overflow_paths, "fail", None, 1, 5, two_conditions);
    fs::write(&overflow_source, r#"{"value":1}"#).expect("overflow baseline");
    run_once(&overflow_paths).expect("overflow baseline run");
    fs::write(&overflow_source, r#"{"value":2}"#).expect("overflow event");
    let overflow = run_once(&overflow_paths).expect("overflow run");
    assert_eq!(overflow.outbox_overflow().len(), 1);
    let overflow_state: serde_json::Value =
        serde_json::from_slice(&fs::read(overflow_paths.state_file()).expect("overflow state"))
            .expect("overflow JSON");
    assert_eq!(
        overflow_state["outbox"].as_array().expect("outbox").len(),
        1
    );
}

#[test]
fn lowering_max_attempts_dead_letters_an_exhausted_pending_record_without_another_process_run() {
    let (_temporary, paths) = fixture_paths();
    let source = paths.target_dir().join("source.json");
    let process_runs = paths.target_dir().join("process-runs.log");
    let condition = "[[conditions]]\ncondition_id = \"changed\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"";
    write_portable_delivery_target(&paths, "fail", None, 2, 2, condition);
    fs::write(&source, r#"{"value":1}"#).expect("baseline");
    run_once(&paths).expect("baseline");
    fs::write(&source, r#"{"value":2}"#).expect("event");
    let retry = run_once(&paths).expect("retry scheduled");
    assert_eq!(
        retry.delivery_outcomes()[0].status(),
        DeliveryStatus::RetryScheduled
    );
    assert!(!process_runs.exists());

    make_pending_outbox_due(&paths);
    write_portable_delivery_target(&paths, "fail", None, 2, 1, condition);
    let dead_letter = run_once(&paths).expect("lowered-attempt drain");
    assert_eq!(dead_letter.delivery_outcomes().len(), 1);
    assert_eq!(
        dead_letter.delivery_outcomes()[0].status(),
        DeliveryStatus::DeadLettered
    );
    assert_eq!(dead_letter.delivery_outcomes()[0].attempt_count(), 1);
    assert!(!process_runs.exists());
}

#[test]
fn decimal_observations_preserve_evidence_and_normalize_comparison() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "decimal", "", "/price");
    fs::write(paths.target_dir().join("source.json"), r#"{"price":1.00}"#).expect("write source");

    let initialized = run_once(&paths).expect("initial run");
    assert_eq!(initialized.outcome(), RunOutcome::Initialized);
    assert_eq!(
        initialized
            .observation()
            .expect("observation")
            .raw_selected(),
        "1.00"
    );
    assert_eq!(
        initialized
            .observation()
            .expect("observation")
            .canonical_value(),
        "1"
    );

    fs::write(paths.target_dir().join("source.json"), r#"{"price":1.0}"#)
        .expect("write changed presentation");
    let unchanged = run_once(&paths).expect("second run");
    assert_eq!(unchanged.outcome(), RunOutcome::Unchanged);
    assert_eq!(
        unchanged
            .observation()
            .expect("second observation")
            .raw_selected(),
        "1.0"
    );
    assert_eq!(
        unchanged
            .observation()
            .expect("second observation")
            .canonical_value(),
        "1"
    );
    assert!(paths.state_file().is_file());
}

#[test]
fn contract_change_is_refused_without_state_mutation_until_blind_reset() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/price");
    fs::write(
        paths.target_dir().join("source.json"),
        r#"{"price":7,"other":9}"#,
    )
    .expect("write source");
    run_once(&paths).expect("initialize");
    let original_state = fs::read(paths.state_file()).expect("read state bytes");

    write_target(&paths, "integer", "", "/other");
    let refused = run_once(&paths).expect("refused run");
    assert_eq!(refused.outcome(), RunOutcome::RefusedContractDigest);
    assert_eq!(
        fs::read(paths.state_file()).expect("read retained state"),
        original_state
    );

    let reset_report = reset(&paths).expect("blind reset");
    let reset_json = serde_json::to_value(reset_report).expect("reset json");
    assert_eq!(reset_json["storage_cleared"], true);
    assert!(!paths.storage_root().exists());
    assert!(paths.target_file().is_file());
    assert_eq!(
        run_once(&paths).expect("fresh v2 run").outcome(),
        RunOutcome::Initialized
    );
}

#[test]
fn reset_removes_only_the_v2_storage_root_without_inspecting_other_paths() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.storage_root()).expect("create v2 storage");
    fs::write(paths.state_file(), [0xff]).expect("write invalid v2 state");
    fs::write(paths.target_dir().join("operator-note.txt"), "keep").expect("write unrelated note");
    fs::create_dir_all(paths.target_dir().join("operator-data")).expect("create unrelated data");
    fs::write(paths.target_dir().join("operator-data/marker"), "keep")
        .expect("write unrelated marker");

    reset(&paths).expect("reset without artifact reads");
    assert!(!paths.storage_root().exists());
    assert!(paths.target_dir().join("operator-note.txt").exists());
    assert!(paths.target_dir().join("operator-data/marker").exists());
}

#[cfg(unix)]
#[test]
fn reset_stages_and_delivers_one_durable_reset_event_after_the_blind_delete() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    let log = paths.target_dir().join("reset-deliveries.jsonl");
    let route_args = format!(
        "[\"-c\", \"cat >> \\\"$1\\\"\", \"--\", {:?}]",
        log.to_string_lossy()
    );
    let target = fs::read_to_string(paths.target_file()).expect("target");
    fs::write(
        paths.target_file(),
        format!(
            "{target}\n[[routes]]\nroute_id = \"run\"\nroute_family = \"on_run\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = \"/bin/sh\"\nargs = {route_args}\ntimeout_ms = 1000\n"
        ),
    )
    .expect("add reset route");

    let report = reset(&paths).expect("reset");
    assert_eq!(
        serde_json::to_value(&report).expect("reset JSON")["storage_cleared"],
        false
    );
    assert_eq!(report.delivery_outcomes().len(), 1);
    assert_eq!(
        report.delivery_outcomes()[0].status(),
        DeliveryStatus::Delivered
    );
    let events = fs::read_to_string(log).expect("reset event");
    let event: serde_json::Value = serde_json::from_str(events.trim()).expect("reset payload");
    assert_eq!(event["event_kind"], "reset");
    assert!(paths.state_file().is_file());
}

#[test]
fn reset_reports_only_the_v2_storage_root_and_recovers_a_malformed_root_node() {
    let (_temporary, paths) = fixture_paths();
    assert_eq!(
        serde_json::to_value(reset(&paths).expect("empty reset")).expect("empty reset JSON")["storage_cleared"],
        false
    );

    fs::create_dir_all(paths.target_dir()).expect("target directory");
    let unrelated_file = paths.target_dir().join("operator-note.txt");
    fs::write(&unrelated_file, "keep").expect("unrelated file");
    assert_eq!(
        serde_json::to_value(reset(&paths).expect("unrelated reset")).expect("reset JSON")["storage_cleared"],
        false
    );
    assert!(unrelated_file.exists());

    fs::write(paths.storage_root(), "malformed storage root").expect("write malformed root");
    assert_eq!(
        serde_json::to_value(reset(&paths).expect("malformed root reset")).expect("reset JSON")["storage_cleared"],
        true
    );
    assert!(!paths.storage_root().exists());
}

#[test]
fn predecessor_schemas_are_rejected_without_migration_and_reset_remains_blind() {
    let (_temporary, target_paths) = fixture_paths();
    write_target(&target_paths, "integer", "", "/value");
    let legacy_target = fs::read_to_string(target_paths.target_file())
        .expect("target")
        .replace("schema_version = 9", "schema_version = 7");
    fs::write(target_paths.target_file(), legacy_target).expect("legacy target");
    assert_eq!(
        run_once(&target_paths)
            .expect("legacy target report")
            .outcome(),
        RunOutcome::ConfigInvalid
    );
    assert!(!target_paths.storage_root().exists());

    let (_temporary, state_paths) = fixture_paths();
    write_target(&state_paths, "integer", "", "/value");
    fs::write(
        state_paths.target_dir().join("source.json"),
        r#"{"value":7}"#,
    )
    .expect("source");
    run_once(&state_paths).expect("current state");
    let mut legacy_state: serde_json::Value =
        serde_json::from_slice(&fs::read(state_paths.state_file()).expect("state"))
            .expect("state JSON");
    legacy_state["schema_version"] = serde_json::json!(6);
    fs::write(
        state_paths.state_file(),
        serde_json::to_vec(&legacy_state).expect("legacy state JSON"),
    )
    .expect("legacy state");
    assert_eq!(
        run_once(&state_paths)
            .expect("legacy state report")
            .outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&state_paths).expect("legacy state status").kind(),
        StatusKind::InvalidState
    );
    assert_eq!(
        serde_json::to_value(reset(&state_paths).expect("blind reset")).expect("reset JSON")["storage_cleared"],
        true
    );
    assert!(!state_paths.storage_root().exists());
}

#[test]
fn source_suspect_failures_persist_health_without_advancing_the_accepted_baseline() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/nested");
    fs::write(
        paths.target_dir().join("source.json"),
        r#"{"nested":{"value":1}}"#,
    )
    .expect("write source");
    assert_eq!(
        run_once(&paths).expect("structured leaf failure").outcome(),
        RunOutcome::AcquisitionFailed
    );
    let state = load_state(&paths)
        .expect("load source-health state")
        .expect("source-health state");
    assert!(state.accepted_observation().is_none());
    assert_eq!(state.observation_seq(), 0);
    let health: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("health state"))
            .expect("health JSON");
    assert_eq!(health["source_health"]["state"], "suspect");
    assert_eq!(
        health["source_health"]["reason_class"],
        "json_non_scalar_pointer_target"
    );
    assert_eq!(health["source_health"]["consecutive_unresolved"], 1);

    assert_eq!(
        run_once(&paths)
            .expect("repeated structured leaf failure")
            .outcome(),
        RunOutcome::AcquisitionFailed
    );
    let health: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("health state"))
            .expect("health JSON");
    assert_eq!(health["source_health"]["consecutive_unresolved"], 2);

    assert_eq!(
        run_once(&paths)
            .expect("escalation-boundary structured leaf failure")
            .outcome(),
        RunOutcome::AcquisitionFailed
    );
    let health: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("health state"))
            .expect("health JSON");
    assert_eq!(health["source_health"]["consecutive_unresolved"], 3);

    fs::write(
        paths.target_dir().join("source.json"),
        r#"{"nested":"not-an-integer"}"#,
    )
    .expect("write source");
    assert_eq!(
        run_once(&paths).expect("type failure").outcome(),
        RunOutcome::ValueUnparseable
    );
    let health: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("health state"))
            .expect("health JSON");
    assert_eq!(health["source_health"]["reason_class"], "value_unparseable");
    assert_eq!(health["source_health"]["consecutive_unresolved"], 1);
    assert!(health["accepted_observation"].is_null());

    fs::write(paths.target_dir().join("source.json"), r#"{"nested":7}"#)
        .expect("write valid source");
    assert_eq!(
        run_once(&paths).expect("valid recovery").outcome(),
        RunOutcome::Initialized
    );
    let recovered: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("recovered state"))
            .expect("recovered state JSON");
    assert_eq!(recovered["source_health"]["state"], "healthy");
    assert_eq!(recovered["observation_seq"], 1);
}

#[test]
fn every_m3_source_failure_class_persists_its_closed_health_reason() {
    let cases = [
        (None, RunOutcome::FetchFailed, "fetch_failed"),
        (
            Some("not JSON"),
            RunOutcome::AcquisitionFailed,
            "json_malformed",
        ),
        (
            Some(r#"{"other":1}"#),
            RunOutcome::AcquisitionFailed,
            "json_missing_pointer_target",
        ),
        (
            Some(r#"{"value":[]}"#),
            RunOutcome::AcquisitionFailed,
            "json_non_scalar_pointer_target",
        ),
        (
            Some(r#"{"value":"not-an-integer"}"#),
            RunOutcome::ValueUnparseable,
            "value_unparseable",
        ),
    ];

    for (source_body, outcome, reason_class) in cases {
        let (_temporary, paths) = fixture_paths();
        write_target(&paths, "integer", "", "/value");
        if let Some(source_body) = source_body {
            fs::write(paths.target_dir().join("source.json"), source_body).expect("write source");
        }

        let report = run_once(&paths).expect("source failure report");
        assert_eq!(report.outcome(), outcome);
        assert!(report.state_persisted());
        let state: serde_json::Value =
            serde_json::from_slice(&fs::read(paths.state_file()).expect("source health state"))
                .expect("source health JSON");
        assert_eq!(state["accepted_observation"], serde_json::Value::Null);
        assert_eq!(state["fixed_initial_baseline"], serde_json::Value::Null);
        assert_eq!(state["observation_seq"], 0);
        assert_eq!(state["condition_state"], serde_json::json!({}));
        assert_eq!(state["source_health"]["state"], "suspect");
        assert_eq!(state["source_health"]["reason_class"], reason_class);
        assert_eq!(state["source_health"]["consecutive_unresolved"], 1);
    }
}

#[test]
fn failure_branches_preserve_preexisting_measurement_and_condition_facts() {
    let (_temporary, paths) = fixture_paths();
    let conditions = "[[conditions]]\ncondition_id = \"band\"\n\n[conditions.predicate]\nkind = \"band\"\nenter_threshold = \"10\"\nexit_threshold = \"8\"\ndirection = \"rising\"";
    write_target_with_conditions(&paths, "integer", "", "/value", conditions);
    let source = paths.target_dir().join("source.json");
    fs::write(&source, r#"{"value":10}"#).expect("write baseline source");
    assert_eq!(
        run_once(&paths).expect("initialize baseline").outcome(),
        RunOutcome::Initialized
    );
    let baseline: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("baseline state"))
            .expect("baseline JSON");

    fs::write(&source, "not JSON").expect("write malformed source");
    assert_eq!(
        run_once(&paths).expect("source-suspect run").outcome(),
        RunOutcome::AcquisitionFailed
    );
    let after_source_suspect: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("source-suspect state"))
            .expect("source-suspect JSON");
    assert_eq!(
        after_source_suspect["accepted_observation"],
        baseline["accepted_observation"]
    );
    assert_eq!(
        after_source_suspect["fixed_initial_baseline"],
        baseline["fixed_initial_baseline"]
    );
    assert_eq!(
        after_source_suspect["observation_seq"],
        baseline["observation_seq"]
    );
    assert_eq!(
        after_source_suspect["condition_state"],
        baseline["condition_state"]
    );

    fs::write(&source, r#"{"value":"not-an-integer"}"#).expect("write unparseable source");
    assert_eq!(
        run_once(&paths).expect("parse-failure run").outcome(),
        RunOutcome::ValueUnparseable
    );
    let after_parse_failure: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("parse-failure state"))
            .expect("parse-failure JSON");
    assert_eq!(
        after_parse_failure["accepted_observation"],
        baseline["accepted_observation"]
    );
    assert_eq!(
        after_parse_failure["fixed_initial_baseline"],
        baseline["fixed_initial_baseline"]
    );
    assert_eq!(
        after_parse_failure["observation_seq"],
        baseline["observation_seq"]
    );
    assert_eq!(
        after_parse_failure["condition_state"],
        baseline["condition_state"]
    );

    write_target_with_conditions(&paths, "integer", "", "not-a-json-pointer", conditions);
    let invalid_digest = load_target(&paths)
        .expect("invalid-pointer target")
        .contract_digest_sha256()
        .expect("invalid-pointer digest");
    let mut permanent_input = after_parse_failure.clone();
    permanent_input["contract_digest_sha256"] = serde_json::Value::String(invalid_digest);
    fs::write(
        paths.state_file(),
        serde_json::to_vec(&permanent_input).expect("permanent input JSON"),
    )
    .expect("write matching permanent input");

    assert_eq!(
        run_once(&paths).expect("permanent-error run").outcome(),
        RunOutcome::ConfigInvalid
    );
    let after_permanent: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("permanent state"))
            .expect("permanent JSON");
    assert_eq!(
        after_permanent["accepted_observation"],
        after_parse_failure["accepted_observation"]
    );
    assert_eq!(
        after_permanent["fixed_initial_baseline"],
        after_parse_failure["fixed_initial_baseline"]
    );
    assert_eq!(
        after_permanent["observation_seq"],
        after_parse_failure["observation_seq"]
    );
    assert_eq!(
        after_permanent["condition_state"],
        after_parse_failure["condition_state"]
    );
    assert_eq!(
        after_permanent["source_health"],
        after_parse_failure["source_health"]
    );
    assert_eq!(
        after_permanent["permanent_error_episode"]["error_code"],
        "invalid_json_pointer"
    );
}

#[test]
fn live_runtime_stages_named_conditions_against_pre_run_temporal_state() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create target directory");
    let source = paths.target_dir().join("source.json");
    let source_path = format!("{:?}", source.to_string_lossy());
    fs::write(
        paths.target_file(),
        format!(
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\n\n[[conditions]]\ncondition_id = \"changed\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"\n\n[target]\nkind = \"file\"\nfile_path = {source_path}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
        ),
    )
    .expect("write target");
    fs::write(&source, r#"{"value":10}"#).expect("write first source");

    assert_eq!(
        run_once(&paths).expect("first valid run").outcome(),
        RunOutcome::Initialized
    );
    let first: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("first state"))
            .expect("first state JSON");
    assert_eq!(first["observation_seq"], 1);
    assert_eq!(first["condition_state"]["changed"]["result"], "unavailable");
    assert_eq!(
        first["condition_state"]["changed"]["last_transition_value"]["canonical_value"],
        "10"
    );

    fs::write(&source, r#"{"value":20}"#).expect("write second source");
    assert_eq!(
        run_once(&paths).expect("second valid run").outcome(),
        RunOutcome::Changed
    );
    let second: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("second state"))
            .expect("second state JSON");
    assert_eq!(second["observation_seq"], 2);
    assert_eq!(second["condition_state"]["changed"]["result"], "satisfied");
    assert_eq!(
        second["condition_state"]["changed"]["last_transition_value"]["canonical_value"],
        "20"
    );
    assert_eq!(second["source_health"]["state"], "healthy");
}

#[test]
fn valid_issue_outcomes_persist_without_blocking_accepted_state_advancement() {
    let (_temporary, paths) = fixture_paths();
    let conditions = "[[conditions]]\ncondition_id = \"steady\"\n\n[conditions.predicate]\nkind = \"gt\"\nthreshold = \"-1\"\n\n[[conditions]]\ncondition_id = \"zero-reference\"\n\n[conditions.predicate]\nkind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"1\"";
    write_target_with_conditions(&paths, "integer", "", "/value", conditions);
    let source = paths.target_dir().join("source.json");

    fs::write(&source, r#"{"value":0}"#).expect("write zero baseline");
    assert_eq!(
        run_once(&paths).expect("first valid run").outcome(),
        RunOutcome::Initialized
    );
    let first: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("first state"))
            .expect("first state JSON");
    assert_eq!(first["observation_seq"], 1);
    assert_eq!(
        first["condition_state"]["zero-reference"]["result"],
        "unavailable"
    );
    assert_eq!(
        first["condition_state"]["steady"]["last_transition_value"]["canonical_value"],
        "0"
    );

    fs::write(&source, r#"{"value":1}"#).expect("write zero-reference value");
    assert_eq!(
        run_once(&paths).expect("zero-reference run").outcome(),
        RunOutcome::Changed
    );
    let second: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("second state"))
            .expect("second state JSON");
    assert_eq!(second["observation_seq"], 2);
    assert_eq!(second["accepted_observation"]["canonical_value"], "1");
    assert_eq!(
        second["condition_state"]["zero-reference"]["result"],
        "zero_reference"
    );
    assert_eq!(
        second["condition_state"]["zero-reference"]["last_transition_value"]["canonical_value"],
        "1"
    );
    assert_eq!(
        second["condition_state"]["steady"]["last_transition_value"],
        first["condition_state"]["steady"]["last_transition_value"]
    );
}

#[test]
fn valid_overflow_outcomes_persist_without_blocking_accepted_state_advancement() {
    let (_temporary, paths) = fixture_paths();
    let conditions = "[[conditions]]\ncondition_id = \"overflow\"\n\n[conditions.predicate]\nkind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"1\"";
    write_target_with_conditions(&paths, "integer", "", "/value", conditions);
    let source = paths.target_dir().join("source.json");

    fs::write(&source, format!(r#"{{"value":{}}}"#, i128::MIN)).expect("write minimum");
    assert_eq!(
        run_once(&paths).expect("first valid run").outcome(),
        RunOutcome::Initialized
    );
    fs::write(&source, format!(r#"{{"value":{}}}"#, i128::MAX)).expect("write maximum");
    assert_eq!(
        run_once(&paths).expect("overflow run").outcome(),
        RunOutcome::Changed
    );

    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("state")).expect("state JSON");
    assert_eq!(state["observation_seq"], 2);
    assert_eq!(
        state["accepted_observation"]["canonical_value"],
        i128::MAX.to_string()
    );
    assert_eq!(
        state["condition_state"]["overflow"]["result"],
        "arithmetic_overflow"
    );
}

#[test]
fn live_runtime_persists_level_hysteresis_between_runs() {
    let (_temporary, paths) = fixture_paths();
    let conditions = "[[conditions]]\ncondition_id = \"band\"\n\n[conditions.predicate]\nkind = \"band\"\nenter_threshold = \"10\"\nexit_threshold = \"8\"\ndirection = \"rising\"";
    write_target_with_conditions(&paths, "integer", "", "/value", conditions);
    let source = paths.target_dir().join("source.json");

    fs::write(&source, r#"{"value":10}"#).expect("write entering value");
    run_once(&paths).expect("enter band");
    let entered: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("entered state"))
            .expect("entered state JSON");
    assert_eq!(entered["condition_state"]["band"]["result"], "satisfied");
    assert_eq!(entered["condition_state"]["band"]["active"], true);

    fs::write(&source, r#"{"value":9}"#).expect("write retained value");
    run_once(&paths).expect("retain band");
    let retained: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("retained state"))
            .expect("retained state JSON");
    assert_eq!(retained["observation_seq"], 2);
    assert_eq!(retained["condition_state"]["band"]["result"], "satisfied");
    assert_eq!(retained["condition_state"]["band"]["active"], true);
    assert_eq!(
        retained["condition_state"]["band"]["last_transition_value"],
        entered["condition_state"]["band"]["last_transition_value"]
    );

    fs::write(&source, r#"{"value":7}"#).expect("write leaving value");
    run_once(&paths).expect("leave band");
    let left: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("left state"))
            .expect("left state JSON");
    assert_eq!(left["observation_seq"], 3);
    assert_eq!(left["condition_state"]["band"]["result"], "not_satisfied");
    assert_eq!(left["condition_state"]["band"]["active"], false);
}

#[test]
fn named_conditions_keep_last_transition_references_independent() {
    let (_temporary, paths) = fixture_paths();
    let conditions = "[[conditions]]\ncondition_id = \"alpha\"\n\n[conditions.predicate]\nkind = \"gt\"\nthreshold = \"0\"\n\n[[conditions]]\ncondition_id = \"beta\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_condition_transition\"";
    write_target_with_conditions(&paths, "integer", "", "/value", conditions);
    let source = paths.target_dir().join("source.json");

    fs::write(&source, r#"{"value":10}"#).expect("write first source");
    run_once(&paths).expect("first run");
    fs::write(&source, r#"{"value":20}"#).expect("write second source");
    run_once(&paths).expect("second run");
    fs::write(&source, r#"{"value":20}"#).expect("write third source");
    run_once(&paths).expect("third run");

    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("state")).expect("state JSON");
    assert_eq!(state["condition_state"]["alpha"]["result"], "satisfied");
    assert_eq!(
        state["condition_state"]["alpha"]["last_transition_value"]["canonical_value"],
        "10"
    );
    assert_eq!(state["condition_state"]["beta"]["result"], "not_satisfied");
    assert_eq!(
        state["condition_state"]["beta"]["last_transition_value"]["canonical_value"],
        "20"
    );
}

#[test]
fn permanent_json_pointer_errors_form_one_episode_without_touching_source_health() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "not-a-json-pointer");

    let first = run_once(&paths).expect("first permanent error");
    assert_eq!(first.outcome(), RunOutcome::ConfigInvalid);
    assert!(first.state_persisted());
    let first_state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("first state"))
            .expect("first state JSON");
    assert_eq!(
        first_state["permanent_error_episode"]["error_code"],
        "invalid_json_pointer"
    );
    assert_eq!(first_state["source_health"]["state"], "healthy");

    let second = run_once(&paths).expect("repeated permanent error");
    assert_eq!(second.outcome(), RunOutcome::ConfigInvalid);
    let second_state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("second state"))
            .expect("second state JSON");
    assert_eq!(
        second_state["permanent_error_episode"]["first_seen_at"],
        first_state["permanent_error_episode"]["first_seen_at"]
    );
    assert_eq!(second_state["source_health"]["state"], "healthy");
    assert!(second_state["accepted_observation"].is_null());
}

#[test]
fn root_json_pointer_selects_a_root_scalar_as_exact_evidence() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "decimal", "", "");
    fs::write(paths.target_dir().join("source.json"), "1.00").expect("write root scalar");

    let report = run_once(&paths).expect("root scalar run");
    assert_eq!(report.outcome(), RunOutcome::Initialized);
    assert_eq!(
        report.observation().expect("observation").raw_selected(),
        "1.00"
    );
    assert_eq!(
        report.observation().expect("observation").canonical_value(),
        "1"
    );
}

#[test]
fn persisted_parse_diagnostics_are_refused_without_state_mutation() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":7}"#).expect("source");
    run_once(&paths).expect("initial state");

    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("state")).expect("state JSON");
    state["accepted_observation"]["parse_diagnostics"] = serde_json::json!(["invented diagnostic"]);
    fs::write(
        paths.state_file(),
        serde_json::to_vec(&state).expect("state JSON"),
    )
    .expect("write malformed state");

    assert_eq!(
        run_once(&paths).expect("invalid state report").outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&paths).expect("invalid state status").kind(),
        StatusKind::InvalidState
    );
    assert_eq!(
        fs::read(paths.state_file()).expect("state remains untouched"),
        serde_json::to_vec(&state).expect("state JSON")
    );
}

#[cfg(unix)]
#[test]
fn normal_state_io_refuses_symlinked_storage_nodes_while_reset_remains_blind() {
    let (temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":7}"#).expect("source");
    let outside_root = temporary.path().join("outside-root");
    fs::create_dir_all(&outside_root).expect("outside root");
    let outside_state = outside_root.join("state.json");
    fs::write(&outside_state, "outside state").expect("outside state");
    std::os::unix::fs::symlink(&outside_root, paths.storage_root()).expect("storage symlink");

    assert_eq!(
        run_once(&paths)
            .expect("symlinked storage report")
            .outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&paths).expect("symlinked storage status").kind(),
        StatusKind::InvalidState
    );
    assert_eq!(
        fs::read_to_string(&outside_state).expect("outside state"),
        "outside state"
    );
    reset(&paths).expect("blind reset removes root link");
    assert_eq!(
        fs::read_to_string(&outside_state).expect("outside state survives"),
        "outside state"
    );

    run_once(&paths).expect("fresh owned storage");
    fs::remove_file(paths.state_file()).expect("remove owned state");
    let outside_file = temporary.path().join("outside-file");
    fs::write(&outside_file, "outside file").expect("outside file");
    std::os::unix::fs::symlink(&outside_file, paths.state_file()).expect("state symlink");

    assert_eq!(
        run_once(&paths).expect("symlinked state report").outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&paths).expect("symlinked state status").kind(),
        StatusKind::InvalidState
    );
    reset(&paths).expect("blind reset removes state-link root");
    assert_eq!(
        fs::read_to_string(&outside_file).expect("outside file survives"),
        "outside file"
    );
}

#[test]
fn run_status_batch_and_persistence_cover_the_v2_lifecycle() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":7}"#).expect("write source");
    assert_eq!(
        status(&paths).expect("pending status").kind(),
        StatusKind::Pending
    );
    let preview = run_once_with_mode(&paths, RunMode::DryRun).expect("dry run");
    assert_eq!(preview.outcome(), RunOutcome::Initialized);
    assert!(!preview.state_persisted());
    assert!(!paths.state_file().exists());
    assert_eq!(
        run_once(&paths).expect("initial run").outcome(),
        RunOutcome::Initialized
    );
    assert_eq!(
        status(&paths).expect("ready status").kind(),
        StatusKind::Ready
    );
    fs::write(paths.target_dir().join("source.json"), r#"{"value":8}"#).expect("change source");
    assert_eq!(
        run_once(&paths).expect("changed run").outcome(),
        RunOutcome::Changed
    );
    assert_eq!(
        run_once(&paths).expect("unchanged run").outcome(),
        RunOutcome::Unchanged
    );
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("unchanged state"))
            .expect("unchanged state JSON");
    assert_eq!(state["observation_seq"], 3);

    let second = TargetPaths::try_new(paths.watch_root(), "second").expect("second paths");
    write_target(&second, "integer", "", "/value");
    fs::write(second.target_dir().join("source.json"), r#"{"value":1}"#).expect("second source");
    let batch = run_batch_internal(vec![paths.clone(), second], RunMode::DryRun, 2).expect("batch");
    assert_eq!(batch.reports().len(), 2);
    assert!(run_batch_internal(Vec::new(), RunMode::Live, 0).is_err());

    fs::write(paths.state_file(), "not JSON").expect("invalid state");
    assert_eq!(
        run_once(&paths).expect("invalid state report").outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&paths).expect("invalid status").kind(),
        StatusKind::InvalidState
    );
    reset(&paths).expect("reset invalid state");
    fs::write(paths.target_dir().join("source.json"), "not JSON").expect("invalid JSON");
    assert_eq!(
        run_once(&paths).expect("JSON failure").outcome(),
        RunOutcome::AcquisitionFailed
    );
    fs::remove_file(paths.target_dir().join("source.json")).expect("remove source");
    assert_eq!(
        run_once(&paths).expect("missing source").outcome(),
        RunOutcome::FetchFailed
    );
}

#[test]
fn dry_runs_classify_failure_branches_without_writing_temporal_state() {
    let (_temporary, source_failure_paths) = fixture_paths();
    write_target(&source_failure_paths, "integer", "", "/value");
    fs::write(
        source_failure_paths.target_dir().join("source.json"),
        "not JSON",
    )
    .expect("write malformed source");
    let source_failure =
        run_once_with_mode(&source_failure_paths, RunMode::DryRun).expect("dry source failure");
    assert_eq!(source_failure.outcome(), RunOutcome::AcquisitionFailed);
    assert!(!source_failure.state_persisted());
    assert!(!source_failure_paths.state_file().exists());

    let (_temporary, permanent_failure_paths) = fixture_paths();
    write_target(
        &permanent_failure_paths,
        "integer",
        "",
        "not-a-json-pointer",
    );
    let permanent_failure = run_once_with_mode(&permanent_failure_paths, RunMode::DryRun)
        .expect("dry permanent failure");
    assert_eq!(permanent_failure.outcome(), RunOutcome::ConfigInvalid);
    assert!(!permanent_failure.state_persisted());
    assert!(!permanent_failure_paths.state_file().exists());
}

#[test]
fn target_load_lock_and_invalid_storage_failures_are_reported_without_hidden_fallbacks() {
    let (_temporary, paths) = fixture_paths();
    assert_eq!(
        run_once(&paths).expect("missing target report").outcome(),
        RunOutcome::TargetUnavailable
    );
    assert_eq!(
        status(&paths).expect("missing target status").kind(),
        StatusKind::UnavailableTarget
    );
    fs::create_dir_all(paths.target_dir()).expect("target directory");
    fs::write(paths.target_file(), "not TOML").expect("invalid target");
    assert_eq!(
        run_once(&paths).expect("invalid target report").outcome(),
        RunOutcome::ConfigInvalid
    );
    assert_eq!(
        status(&paths).expect("invalid target status").kind(),
        StatusKind::InvalidConfig
    );

    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":7}"#).expect("source");
    let lock = lock_exclusive(&paths).expect("hold lock");
    assert_eq!(
        run_once(&paths).expect("lock outcome").outcome(),
        RunOutcome::LockUnavailable
    );
    assert!(reset(&paths).is_err());
    drop(lock);

    fs::write(paths.storage_root(), "not a directory").expect("storage blocker");
    assert_eq!(
        run_once(&paths).expect("invalid storage outcome").outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&paths).expect("invalid storage status").kind(),
        StatusKind::InvalidState
    );
}

#[test]
fn storage_validation_rejects_non_directory_ancestors_and_non_regular_state_nodes() {
    let (_temporary, paths) = fixture_paths();
    fs::write(paths.target_dir(), "not a directory").expect("target-directory blocker");
    assert!(load_state(&paths).is_err());

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::create_dir_all(paths.storage_root()).expect("storage root");
    fs::create_dir(paths.state_file()).expect("state-directory blocker");
    assert_eq!(
        run_once(&paths).expect("state-directory report").outcome(),
        RunOutcome::StateInvalid
    );
}

#[cfg(unix)]
#[test]
fn state_io_reports_permission_failures_and_does_not_hide_persist_failure() {
    use std::os::unix::fs::PermissionsExt;

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":7}"#).expect("source");
    run_once(&paths).expect("initial committed state");
    let committed_before_failure = fs::read(paths.state_file()).expect("committed state bytes");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":8}"#).expect("changed source");
    fs::create_dir_all(paths.storage_root()).expect("storage root");
    let original_permissions = fs::metadata(paths.storage_root())
        .expect("storage metadata")
        .permissions();
    fs::set_permissions(paths.storage_root(), std::fs::Permissions::from_mode(0o500))
        .expect("make storage non-writable");
    let report = run_once(&paths).expect("persist failure report");
    fs::set_permissions(paths.storage_root(), original_permissions.clone())
        .expect("restore storage permissions");
    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
    assert_eq!(
        fs::read(paths.state_file()).expect("retained committed state"),
        committed_before_failure
    );

    fs::set_permissions(paths.storage_root(), std::fs::Permissions::from_mode(0o000))
        .expect("make storage inaccessible");
    let inaccessible = load_state(&paths);
    fs::set_permissions(paths.storage_root(), original_permissions)
        .expect("restore storage permissions");
    assert!(inaccessible.is_err());

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":7}"#).expect("source");
    let original_permissions = fs::metadata(paths.target_dir())
        .expect("target directory metadata")
        .permissions();
    fs::set_permissions(paths.target_dir(), std::fs::Permissions::from_mode(0o500))
        .expect("make target directory non-writable");
    let report = run_once(&paths).expect("storage-creation failure report");
    fs::set_permissions(paths.target_dir(), original_permissions)
        .expect("restore target directory permissions");
    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
}

#[cfg(unix)]
#[test]
fn outbox_delivery_never_runs_before_the_state_and_event_commit() {
    use std::os::unix::fs::PermissionsExt;

    let (_temporary, paths) = fixture_paths();
    let source = paths.target_dir().join("source.json");
    let log = paths.target_dir().join("commit-failure-deliveries.jsonl");
    let route_args = format!(
        "[\"-c\", \"cat >> \\\"$1\\\"\", \"--\", {:?}]",
        log.to_string_lossy()
    );
    let condition = "[[conditions]]\ncondition_id = \"changed\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"";
    write_delivery_target(&paths, &route_args, 4, 3, condition);
    fs::write(&source, r#"{"value":1}"#).expect("baseline source");
    run_once(&paths).expect("baseline state");
    let committed = fs::read(paths.state_file()).expect("committed state");
    fs::write(&source, r#"{"value":2}"#).expect("event source");

    let original = fs::metadata(paths.storage_root())
        .expect("storage metadata")
        .permissions();
    fs::set_permissions(paths.storage_root(), fs::Permissions::from_mode(0o500))
        .expect("make storage read-only");
    let report = run_once(&paths).expect("persist-failure report");
    fs::set_permissions(paths.storage_root(), original).expect("restore storage permissions");

    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
    assert!(!report.state_persisted());
    assert!(report.delivery_outcomes().is_empty());
    assert_eq!(
        fs::read(paths.state_file()).expect("retained state"),
        committed
    );
    assert!(!log.exists());
}

#[cfg(unix)]
#[test]
fn post_commit_outbox_write_failures_remain_structured_delivery_evidence() {
    use std::os::unix::fs::PermissionsExt;

    let (_temporary, paths) = fixture_paths();
    let source = paths.target_dir().join("source.json");
    let condition = "[[conditions]]\ncondition_id = \"changed\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"";
    let storage_root = paths.storage_root();
    let route_args = format!(
        "[\"-c\", \"chmod 500 \\\"$1\\\"\", \"--\", {:?}]",
        storage_root.to_string_lossy()
    );
    write_delivery_target(&paths, &route_args, 4, 3, condition);
    fs::write(&source, r#"{"value":1}"#).expect("baseline source");
    run_once(&paths).expect("baseline state");
    let original = fs::metadata(&storage_root)
        .expect("storage metadata")
        .permissions();

    fs::write(&source, r#"{"value":2}"#).expect("event source");
    let report = run_once(&paths).expect("structured post-commit failure report");
    fs::set_permissions(&storage_root, original).expect("restore storage permissions");

    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
    assert!(report.state_persisted());
    assert_eq!(report.delivery_outcomes().len(), 1);
    assert_eq!(
        report.delivery_outcomes()[0].status(),
        DeliveryStatus::DeliveredUncommitted
    );
    assert!(
        report.delivery_outcomes()[0]
            .error()
            .is_some_and(|error| error.contains("could not persist"))
    );
    assert!(report.outbox_error().is_some());
    assert!(report.has_delivery_problem());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(paths.state_file()).expect("state"))
            .expect("state JSON")["outbox"]
            .as_array()
            .expect("outbox")
            .len(),
        1
    );

    let reset_root = paths.storage_root();
    let target = fs::read_to_string(paths.target_file()).expect("target");
    let reset_args = format!(
        "[\"-c\", \"chmod 500 \\\"$1\\\"\", \"--\", {:?}]",
        reset_root.to_string_lossy()
    );
    fs::write(
        paths.target_file(),
        format!(
            "{target}\n[[routes]]\nroute_id = \"reset\"\nroute_family = \"on_run\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = \"/bin/sh\"\nargs = {reset_args}\ntimeout_ms = 1000\n"
        ),
    )
    .expect("add reset route");
    let reset = reset(&paths).expect("structured reset failure report");
    fs::set_permissions(&reset_root, fs::Permissions::from_mode(0o700))
        .expect("restore reset storage permissions");

    assert_eq!(reset.delivery_outcomes().len(), 1);
    assert_eq!(
        reset.delivery_outcomes()[0].status(),
        DeliveryStatus::DeliveredUncommitted
    );
    assert!(reset.outbox_error().is_some());
    assert!(reset.has_delivery_problem());
}

#[cfg(unix)]
#[test]
fn non_lockable_lock_nodes_surface_io_errors() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.lock_root()).expect("lock root");
    let status = std::process::Command::new("mkfifo")
        .arg(paths.run_lock_file())
        .status()
        .expect("invoke mkfifo");
    assert!(status.success());

    assert!(matches!(lock_exclusive(&paths), Err(LockError::Io(_))));
    assert!(lock_shared(&paths).is_err());
}

#[test]
fn direct_acquisition_fetch_and_error_helpers_cover_scalar_transport_boundaries() {
    let source_path = crate::test_support::absolute_file_path("source.json");
    let document = toml::from_str::<TargetDocument>(&format!(
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
        ))
        .expect("target");
    assert_eq!(
        acquire_json_scalar(&document, r#"{"value":"text"}"#).expect("string"),
        r#""text""#
    );
    assert_eq!(
        acquire_json_scalar(&document, r#"{"value":"1.2.3\u002bbuild.7"}"#)
            .expect("escaped string"),
        r#""1.2.3\u002bbuild.7""#
    );
    assert_eq!(
        acquire_json_scalar(&document, r#"{"value":7}"#).expect("number"),
        "7"
    );
    assert_eq!(
        acquire_json_scalar(&document, r#"{"value":true}"#).expect("boolean"),
        "true"
    );
    assert_eq!(
        acquire_json_scalar(&document, r#"{"value":null}"#).expect("null"),
        "null"
    );
    assert!(acquire_json_scalar(&document, r#"{"value":[]}"#).is_err());
    assert!(acquire_json_scalar(&document, r#"{"other":1}"#).is_err());
    assert!(acquire_json_scalar(&document, "not JSON").is_err());

    let temporary = tempdir().expect("temporary directory");
    let file = temporary.path().join("source");
    fs::write(&file, "123").expect("source");
    assert_eq!(
        read_file_source(&file.to_string_lossy(), 3).expect("file"),
        "123"
    );
    assert!(read_file_source(&file.to_string_lossy(), 2).is_err());
    fs::write(&file, [0xff]).expect("invalid UTF-8");
    assert!(read_file_source(&file.to_string_lossy(), 10).is_err());
    assert!(read_file_source("/does/not/exist", 10).is_err());
    assert!(read_file_source(&temporary.path().to_string_lossy(), 10).is_err());
    let mismatched: TargetDocument = toml::from_str(&format!(
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"http\"\nuser_agent = \"test\"\naccept = \"application/json\"\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
        ))
        .expect("mismatched target");
    assert!(fetch_source(&mismatched).is_err());
    let http: TargetDocument = toml::from_str(
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"http\"\nsource_url = \"http://127.0.0.1:1/value\"\n\n[fetch]\nengine = \"http\"\nuser_agent = \"ffhn-test\"\naccept = \"application/json\"\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
        )
        .expect("HTTP target");
    assert!(fetch_source(&http).is_err());

    let json_error = serde_json::from_str::<serde_json::Value>("not JSON").expect_err("JSON error");
    assert_eq!(
        detail_from_error(&CoreError::Json(json_error), None).kind(),
        ProcessErrorKind::Json
    );
    let toml_error = toml::from_str::<TargetDocument>("not TOML").expect_err("TOML error");
    assert_eq!(
        detail_from_error(&CoreError::Toml(toml_error), None).kind(),
        ProcessErrorKind::Toml
    );
    assert_eq!(
        detail_from_error(&CoreError::contract("bad"), None).kind(),
        ProcessErrorKind::Contract
    );
}

#[test]
fn http_fetch_handles_success_and_transport_failure_statuses() {
    assert_eq!(
        fetch_http_from_once("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}", 10)
            .expect("HTTP body"),
        "{}"
    );
    assert!(
        fetch_http_from_once(
            "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n",
            10
        )
        .is_err()
    );
    assert!(fetch_http_from_once("HTTP/1.1 200 OK\r\nContent-Length: 99\r\n\r\n", 10).is_err());
    assert!(fetch_http_from_once("HTTP/1.1 200 OK\r\n\r\n01234567890", 10).is_err());
    assert!(fetch_http_from_once("HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\n{}", 10).is_err());
    assert!(
        fetch_http_bytes_from_once(
            b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n\xff".to_vec(),
            10
        )
        .is_err()
    );
}

#[test]
fn v2_runtime_handles_disabled_contract_mismatch_and_owned_lock_boundaries() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(
        paths.target_dir().join("source.json"),
        r#"{"value":7,"other":8}"#,
    )
    .expect("source");
    run_once(&paths).expect("initial state");

    let disabled_target = fs::read_to_string(paths.target_file())
        .expect("target")
        .replace("enabled = true", "enabled = false");
    fs::write(paths.target_file(), disabled_target).expect("disabled target");
    let disabled_digest = load_target(&paths)
        .expect("disabled target loads")
        .contract_digest_sha256()
        .expect("disabled digest");
    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("state")).expect("JSON");
    state["contract_digest_sha256"] = serde_json::Value::String(disabled_digest);
    fs::write(
        paths.state_file(),
        serde_json::to_vec(&state).expect("state JSON"),
    )
    .expect("matching disabled state");
    let skipped = run_once(&paths).expect("disabled run");
    assert_eq!(skipped.outcome(), RunOutcome::SkippedDisabled);
    assert_eq!(
        serde_json::to_value(skipped).expect("skipped report")["previous_canonical_value"],
        "7"
    );

    write_target(&paths, "integer", "", "/other");
    assert_eq!(
        status(&paths).expect("contract mismatch status").kind(),
        StatusKind::InvalidState
    );

    let (_temporary, mismatched_target_paths) = fixture_paths();
    write_target(&mismatched_target_paths, "integer", "", "/value");
    fs::write(
        mismatched_target_paths.target_dir().join("source.json"),
        r#"{"value":1}"#,
    )
    .expect("source");
    let mismatched_target = fs::read_to_string(mismatched_target_paths.target_file())
        .expect("target")
        .replace("target_id = \"demo\"", "target_id = \"other\"");
    fs::write(mismatched_target_paths.target_file(), mismatched_target).expect("bad target id");
    assert_eq!(
        run_once(&mismatched_target_paths)
            .expect("target mismatch report")
            .outcome(),
        RunOutcome::ConfigInvalid
    );
    assert_eq!(
        status(&mismatched_target_paths)
            .expect("target mismatch status")
            .kind(),
        StatusKind::InvalidConfig
    );

    let (_temporary, mismatched_state_paths) = fixture_paths();
    write_target(&mismatched_state_paths, "integer", "", "/value");
    fs::write(
        mismatched_state_paths.target_dir().join("source.json"),
        r#"{"value":1}"#,
    )
    .expect("source");
    run_once(&mismatched_state_paths).expect("state");
    let mut wrong_state: serde_json::Value =
        serde_json::from_slice(&fs::read(mismatched_state_paths.state_file()).expect("state JSON"))
            .expect("state document");
    wrong_state["target_id"] = serde_json::Value::String("other".to_owned());
    fs::write(
        mismatched_state_paths.state_file(),
        serde_json::to_vec(&wrong_state).expect("wrong state JSON"),
    )
    .expect("wrong state");
    assert_eq!(
        run_once(&mismatched_state_paths)
            .expect("state mismatch report")
            .outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&mismatched_state_paths)
            .expect("state mismatch status")
            .kind(),
        StatusKind::InvalidState
    );

    let (_temporary, lock_paths) = fixture_paths();
    write_target(&lock_paths, "integer", "", "/value");
    fs::write(
        lock_paths.target_dir().join("source.json"),
        r#"{"value":1}"#,
    )
    .expect("source");
    fs::write(
        lock_paths.watch_root().join(".ffhn-locks"),
        "not a directory",
    )
    .expect("lock parent blocker");
    assert!(run_once(&lock_paths).is_err());
    assert!(status(&lock_paths).is_err());
    assert!(reset(&lock_paths).is_err());

    let temporary = tempdir().expect("temporary directory");
    let parent_file = temporary.path().join("parent-file");
    fs::write(&parent_file, "not a directory").expect("parent file");
    assert!(blind_remove_storage_root(&parent_file.join("child")).is_err());
}

#[test]
fn shared_status_lock_waits_for_a_live_run_lock_to_be_released() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":1}"#).expect("source");
    let (ready_sender, ready_receiver) = mpsc::channel();
    let lock_paths = paths.clone();
    let worker = thread::spawn(move || {
        let lock = lock_exclusive(&lock_paths).expect("exclusive lock");
        ready_sender.send(()).expect("ready");
        thread::sleep(Duration::from_millis(20));
        drop(lock);
    });
    ready_receiver.recv().expect("lock acquired");
    assert_eq!(
        status(&paths).expect("waited status").kind(),
        StatusKind::Pending
    );
    worker.join().expect("worker");
}

fn fetch_http_from_once(response: &str, max_bytes: usize) -> Result<String, ProcessErrorDetail> {
    fetch_http_bytes_from_once(response.as_bytes().to_vec(), max_bytes)
}

fn fetch_http_bytes_from_once(
    response: Vec<u8>,
    max_bytes: usize,
) -> Result<String, ProcessErrorDetail> {
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

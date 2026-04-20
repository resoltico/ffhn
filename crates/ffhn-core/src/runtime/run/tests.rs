use super::super::lock::{try_lock_exclusive, with_exclusive_lock_error_injected};
use super::super::storage::{read_json, write_exact_text, write_json, write_text};
use super::change::{common_suffix_len, excerpt_from_lines, split_lines};
use super::notifications::{
    NotificationProcess, deliver_notification, notification_event, wait_for_notification_process,
    write_child_notification_payload_or_failure, write_notification_payload_or_failure,
};
use super::*;
use crate::stable_json::sha256_hex;
use crate::{
    ChangeKind, CompareBasis, CompareConfig, EXTRACTION_RECORD_SCHEMA_NAME,
    EXTRACTION_RECORD_SCHEMA_VERSION, ExtractionRecord, FetchConfig, FetchEngine,
    HTMLCUT_INTEROP_PROFILE, HttpMethod, NotificationEvent, NotificationHook, OutputKind,
    ReasonCode, RunChangeSection, RunFetchSection, SelectionConfig, SelectionKind, SelectionMatch,
    SnapshotReference, SnapshotSlot, StatePhase, TargetDocument, TargetSource, WhitespaceMode,
};
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tempfile::tempdir;
use url::Url;

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct TestResponse {
    status_line: &'static str,
    content_type: &'static str,
    body: &'static str,
}

struct BrokenWriter;

impl Write for BrokenWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("broken writer"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("broken writer"))
    }
}

struct FakeNotificationProcess {
    polls: Vec<std::io::Result<Option<ExitStatus>>>,
    killed: usize,
    waited: usize,
}

impl NotificationProcess for FakeNotificationProcess {
    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        self.polls.remove(0)
    }

    fn kill(&mut self) -> std::io::Result<()> {
        self.killed += 1;
        Ok(())
    }

    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.waited += 1;
        Ok(exit_status(0))
    }
}

#[cfg(unix)]
fn exit_status(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code << 8)
}

fn noop_abort() {}

fn live_success_report(target_id: &str) -> RunReport {
    RunReport {
        schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: RUN_REPORT_SCHEMA_VERSION,
        run_report_digest_sha256: String::new(),
        target_id: target_id.to_owned(),
        run_started_at: "2026-04-05T10:15:30Z".to_owned(),
        run_finished_at: "2026-04-05T10:15:31Z".to_owned(),
        run_mode: RunMode::Live,
        run_outcome: RunOutcome::Changed,
        reason_code: ReasonCode::Ok,
        failure_class: None,
        target_status_after_run: TargetStatus::Ready,
        compare_basis: CompareBasis::CanonicalTextSha256,
        previous_compare_digest_sha256: Some(DIGEST.to_owned()),
        current_compare_digest_sha256: Some(DIGEST.to_owned()),
        state_phase_before_run: StatePhase::HasBaseline,
        state_phase_after_run: StatePhase::HasBaseline,
        fetch: Some(RunFetchSection {
            engine: FetchEngine::Http,
            final_url: Some("https://example.com/final".to_owned()),
            http_status: Some(200),
            content_type: Some("text/html".to_owned()),
            bytes_read: Some(42),
            duration_ms: 12,
        }),
        extraction: Some(RunExtractionSection {
            interop_profile: HTMLCUT_INTEROP_PROFILE.to_owned(),
            htmlcut_plan_digest_sha256: DIGEST.to_owned(),
            htmlcut_result_digest_sha256: DIGEST.to_owned(),
            comparison_input_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            strategy_kind: SelectionKind::CssSelector,
            selection_mode: SelectionMatch::Single,
            output_kind: OutputKind::OuterHtml,
            candidate_count: 1,
            selected_candidate_index: 1,
            warning_codes: Vec::new(),
            duration_ms: 8,
        }),
        compare: Some(RunCompareSection {
            canonicalizers: vec!["trim".to_owned()],
            duration_ms: 3,
        }),
        change: Some(RunChangeSection {
            kind: ChangeKind::Changed,
            previous_text_bytes: Some(6),
            current_text_bytes: 7,
            previous_line_count: Some(1),
            current_line_count: 1,
            common_prefix_lines: 0,
            common_suffix_lines: 0,
            changed_region: None,
        }),
        persist: RunPersistSection {
            duration_ms: 2,
            wrote_state: true,
            wrote_last_run: false,
        },
        notifications: Vec::new(),
        extensions: None,
    }
    .with_digest()
    .expect("report digest")
}

fn serve_once(response: TestResponse) -> (Url, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind server");
    let address = listener.local_addr().expect("server addr");
    let url = Url::parse(&format!("http://{address}")).expect("server url");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept connection");
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request);
        let raw = format!(
            "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n{}",
            response.status_line,
            response.content_type,
            response.body.len(),
            response.body
        );
        let _ = stream.write_all(raw.as_bytes());
    });
    (url, handle)
}

fn serve_once_with_delay(
    response: TestResponse,
    delay_before_response_ms: u64,
) -> (Url, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind server");
    let address = listener.local_addr().expect("server addr");
    let url = Url::parse(&format!("http://{address}")).expect("server url");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept connection");
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request);
        thread::sleep(Duration::from_millis(delay_before_response_ms));
        let raw = format!(
            "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n{}",
            response.status_line,
            response.content_type,
            response.body.len(),
            response.body
        );
        let _ = stream.write_all(raw.as_bytes());
    });
    (url, handle)
}

fn serve_once_with_accept_signal(
    response: TestResponse,
    delay_before_response_ms: u64,
) -> (Url, mpsc::Receiver<()>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind server");
    let address = listener.local_addr().expect("server addr");
    let url = Url::parse(&format!("http://{address}")).expect("server url");
    let (accepted_tx, accepted_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept connection");
        accepted_tx.send(()).expect("signal accepted connection");
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request);
        thread::sleep(Duration::from_millis(delay_before_response_ms));
        let raw = format!(
            "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n{}",
            response.status_line,
            response.content_type,
            response.body.len(),
            response.body
        );
        let _ = stream.write_all(raw.as_bytes());
    });
    (url, accepted_rx, handle)
}

fn target_document(
    target_id: &str,
    enabled: bool,
    source_url: Url,
    selector: &str,
    selection_match: SelectionMatch,
) -> TargetDocument {
    TargetDocument {
        schema_name: crate::TARGET_SCHEMA_NAME.to_owned(),
        schema_version: crate::TARGET_SCHEMA_VERSION,
        target_id: target_id.to_owned(),
        display_name: "Demo".to_owned(),
        enabled,
        target: TargetSource {
            kind: crate::model::TargetKind::Http,
            source_url: Some(source_url),
            file_path: None,
        },
        fetch: FetchConfig {
            engine: FetchEngine::Http,
            method: HttpMethod::GET,
            timeout_ms: 15_000,
            max_bytes: 2_000_000,
            user_agent: "ffhn/2.0.0".to_owned(),
            follow_redirects: true,
            accept: "text/html".to_owned(),
            headers: Default::default(),
            extensions: None,
        },
        selection: SelectionConfig {
            kind: SelectionKind::CssSelector,
            r#match: selection_match,
            index: None,
            output: OutputKind::OuterHtml,
            whitespace: WhitespaceMode::Normalize,
            rewrite_urls: false,
            selector: Some(selector.to_owned()),
            start: None,
            end: None,
            mode: None,
            include_start: None,
            include_end: None,
            flags: Vec::new(),
        },
        compare: CompareConfig {
            basis: CompareBasis::CanonicalTextSha256,
            canonicalization: Vec::new(),
        },
        storage: Default::default(),
        notifications: Vec::new(),
        extensions: None,
    }
}

fn write_target(paths: &TargetPaths, target: &TargetDocument) {
    write_text(
        paths.target_file(),
        &toml::to_string(target).expect("target toml"),
    )
    .expect("write target");
}

fn snapshot_reference(
    slot: SnapshotSlot,
    name: &str,
    canonical: &str,
    outer: &str,
) -> SnapshotReference {
    SnapshotReference {
        slot,
        canonical_text_sha256: sha256_hex(canonical.as_bytes()),
        outer_html_sha256: sha256_hex(outer.as_bytes()),
        extraction_record_path: format!("snapshots/{name}/extraction.json"),
        canonical_text_path: format!("snapshots/{name}/canonical.txt"),
        outer_html_path: format!("snapshots/{name}/outer.html"),
        captured_at: "2026-04-05T10:15:30Z".to_owned(),
    }
}

fn write_snapshot_state(paths: &TargetPaths, canonical: &str, outer: &str) {
    let reference = snapshot_reference(SnapshotSlot::Current, "current", canonical, outer);
    write_exact_text(
        paths.target_dir().join(&reference.canonical_text_path),
        canonical,
    )
    .expect("write canonical");
    write_exact_text(paths.target_dir().join(&reference.outer_html_path), outer)
        .expect("write outer");
    write_json(
        paths.target_dir().join(&reference.extraction_record_path),
        &ExtractionRecord {
            schema_name: EXTRACTION_RECORD_SCHEMA_NAME.to_owned(),
            schema_version: EXTRACTION_RECORD_SCHEMA_VERSION,
            interop_profile: HTMLCUT_INTEROP_PROFILE.to_owned(),
            htmlcut_plan_digest_sha256: DIGEST.to_owned(),
            htmlcut_result_digest_sha256: DIGEST.to_owned(),
            comparison_input_sha256: DIGEST.to_owned(),
            outer_html_sha256: reference.outer_html_sha256.clone(),
            strategy_kind: SelectionKind::CssSelector,
            selection_mode: SelectionMatch::Single,
            output_kind: OutputKind::OuterHtml,
            candidate_count: 1,
            selected_candidate_index: 1,
            match_metadata: json!({"selector": "main"}),
            warning_codes: Vec::new(),
            created_at: "2026-04-05T10:15:30Z".to_owned(),
            extensions: None,
        },
    )
    .expect("write extraction");
    write_json(
        paths.state_file(),
        &crate::StateDocument {
            schema_name: crate::STATE_SCHEMA_NAME.to_owned(),
            schema_version: crate::STATE_SCHEMA_VERSION,
            target_id: paths.target_id().to_owned(),
            state_phase: StatePhase::HasBaseline,
            last_run_at: Some("2026-04-05T10:15:30Z".to_owned()),
            last_run_outcome: Some(RunOutcome::Initialized),
            last_reason_code: Some(ReasonCode::Ok),
            current_snapshot: Some(reference),
            snapshot_history: Vec::new(),
            extensions: None,
        },
    )
    .expect("write state");
}

#[test]
fn helper_functions_cover_error_mapping_and_compare_outcomes() {
    assert_eq!(
        reason_code_for_htmlcut_error(ErrorCode::PlanInvalid),
        ReasonCode::ExtractionPlanInvalid
    );
    assert_eq!(
        reason_code_for_htmlcut_error(ErrorCode::NoMatch),
        ReasonCode::ExtractionNoMatch
    );
    assert_eq!(
        reason_code_for_htmlcut_error(ErrorCode::AmbiguousMatch),
        ReasonCode::ExtractionAmbiguousMatch
    );
    assert_eq!(
        reason_code_for_htmlcut_error(ErrorCode::InternalError),
        ReasonCode::ExtractionInternalError
    );

    assert_eq!(
        run_outcome_from_digests(None, DIGEST),
        RunOutcome::Initialized
    );
    assert_eq!(
        run_outcome_from_digests(Some(DIGEST), DIGEST),
        RunOutcome::Unchanged
    );
    assert_eq!(
        run_outcome_from_digests(
            Some(DIGEST),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        ),
        RunOutcome::Changed
    );
    assert_eq!(
        failure_run_outcome(ReasonCode::FetchTimeout),
        RunOutcome::FailedTransient
    );
    assert_eq!(
        failure_run_outcome(ReasonCode::Disabled),
        RunOutcome::FailedPermanent
    );
    assert_eq!(
        notification_event(RunOutcome::SkippedDisabled),
        NotificationEvent::SkippedDisabled
    );

    let changed = build_change_section(
        Some("keep\nbefore\nsuffix"),
        "keep\nafter\nsuffix",
        RunOutcome::Changed,
    );
    assert_eq!(changed.kind, ChangeKind::Changed);
    assert_eq!(changed.common_prefix_lines, 1);
    assert_eq!(changed.common_suffix_lines, 1);
    assert_eq!(
        changed
            .changed_region
            .as_ref()
            .expect("changed region")
            .current_excerpt
            .as_deref(),
        Some("after")
    );

    let unchanged = build_change_section(Some("same"), "same", RunOutcome::SkippedDisabled);
    assert_eq!(unchanged.kind, ChangeKind::Unchanged);
    assert!(unchanged.changed_region.is_none());
    assert!(split_lines("").is_empty());
    assert_eq!(common_suffix_len(&["a", "b"], &["x", "b"], 0), 1);
    assert_eq!(
        excerpt_from_lines(&["1", "2", "3", "4", "5"]).as_deref(),
        Some("1\n2\n3\n4\n...")
    );
    let long_line = "x".repeat(300);
    let truncated = excerpt_from_lines(&[long_line.as_str()]);
    assert!(truncated.expect("truncated excerpt").ends_with("..."));
}

#[test]
fn notification_helpers_cover_payload_failures_wait_paths_and_panics() {
    noop_abort();
    assert!(
        write_notification_payload_or_failure::<BrokenWriter>(
            "notify",
            NotificationEvent::Changed,
            Instant::now(),
            None,
            "payload",
            noop_abort,
        )
        .is_none()
    );

    let payload_failure = write_notification_payload_or_failure(
        "notify",
        NotificationEvent::Changed,
        Instant::now(),
        Some(BrokenWriter),
        "payload",
        || {},
    )
    .expect("payload failure");
    assert!(!payload_failure.delivered);
    assert!(payload_failure.error.is_some());
    assert!(BrokenWriter.flush().is_err());

    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg("exec 0<&-; sleep 5")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("notification helper child");
    thread::sleep(Duration::from_millis(100));
    let child_payload_failure = write_child_notification_payload_or_failure(
        "notify",
        NotificationEvent::Changed,
        Instant::now(),
        &mut child,
        &"payload".repeat(16_384),
    )
    .expect("child payload failure");
    assert!(!child_payload_failure.delivered);
    assert!(child_payload_failure.error.is_some());
    assert!(child.try_wait().expect("child try_wait").is_some());

    let success = wait_for_notification_process(
        &mut FakeNotificationProcess {
            polls: vec![Ok(Some(exit_status(0)))],
            killed: 0,
            waited: 0,
        },
        "notify",
        NotificationEvent::Changed,
        Instant::now(),
        50,
    );
    assert!(success.delivered);

    let failure = wait_for_notification_process(
        &mut FakeNotificationProcess {
            polls: vec![Ok(Some(exit_status(7)))],
            killed: 0,
            waited: 0,
        },
        "notify",
        NotificationEvent::Changed,
        Instant::now(),
        50,
    );
    assert_eq!(failure.exit_code, Some(7));
    assert!(!failure.delivered);

    let mut timed_out_process = FakeNotificationProcess {
        polls: vec![Ok(None)],
        killed: 0,
        waited: 0,
    };
    let timed_out = wait_for_notification_process(
        &mut timed_out_process,
        "notify",
        NotificationEvent::Changed,
        Instant::now() - Duration::from_millis(100),
        10,
    );
    assert!(timed_out.timed_out);
    assert_eq!(timed_out_process.killed, 1);
    assert_eq!(timed_out_process.waited, 1);

    let wait_error = wait_for_notification_process(
        &mut FakeNotificationProcess {
            polls: vec![Err(std::io::Error::other("wait failed"))],
            killed: 0,
            waited: 0,
        },
        "notify",
        NotificationEvent::Changed,
        Instant::now(),
        50,
    );
    assert!(
        wait_error
            .error
            .expect("wait error")
            .contains("wait failed")
    );

    let panic = join_batch_handle(thread::spawn(|| panic!("boom")));
    assert!(panic.is_err());
}

#[test]
fn required_outer_html_enforces_the_persisted_artifact_contract() {
    let url = Url::parse("https://example.com").expect("url");
    let target = target_document("demo", true, url.clone(), "main", SelectionMatch::Single);
    let plan = super::super::interop::build_htmlcut_plan(&target).expect("plan");
    let source = HtmlInput::new("demo".to_owned(), "<main>Hello</main>".to_owned())
        .expect("source")
        .with_input_base_url(url);

    let mut result = execute_plan(&source, &plan).expect("result");
    assert_eq!(
        required_outer_html(&result).expect("outer html"),
        "<main>Hello</main>"
    );

    result.selected_match.outer_html = None;
    assert!(required_outer_html(&result).is_err());
}

#[test]
fn run_batch_covers_all_outcome_buckets_and_fatal_errors() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path();

    let initialized_paths = TargetPaths::new(watch_root, "initialized");
    let (url, initialized_handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Init</main></body></html>",
    });
    write_target(
        &initialized_paths,
        &target_document("initialized", true, url, "main", SelectionMatch::Single),
    );

    let changed_paths = TargetPaths::new(watch_root, "changed");
    let (url, changed_handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>After</main></body></html>",
    });
    write_target(
        &changed_paths,
        &target_document("changed", true, url, "main", SelectionMatch::Single),
    );
    write_snapshot_state(&changed_paths, "before", "<main>Before</main>");

    let unchanged_paths = TargetPaths::new(watch_root, "unchanged");
    let (url, unchanged_handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Same</main></body></html>",
    });
    write_target(
        &unchanged_paths,
        &target_document("unchanged", true, url, "main", SelectionMatch::Single),
    );
    write_snapshot_state(&unchanged_paths, "Same", "<main>Same</main>");

    let transient_paths = TargetPaths::new(watch_root, "transient");
    let (url, transient_handle) = serve_once(TestResponse {
        status_line: "500 Internal Server Error",
        content_type: "text/html",
        body: "boom",
    });
    write_target(
        &transient_paths,
        &target_document("transient", true, url, "main", SelectionMatch::Single),
    );

    let permanent_paths = TargetPaths::new(watch_root, "permanent");
    write_target(
        &permanent_paths,
        &target_document(
            "other",
            true,
            Url::parse("https://example.com").expect("url"),
            "main",
            SelectionMatch::Single,
        ),
    );

    let skipped_paths = TargetPaths::new(watch_root, "skipped");
    write_target(
        &skipped_paths,
        &target_document(
            "skipped",
            false,
            Url::parse("https://example.com").expect("url"),
            "main",
            SelectionMatch::Single,
        ),
    );

    let fatal_paths = TargetPaths::new(watch_root, "fatal");
    write_target(
        &fatal_paths,
        &target_document(
            "fatal",
            true,
            Url::parse("https://example.com").expect("url"),
            "main",
            SelectionMatch::Single,
        ),
    );
    std::fs::write(fatal_paths.lock_dir(), "blocked").expect("block fatal lock path");

    let targets = vec![
        "initialized".to_owned(),
        "changed".to_owned(),
        "unchanged".to_owned(),
        "transient".to_owned(),
        "permanent".to_owned(),
        "skipped".to_owned(),
        "fatal".to_owned(),
    ];
    let report = run_batch(watch_root, &targets, RunOptions::LIVE, 3).expect("batch report");

    initialized_handle.join().expect("initialized join");
    changed_handle.join().expect("changed join");
    unchanged_handle.join().expect("unchanged join");
    transient_handle.join().expect("transient join");

    assert_eq!(report.requested_targets, targets);
    assert_eq!(report.outcome_counts.initialized, 1);
    assert_eq!(report.outcome_counts.changed, 1);
    assert_eq!(report.outcome_counts.unchanged, 1);
    assert_eq!(report.outcome_counts.failed_transient, 1);
    assert_eq!(report.outcome_counts.failed_permanent, 1, "{report:?}");
    assert_eq!(report.outcome_counts.skipped_disabled, 1);
    assert_eq!(report.outcome_counts.fatal_error, 1);
    let fatal_entry = report
        .entries
        .iter()
        .find(|entry| entry.target_id == "fatal")
        .expect("fatal entry");
    assert!(fatal_entry.run_report.is_none());
    assert!(
        fatal_entry
            .fatal_error
            .as_deref()
            .is_some_and(|error| error.contains("filesystem error"))
    );
}

#[test]
fn run_batch_rejects_zero_concurrency_and_duplicate_targets() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path();
    let paths = TargetPaths::new(watch_root, "demo");
    write_target(
        &paths,
        &target_document(
            "demo",
            true,
            Url::parse("https://example.com").expect("url"),
            "main",
            SelectionMatch::Single,
        ),
    );

    assert!(run_batch(watch_root, &["demo".to_owned()], RunOptions::LIVE, 0).is_err());
    assert!(
        run_batch(
            watch_root,
            &["demo".to_owned(), "demo".to_owned()],
            RunOptions::LIVE,
            1
        )
        .is_err()
    );
}

#[test]
fn run_once_dry_run_skips_live_only_state_failures_and_persistence() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let source_url = Url::parse("https://example.com").expect("url");

    write_target(
        &paths,
        &target_document(
            "demo",
            true,
            source_url.clone(),
            "main",
            SelectionMatch::Single,
        ),
    );
    write_json(
        paths.state_file(),
        &crate::StateDocument {
            schema_name: "wrong".to_owned(),
            schema_version: crate::STATE_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            state_phase: StatePhase::HasBaseline,
            last_run_at: None,
            last_run_outcome: None,
            last_reason_code: None,
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        },
    )
    .expect("write invalid state");
    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once_with_options(&paths, RunOptions::DRY_RUN).expect("dry run");
    handle.join().expect("dry-run join");
    assert_eq!(report.run_mode, RunMode::DryRun);
    assert_eq!(report.run_outcome, RunOutcome::Initialized);
    assert!(!report.persist.wrote_state);
    assert!(!paths.last_run_file().exists());

    write_target(
        &paths,
        &target_document("demo", true, source_url, "main", SelectionMatch::Single),
    );
    write_snapshot_state(&paths, "hello", "<main>Hello</main>");
    write_exact_text(
        paths.target_dir().join("snapshots/current/outer.html"),
        "<main>Tampered</main>",
    )
    .expect("tamper outer html");
    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let integrity_dry_run = run_once_with_options(&paths, RunOptions::DRY_RUN)
        .expect("dry run with integrity mismatch");
    handle.join().expect("integrity join");
    assert_eq!(integrity_dry_run.run_outcome, RunOutcome::Initialized);

    let (url, handle) = serve_once(TestResponse {
        status_line: "500 Internal Server Error",
        content_type: "text/html",
        body: "boom",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let fetch_failure =
        run_once_with_options(&paths, RunOptions::DRY_RUN).expect("dry-run fetch failure");
    handle.join().expect("fetch failure join");
    assert_eq!(fetch_failure.run_outcome, RunOutcome::FailedTransient);
    assert!(!fetch_failure.persist.wrote_state);

    #[cfg(unix)]
    {
        write_snapshot_state(&paths, "before", "<main>Before</main>");
        let metadata = std::fs::metadata(paths.state_file()).expect("state metadata");
        let original = metadata.permissions();
        let mut denied = original.clone();
        denied.set_mode(0o000);
        std::fs::set_permissions(paths.state_file(), denied).expect("deny state permissions");
        let (url, handle) = serve_once(TestResponse {
            status_line: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: "<html><body><main>Hello</main></body></html>",
        });
        write_target(
            &paths,
            &target_document("demo", true, url, "main", SelectionMatch::Single),
        );
        let unreadable_state =
            run_once_with_options(&paths, RunOptions::DRY_RUN).expect("dry-run unreadable");
        std::fs::set_permissions(paths.state_file(), original).expect("restore state permissions");
        handle.join().expect("unreadable state join");
        assert_eq!(unreadable_state.run_outcome, RunOutcome::Initialized);
        assert!(!unreadable_state.persist.wrote_state);
    }

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><aside>No match</aside></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let extraction_failure =
        run_once_with_options(&paths, RunOptions::DRY_RUN).expect("dry-run extraction failure");
    handle.join().expect("extraction join");
    assert_eq!(extraction_failure.run_outcome, RunOutcome::FailedPermanent);
    assert!(extraction_failure.fetch.is_some());
    assert!(!paths.last_run_file().exists());
}

#[test]
fn run_once_dry_run_waits_for_a_stable_locked_view() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let (url, response_handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );

    let exclusive_lock = try_lock_exclusive(&paths).expect("exclusive lock");
    let dry_paths = paths.clone();
    let (completion_tx, completion_rx) = mpsc::channel();
    let dry_run = thread::spawn(move || {
        let report = run_once_with_options(&dry_paths, RunOptions::DRY_RUN).expect("dry run");
        completion_tx.send(()).expect("completion signal");
        report
    });

    assert!(matches!(
        completion_rx.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));

    drop(exclusive_lock);
    let report = dry_run.join().expect("join dry run");
    response_handle.join().expect("join response server");

    assert_eq!(report.run_mode, RunMode::DryRun);
    assert_eq!(report.run_outcome, RunOutcome::Initialized);
    assert!(paths.lock_dir().is_dir());
    assert!(paths.run_lock_file().is_file());
    assert!(!paths.state_file().exists());
    assert!(!paths.last_run_file().exists());
}

#[test]
fn run_once_dry_run_surfaces_shared_lock_errors_as_fatal_core_errors() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    write_target(
        &paths,
        &target_document(
            "demo",
            true,
            Url::parse("https://example.com").expect("url"),
            "main",
            SelectionMatch::Single,
        ),
    );
    std::fs::create_dir_all(paths.target_dir()).expect("create target dir");
    std::fs::write(paths.lock_dir(), "blocked").expect("block lock path");

    let error = run_once_with_options(&paths, RunOptions::DRY_RUN).expect_err("lock io error");
    assert!(matches!(error, CoreError::Io { .. }));
}

#[test]
fn finish_report_and_notifications_cover_failure_paths() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let target = target_document(
        "demo",
        true,
        Url::parse("https://example.com").expect("url"),
        "main",
        SelectionMatch::Single,
    );

    std::fs::create_dir_all(paths.last_run_file()).expect("block last_run path");
    let report = finish_report(&paths, Some(&target), live_success_report("demo"))
        .expect("finish report with blocked last_run");
    assert!(!report.persist.wrote_last_run);

    let no_target = finish_report(&paths, None, live_success_report("demo"))
        .expect("finish report without target");
    assert!(no_target.notifications.is_empty());
    assert!(!no_target.persist.wrote_last_run);

    let delivered = deliver_notification(
        &NotificationHook {
            name: "ok".to_owned(),
            on: vec![NotificationEvent::Changed],
            shell: "/bin/sh".to_owned(),
            command: "exit 0".to_owned(),
            timeout_ms: 500,
        },
        NotificationEvent::Changed,
        &target,
        &live_success_report("demo"),
        "{\"demo\":true}",
    );
    assert!(delivered.delivered);

    let exited = deliver_notification(
        &NotificationHook {
            name: "fail".to_owned(),
            on: vec![NotificationEvent::Changed],
            shell: "/bin/sh".to_owned(),
            command: "exit 7".to_owned(),
            timeout_ms: 500,
        },
        NotificationEvent::Changed,
        &target,
        &live_success_report("demo"),
        "{\"demo\":true}",
    );
    assert_eq!(exited.exit_code, Some(7));
    assert!(!exited.delivered);

    let timed_out = deliver_notification(
        &NotificationHook {
            name: "timeout".to_owned(),
            on: vec![NotificationEvent::Changed],
            shell: "/bin/sh".to_owned(),
            command: "sleep 1".to_owned(),
            timeout_ms: 10,
        },
        NotificationEvent::Changed,
        &target,
        &live_success_report("demo"),
        "{\"demo\":true}",
    );
    assert!(timed_out.timed_out);

    let payload_failure = deliver_notification(
        &NotificationHook {
            name: "payload".to_owned(),
            on: vec![NotificationEvent::Changed],
            shell: "/bin/sh".to_owned(),
            command: "exec 0<&-; sleep 1".to_owned(),
            timeout_ms: 500,
        },
        NotificationEvent::Changed,
        &target,
        &live_success_report("demo"),
        "{\"demo\":true}",
    );
    assert!(!payload_failure.delivered);
    assert!(payload_failure.error.is_some());

    let spawn_error = deliver_notification(
        &NotificationHook {
            name: "spawn".to_owned(),
            on: vec![NotificationEvent::Changed],
            shell: "/no/such/shell".to_owned(),
            command: "exit 0".to_owned(),
            timeout_ms: 500,
        },
        NotificationEvent::Changed,
        &target,
        &live_success_report("demo"),
        "{\"demo\":true}",
    );
    assert!(spawn_error.error.is_some());
}

#[test]
fn deliver_notification_passes_documented_env_vars_and_stdin_payload() {
    let temp = tempdir().expect("tempdir");
    let payload_path = temp.path().join("payload.json");
    let env_path = temp.path().join("env.txt");
    let target = target_document(
        "demo",
        true,
        Url::parse("https://example.com").expect("url"),
        "main",
        SelectionMatch::Single,
    );
    let report = live_success_report("demo");
    let command = format!(
        "cat > '{}'; printf '%s\\n%s\\n%s\\n%s\\n%s\\n%s\\n' \
\"$FFHN_TARGET_ID\" \"$FFHN_RUN_OUTCOME\" \"$FFHN_REASON_CODE\" \
\"$FFHN_RUN_MODE\" \"$FFHN_FAILURE_CLASS\" \"$FFHN_NOTIFICATION_EVENT\" > '{}'",
        payload_path.display(),
        env_path.display(),
    );
    let delivered = deliver_notification(
        &NotificationHook {
            name: "capture".to_owned(),
            on: vec![NotificationEvent::Changed],
            shell: "/bin/sh".to_owned(),
            command,
            timeout_ms: 500,
        },
        NotificationEvent::Changed,
        &target,
        &report,
        "{\"demo\":true}",
    );
    assert!(delivered.delivered);
    assert_eq!(
        std::fs::read_to_string(&payload_path).expect("payload"),
        "{\"demo\":true}"
    );
    assert_eq!(
        std::fs::read_to_string(&env_path).expect("env"),
        "demo\nchanged\nok\nlive\n\nchanged\n"
    );
}

#[test]
fn run_once_covers_config_invalid_lock_state_and_disabled_paths() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let source_url = Url::parse("https://example.com").expect("url");

    write_target(
        &paths,
        &target_document(
            "other",
            true,
            source_url.clone(),
            "main",
            SelectionMatch::Single,
        ),
    );
    std::fs::write(paths.state_file(), [0xff]).expect("write unreadable state");
    let report = run_once(&paths).expect("config invalid run");
    assert_eq!(report.reason_code, ReasonCode::ConfigInvalid);
    assert_eq!(report.run_outcome, RunOutcome::FailedPermanent);
    assert!(!paths.run_lock_file().exists());

    write_target(
        &paths,
        &target_document(
            "demo",
            true,
            source_url.clone(),
            "main",
            SelectionMatch::Single,
        ),
    );
    let report = run_once(&paths).expect("state unreadable run");
    assert_eq!(report.reason_code, ReasonCode::StateInvalid);
    assert_eq!(report.run_outcome, RunOutcome::FailedPermanent);
    std::fs::remove_file(paths.state_file()).expect("remove unreadable state");

    let _lock = try_lock_exclusive(&paths).expect("lock");
    let report = run_once(&paths).expect("lock unavailable run");
    assert_eq!(report.reason_code, ReasonCode::LockUnavailable);

    drop(_lock);
    write_exact_text(paths.state_file(), "{not json").expect("write malformed state");
    let report = run_once(&paths).expect("malformed state run");
    assert_eq!(report.reason_code, ReasonCode::StateInvalid);

    write_json(
        paths.state_file(),
        &crate::StateDocument {
            schema_name: "wrong".to_owned(),
            schema_version: crate::STATE_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            state_phase: StatePhase::HasBaseline,
            last_run_at: None,
            last_run_outcome: None,
            last_reason_code: None,
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        },
    )
    .expect("write invalid state");
    let report = run_once(&paths).expect("state invalid run");
    assert_eq!(report.reason_code, ReasonCode::StateInvalid);

    write_snapshot_state(&paths, "hello", "<main>Hello</main>");
    write_exact_text(
        paths.target_dir().join("snapshots/current/outer.html"),
        "<main>Tampered</main>",
    )
    .expect("tamper state");
    let report = run_once(&paths).expect("integrity mismatch run");
    assert_eq!(report.reason_code, ReasonCode::IntegrityMismatch);

    std::fs::remove_file(paths.state_file()).expect("remove state");
    write_target(
        &paths,
        &target_document("demo", false, source_url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("disabled run");
    assert_eq!(report.run_outcome, RunOutcome::SkippedDisabled);
    assert_eq!(report.reason_code, ReasonCode::Disabled);
    assert!(report.persist.wrote_state);
    assert!(paths.last_run_file().is_file());
}

#[test]
fn live_run_holds_the_exclusive_lock_until_the_run_finishes() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let (source_url, accepted_rx, response_handle) = serve_once_with_accept_signal(
        TestResponse {
            status_line: "200 OK",
            content_type: "text/html",
            body: "<main>Hello</main>",
        },
        250,
    );
    write_target(
        &paths,
        &target_document("demo", true, source_url, "main", SelectionMatch::Single),
    );

    let first_paths = paths.clone();
    let first_run = thread::spawn(move || run_once(&first_paths).expect("first run"));
    accepted_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("first run reached the fetch server");

    let second_report = run_once(&paths).expect("second run");
    let first_report = first_run.join().expect("join first run");
    response_handle.join().expect("join response server");

    assert_eq!(second_report.reason_code, ReasonCode::LockUnavailable);
    assert_eq!(second_report.run_outcome, RunOutcome::FailedTransient);
    assert_eq!(first_report.run_outcome, RunOutcome::Initialized);
}

#[test]
fn run_once_surfaces_non_contention_lock_errors_as_fatal_core_errors() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    write_target(
        &paths,
        &target_document(
            "demo",
            true,
            Url::parse("https://example.com").expect("url"),
            "main",
            SelectionMatch::Single,
        ),
    );

    with_exclusive_lock_error_injected(std::io::ErrorKind::Other, || {
        let error = run_once(&paths).expect_err("lock io error");
        assert!(matches!(error, CoreError::Io { .. }));
    });
}

#[test]
fn run_once_reports_structured_persist_failures_for_live_disabled_fetch_extraction_and_success_paths()
 {
    let disabled_temp = tempdir().expect("disabled tempdir");
    let disabled_paths = TargetPaths::new(disabled_temp.path(), "demo");
    write_target(
        &disabled_paths,
        &target_document(
            "demo",
            false,
            Url::parse("https://example.com").expect("url"),
            "main",
            SelectionMatch::Single,
        ),
    );
    std::fs::create_dir_all(disabled_paths.state_file()).expect("disabled state dir conflict");
    let disabled_report = run_once(&disabled_paths).expect("disabled persist failure");
    assert_eq!(disabled_report.reason_code, ReasonCode::PersistError);
    assert_eq!(disabled_report.run_outcome, RunOutcome::FailedTransient);
    assert!(disabled_report.fetch.is_none());
    assert!(!disabled_report.persist.wrote_state);
    assert!(disabled_report.persist.wrote_last_run);
    assert!(disabled_paths.last_run_file().is_file());

    let fetch_temp = tempdir().expect("fetch tempdir");
    let fetch_paths = TargetPaths::new(fetch_temp.path(), "demo");
    write_snapshot_state(&fetch_paths, "before", "<main>Before</main>");
    let (url, fetch_handle) = serve_once_with_delay(
        TestResponse {
            status_line: "500 Internal Server Error",
            content_type: "text/html",
            body: "boom",
        },
        100,
    );
    write_target(
        &fetch_paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let fetch_state_path = fetch_paths.state_file();
    let fetch_state_conflict = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        std::fs::remove_file(&fetch_state_path).expect("remove fetch state file");
        std::fs::create_dir(&fetch_state_path).expect("fetch state dir conflict");
    });
    let fetch_report = run_once(&fetch_paths).expect("fetch persist failure");
    fetch_handle.join().expect("fetch join");
    fetch_state_conflict
        .join()
        .expect("fetch state conflict join");
    assert_eq!(fetch_report.reason_code, ReasonCode::PersistError);
    assert_eq!(fetch_report.run_outcome, RunOutcome::FailedTransient);
    assert!(fetch_report.fetch.is_some());
    assert!(fetch_report.extraction.is_none());
    assert!(!fetch_report.persist.wrote_state);

    let extraction_temp = tempdir().expect("extraction tempdir");
    let extraction_paths = TargetPaths::new(extraction_temp.path(), "demo");
    write_snapshot_state(&extraction_paths, "before", "<main>Before</main>");
    let (url, extraction_handle) = serve_once_with_delay(
        TestResponse {
            status_line: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: "<html><body><aside>No match</aside></body></html>",
        },
        100,
    );
    write_target(
        &extraction_paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let extraction_state_path = extraction_paths.state_file();
    let extraction_state_conflict = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        std::fs::remove_file(&extraction_state_path).expect("remove extraction state file");
        std::fs::create_dir(&extraction_state_path).expect("extraction state dir conflict");
    });
    let extraction_report = run_once(&extraction_paths).expect("extraction persist failure");
    extraction_handle.join().expect("extraction join");
    extraction_state_conflict
        .join()
        .expect("extraction state conflict join");
    assert_eq!(extraction_report.reason_code, ReasonCode::PersistError);
    assert_eq!(extraction_report.run_outcome, RunOutcome::FailedTransient);
    assert!(extraction_report.fetch.is_some());
    assert!(extraction_report.extraction.is_none());
    assert!(!extraction_report.persist.wrote_state);

    let success_temp = tempdir().expect("success tempdir");
    let success_paths = TargetPaths::new(success_temp.path(), "demo");
    let (url, success_handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main></body></html>",
    });
    write_target(
        &success_paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    std::fs::create_dir_all(success_paths.state_file()).expect("success state dir conflict");
    let success_report = run_once(&success_paths).expect("success persist failure");
    success_handle.join().expect("success join");
    assert_eq!(success_report.reason_code, ReasonCode::PersistError);
    assert_eq!(success_report.run_outcome, RunOutcome::FailedTransient);
    assert!(success_report.fetch.is_some());
    assert!(success_report.extraction.is_some());
    assert!(success_report.compare.is_some());
    assert!(success_report.change.is_some());
    assert_eq!(success_report.current_compare_digest_sha256, None);
    assert!(!success_report.persist.wrote_state);
    assert!(success_report.persist.wrote_last_run);
    assert!(success_paths.last_run_file().is_file());
}

#[test]
fn run_once_covers_fetch_and_extraction_failures() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");

    let (url, handle) = serve_once(TestResponse {
        status_line: "500 Internal Server Error",
        content_type: "text/html",
        body: "boom",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("fetch failure run");
    handle.join().expect("server join");
    assert_eq!(report.reason_code, ReasonCode::FetchHttpServerError);
    assert_eq!(report.run_outcome, RunOutcome::FailedTransient);

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main><main>Again</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("ambiguous extraction run");
    handle.join().expect("server join");
    assert_eq!(report.reason_code, ReasonCode::ExtractionAmbiguousMatch);

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><aside>No match</aside></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("no-match extraction run");
    handle.join().expect("server join");
    assert_eq!(report.reason_code, ReasonCode::ExtractionNoMatch);
    assert!(paths.last_run_file().is_file());
}

#[test]
fn run_once_initializes_then_detects_unchanged_and_changed_content() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("initialized run");
    handle.join().expect("server join");
    assert_eq!(report.run_outcome, RunOutcome::Initialized);
    assert_eq!(report.reason_code, ReasonCode::Ok);
    assert_eq!(report.target_status_after_run, TargetStatus::Ready);

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Hello</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("unchanged run");
    handle.join().expect("server join");
    assert_eq!(report.run_outcome, RunOutcome::Unchanged);

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        content_type: "text/html; charset=utf-8",
        body: "<html><body><main>Changed</main></body></html>",
    });
    write_target(
        &paths,
        &target_document("demo", true, url, "main", SelectionMatch::Single),
    );
    let report = run_once(&paths).expect("changed run");
    handle.join().expect("server join");
    assert_eq!(report.run_outcome, RunOutcome::Changed);
    assert_eq!(report.reason_code, ReasonCode::Ok);

    let state: crate::StateDocument = read_json(&paths.state_file()).expect("read state");
    assert_eq!(state.state_phase, StatePhase::HasBaseline);
    assert_eq!(state.snapshot_history.len(), 1);
    assert_eq!(state.snapshot_history[0].slot, SnapshotSlot::History);
    assert!(
        state.snapshot_history[0]
            .canonical_text_path
            .starts_with("snapshots/history/")
    );
    assert!(paths.last_run_file().is_file());
    let last_run: crate::RunReport = read_json(&paths.last_run_file()).expect("last run");
    assert!(report.persist.wrote_last_run);
    assert!(last_run.persist.wrote_last_run);
    assert_eq!(last_run, report);
}

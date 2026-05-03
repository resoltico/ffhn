pub(super) use super::super::super::lock::{
    try_lock_exclusive, with_exclusive_lock_error_injected,
};
pub(super) use super::super::super::storage::{
    read_json, with_write_error_injected, write_exact_text, write_json, write_text,
};
pub(super) use super::super::batch::join_batch_handle;
pub(super) use super::super::change::{
    build_change_section, common_suffix_len, excerpt_from_lines, split_lines,
};
pub(super) use super::super::notifications::{
    NotificationProcess, deliver_notification, wait_for_notification_process,
    write_child_notification_payload_or_failure, write_notification_payload_or_failure,
};
pub(super) use super::super::outcome::{
    failure_run_outcome, reason_code_for_htmlcut_error, required_outer_html,
    required_selected_match, run_outcome_from_digests,
};
pub(super) use super::super::reporting::finish_report;
pub(super) use super::super::{RunOptions, run_batch, run_once, run_once_with_options};
pub(super) use crate::stable_json::sha256_hex;
pub(super) use crate::{
    ChangeKind, CompareBasis, CompareConfig, CoreError, EXTRACTION_RECORD_SCHEMA_NAME,
    EXTRACTION_RECORD_SCHEMA_VERSION, ExtractionRecord, FetchConfig, FetchEngine,
    HTMLCUT_INTEROP_PROFILE, HttpMethod, NetworkFetchConfig, NotificationHook, OutputKind,
    PersistWriteStatus, RUN_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_VERSION, ReasonCode,
    RelativeArtifactPath, RunChangeSection, RunCompareSection, RunExtractionSection,
    RunFetchSection, RunMode, RunOutcome, RunPersistSection, RunReport, SelectionConfig,
    SelectionKind, SelectionMatch, SelectionModeConfig, SnapshotReference, SnapshotSlot,
    StatePhase, TargetDocument, TargetId, TargetPaths, TargetSource, TargetStatus, WhitespaceMode,
};
pub(super) use htmlcut_core::interop::v1::{ErrorCode, HtmlInput, execute_plan};
pub(super) use serde_json::json;
pub(super) use std::io::{Read, Write};
pub(super) use std::net::TcpListener;
#[cfg(unix)]
pub(super) use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
pub(super) use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
pub(super) use std::os::windows::process::ExitStatusExt;
pub(super) use std::process::{Command, ExitStatus, Stdio};
pub(super) use std::sync::mpsc;
pub(super) use std::thread;
pub(super) use std::time::{Duration, Instant};
pub(super) use tempfile::tempdir;
pub(super) use url::Url;

pub(super) const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

pub(super) fn target_id(value: &str) -> TargetId {
    TargetId::new(value).expect("target id")
}

pub(super) fn target_ids(values: &[&str]) -> Vec<TargetId> {
    values.iter().map(|value| target_id(value)).collect()
}

pub(super) fn artifact_path(path: impl Into<String>) -> RelativeArtifactPath {
    RelativeArtifactPath::new(path).expect("relative artifact path")
}

pub(super) struct TestResponse {
    pub(super) status_line: &'static str,
    pub(super) content_type: &'static str,
    pub(super) body: &'static str,
}

pub(super) struct BrokenWriter;

impl Write for BrokenWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("broken writer"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("broken writer"))
    }
}

pub(super) struct FakeNotificationProcess {
    pub(super) polls: Vec<std::io::Result<Option<ExitStatus>>>,
    pub(super) killed: usize,
    pub(super) waited: usize,
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
pub(super) fn exit_status(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code << 8)
}

#[cfg(windows)]
pub(super) fn exit_status(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code as u32)
}

pub(super) fn noop_abort() {}

pub(super) fn persist_section(
    duration_ms: u64,
    state_write: PersistWriteStatus,
    last_run_write: PersistWriteStatus,
) -> RunPersistSection {
    RunPersistSection::from_writes(duration_ms, state_write, last_run_write)
}

pub(super) fn live_success_report(target_name: &str) -> RunReport {
    RunReport {
        schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: RUN_REPORT_SCHEMA_VERSION,
        run_report_digest_sha256: String::new(),
        target_id: target_id(target_name),
        run_started_at: "2026-04-05T10:15:30Z".to_owned(),
        run_finished_at: "2026-04-05T10:15:31Z".to_owned(),
        run_mode: RunMode::Live,
        run_outcome: RunOutcome::Changed,
        reason_code: ReasonCode::Ok,
        failure_class: None,
        error_detail: None,
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
        persist: persist_section(
            2,
            PersistWriteStatus::Written,
            PersistWriteStatus::NotAttempted,
        ),
        notifications: Vec::new(),
        extensions: None,
    }
    .with_digest()
    .expect("report digest")
}

pub(super) fn serve_once(response: TestResponse) -> (Url, thread::JoinHandle<()>) {
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

pub(super) fn serve_once_with_delay(
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

pub(super) fn serve_once_with_accept_signal(
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

pub(super) fn target_document(
    target_name: &str,
    enabled: bool,
    source_url: Url,
    selector: &str,
    selection_match: SelectionMatch,
) -> TargetDocument {
    let selection_mode = match selection_match {
        SelectionMatch::Single => SelectionModeConfig::Single,
        SelectionMatch::First => SelectionModeConfig::First,
        SelectionMatch::Nth => SelectionModeConfig::Nth {
            index: std::num::NonZeroUsize::new(1).expect("non-zero index"),
        },
    };

    TargetDocument {
        schema_name: crate::TARGET_SCHEMA_NAME.to_owned(),
        schema_version: crate::TARGET_SCHEMA_VERSION,
        target_id: target_id(target_name),
        display_name: "Demo".to_owned(),
        enabled,
        target: TargetSource::Http { source_url },
        fetch: FetchConfig::Http(NetworkFetchConfig {
            method: HttpMethod::GET,
            timeout_ms: 15_000,
            max_bytes: 2_000_000,
            user_agent: "ffhn/example".to_owned(),
            follow_redirects: true,
            accept: "text/html".to_owned(),
            headers: Default::default(),
            extensions: None,
        }),
        selection: SelectionConfig::CssSelector {
            selection_mode,
            output: OutputKind::OuterHtml,
            whitespace: WhitespaceMode::Normalize,
            rewrite_urls: false,
            selector: selector.to_owned(),
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

pub(super) fn write_target(paths: &TargetPaths, target: &TargetDocument) {
    write_text(
        paths.target_file(),
        &toml::to_string(target).expect("target toml"),
    )
    .expect("write target");
}

pub(super) fn snapshot_reference(
    slot: SnapshotSlot,
    name: &str,
    canonical: &str,
    outer: &str,
) -> SnapshotReference {
    SnapshotReference {
        slot,
        canonical_text_sha256: sha256_hex(canonical.as_bytes()),
        outer_html_sha256: sha256_hex(outer.as_bytes()),
        extraction_record_path: artifact_path(format!("snapshots/{name}/extraction.json")),
        canonical_text_path: artifact_path(format!("snapshots/{name}/canonical.txt")),
        outer_html_path: artifact_path(format!("snapshots/{name}/outer.html")),
        captured_at: "2026-04-05T10:15:30Z".to_owned(),
    }
}

pub(super) fn write_snapshot_state(paths: &TargetPaths, canonical: &str, outer: &str) {
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
            target_id: target_id(paths.target_id()),
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

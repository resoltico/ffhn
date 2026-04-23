use super::*;
use std::collections::BTreeMap;
use std::io::{self, Cursor, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::{
    CompareBasis, CompareConfig, FetchConfig, FetchEngine, HttpMethod, OutputKind, SelectionConfig,
    SelectionKind, SelectionMatch, TargetDocument, TargetSource, WhitespaceMode,
};

struct BrokenReader;

impl Read for BrokenReader {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("boom"))
    }
}

struct TestResponse {
    status_line: &'static str,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
    delay_before_response_ms: u64,
}

fn serve_once(response: TestResponse) -> (Url, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind server");
    let address = listener.local_addr().expect("server addr");
    let url = Url::parse(&format!("http://{address}")).expect("server url");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept connection");
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request);
        if response.delay_before_response_ms > 0 {
            thread::sleep(Duration::from_millis(response.delay_before_response_ms));
        }

        let mut raw = format!("HTTP/1.1 {}\r\n", response.status_line).into_bytes();
        for (name, value) in response.headers {
            raw.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
        }
        raw.extend_from_slice(b"\r\n");
        raw.extend_from_slice(&response.body);
        let _ = stream.write_all(&raw);
    });

    (url, handle)
}

fn target_for(url: Url) -> TargetDocument {
    TargetDocument {
        schema_name: crate::TARGET_SCHEMA_NAME.to_owned(),
        schema_version: crate::TARGET_SCHEMA_VERSION,
        target_id: "demo".to_owned(),
        display_name: "Demo".to_owned(),
        enabled: true,
        target: TargetSource {
            kind: crate::model::TargetKind::Http,
            source_url: Some(url),
            file_path: None,
        },
        fetch: FetchConfig {
            engine: FetchEngine::Http,
            method: HttpMethod::GET,
            timeout_ms: 1_000,
            max_bytes: 1_024,
            user_agent: "ffhn/2.0.0".to_owned(),
            follow_redirects: true,
            accept: "text/html,application/xhtml+xml".to_owned(),
            headers: BTreeMap::new(),
            extensions: None,
        },
        selection: SelectionConfig {
            kind: SelectionKind::CssSelector,
            r#match: SelectionMatch::Single,
            index: None,
            output: OutputKind::OuterHtml,
            whitespace: WhitespaceMode::Normalize,
            rewrite_urls: false,
            selector: Some("main".to_owned()),
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

fn file_target_for(path: &Path) -> TargetDocument {
    TargetDocument {
        schema_name: crate::TARGET_SCHEMA_NAME.to_owned(),
        schema_version: crate::TARGET_SCHEMA_VERSION,
        target_id: "demo_file".to_owned(),
        display_name: "Demo File".to_owned(),
        enabled: true,
        target: TargetSource {
            kind: crate::model::TargetKind::File,
            source_url: None,
            file_path: Some(path.to_string_lossy().into_owned()),
        },
        fetch: FetchConfig {
            engine: FetchEngine::File,
            method: HttpMethod::GET,
            timeout_ms: 1_000,
            max_bytes: 1_024,
            user_agent: String::new(),
            follow_redirects: false,
            accept: String::new(),
            headers: BTreeMap::new(),
            extensions: None,
        },
        selection: SelectionConfig {
            kind: SelectionKind::CssSelector,
            r#match: SelectionMatch::Single,
            index: None,
            output: OutputKind::OuterHtml,
            whitespace: WhitespaceMode::Normalize,
            rewrite_urls: false,
            selector: Some("main".to_owned()),
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

#[test]
fn fetch_target_reads_successful_html_and_normalizes_line_endings() {
    let body = b"<html><main>Hello\r\nWorld</main></html>".to_vec();
    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        headers: vec![
            ("Content-Type", "text/html; charset=utf-8".to_owned()),
            ("Content-Length", body.len().to_string()),
        ],
        body,
        delay_before_response_ms: 0,
    });

    let mut target = target_for(url.clone());
    target
        .fetch
        .headers
        .insert("X-Test".to_owned(), "demo".to_owned());

    let result = fetch_target(&target).expect("successful fetch");
    handle.join().expect("server join");

    assert_eq!(result.final_url.as_str(), url.as_str());
    assert_eq!(result.html, "<html><main>Hello\nWorld</main></html>");
    assert_eq!(result.report.http_status, Some(200));
    assert_eq!(
        result.report.content_type.as_deref(),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(result.report.bytes_read, Some(38));
}

#[test]
fn fetch_target_preserves_browser_engine_in_reports_while_using_http_transport() {
    let body = b"<html><main>Hello</main></html>".to_vec();
    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        headers: vec![
            ("Content-Type", "text/html; charset=utf-8".to_owned()),
            ("Content-Length", body.len().to_string()),
        ],
        body,
        delay_before_response_ms: 0,
    });

    let mut target = target_for(url.clone());
    target.fetch.engine = FetchEngine::Browser;

    let result = fetch_target(&target).expect("successful browser-engine fetch");
    handle.join().expect("server join");

    assert_eq!(result.final_url.as_str(), url.as_str());
    assert_eq!(result.report.engine, FetchEngine::Browser);
    assert_eq!(result.report.http_status, Some(200));
}

#[test]
fn fetch_target_rejects_http_error_unsupported_type_and_oversized_responses() {
    let (url, handle) = serve_once(TestResponse {
        status_line: "500 Internal Server Error",
        headers: vec![("Content-Type", "text/html".to_owned())],
        body: b"boom".to_vec(),
        delay_before_response_ms: 0,
    });
    let failure = fetch_target(&target_for(url)).expect_err("http error");
    handle.join().expect("server join");
    assert_eq!(failure.reason_code, ReasonCode::FetchHttpServerError);
    assert_eq!(failure.report.http_status, Some(500));

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        headers: vec![
            ("Content-Type", "application/json".to_owned()),
            ("Content-Length", "2".to_owned()),
        ],
        body: b"{}".to_vec(),
        delay_before_response_ms: 0,
    });
    let failure = fetch_target(&target_for(url)).expect_err("unsupported content type");
    handle.join().expect("server join");
    assert_eq!(failure.reason_code, ReasonCode::FetchUnsupportedContentType);

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        headers: vec![
            ("Content-Type", "text/html".to_owned()),
            ("Content-Length", "2048".to_owned()),
        ],
        body: b"<main>ignored</main>".to_vec(),
        delay_before_response_ms: 0,
    });
    let failure = fetch_target(&target_for(url)).expect_err("too large by content-length");
    handle.join().expect("server join");
    assert_eq!(failure.reason_code, ReasonCode::FetchTooLarge);

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        headers: vec![("Content-Type", "text/html".to_owned())],
        body: b"<main>this body is too large</main>".to_vec(),
        delay_before_response_ms: 0,
    });
    let mut target = target_for(url);
    target.fetch.max_bytes = 8;
    let failure = fetch_target(&target).expect_err("too large while streaming");
    handle.join().expect("server join");
    assert_eq!(failure.reason_code, ReasonCode::FetchTooLarge);
}

#[test]
fn fetch_target_rejects_decode_network_and_timeout_failures() {
    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        headers: vec![
            ("Content-Type", "text/html; charset=unknown".to_owned()),
            ("Content-Length", "4".to_owned()),
        ],
        body: b"demo".to_vec(),
        delay_before_response_ms: 0,
    });
    let failure = fetch_target(&target_for(url)).expect_err("decode error");
    handle.join().expect("server join");
    assert_eq!(failure.reason_code, ReasonCode::FetchDecodeError);
    assert_eq!(failure.report.bytes_read, Some(4));

    let closed_listener = TcpListener::bind("127.0.0.1:0").expect("bind closed listener");
    let closed_url = Url::parse(&format!(
        "http://{}",
        closed_listener.local_addr().expect("closed addr")
    ))
    .expect("closed url");
    drop(closed_listener);
    let failure = fetch_target(&target_for(closed_url)).expect_err("network error");
    assert_eq!(failure.reason_code, ReasonCode::FetchNetworkError);
    assert!(failure.report.http_status.is_none());

    let (url, handle) = serve_once(TestResponse {
        status_line: "200 OK",
        headers: vec![
            ("Content-Type", "text/html; charset=utf-8".to_owned()),
            ("Content-Length", "17".to_owned()),
        ],
        body: b"<main>slow</main>".to_vec(),
        delay_before_response_ms: 200,
    });
    let mut target = target_for(url);
    target.fetch.timeout_ms = 50;
    let failure = fetch_target(&target).expect_err("timeout");
    handle.join().expect("server join");
    assert_eq!(failure.reason_code, ReasonCode::FetchTimeout);
}

#[test]
fn fetch_target_rejects_invalid_direct_target_documents_without_panicking() {
    let mut missing_http_url = target_for(Url::parse("https://example.com").expect("url"));
    missing_http_url.target.source_url = None;
    let failure = fetch_target(&missing_http_url).expect_err("missing http source url");
    assert_eq!(failure.reason_code, ReasonCode::ConfigInvalid);
    assert_eq!(failure.report.engine, FetchEngine::Http);
    assert_eq!(failure.report.duration_ms, 0);

    let mut invalid_http_scheme = target_for(Url::parse("https://example.com").expect("url"));
    invalid_http_scheme.target.source_url =
        Some(Url::parse("file:///tmp/demo.html").expect("file url"));
    let failure = fetch_target(&invalid_http_scheme).expect_err("non-http source url");
    assert_eq!(failure.reason_code, ReasonCode::ConfigInvalid);
    assert_eq!(failure.report.engine, FetchEngine::Http);

    let mut missing_file_path = file_target_for(Path::new("/tmp/demo.html"));
    missing_file_path.target.file_path = None;
    let failure = fetch_target(&missing_file_path).expect_err("missing file path");
    assert_eq!(failure.reason_code, ReasonCode::ConfigInvalid);
    assert_eq!(failure.report.engine, FetchEngine::File);

    let relative_path_target = file_target_for(Path::new("relative.html"));
    let failure = fetch_target(&relative_path_target).expect_err("relative file path");
    assert_eq!(failure.reason_code, ReasonCode::ConfigInvalid);
    assert_eq!(failure.report.engine, FetchEngine::File);
}

#[test]
fn helper_functions_cover_charset_size_and_error_mapping_edges() {
    let agent = build_agent(&target_for(Url::parse("https://example.com").expect("url")).fetch);
    assert!(matches!(
        agent.config().tls_config().root_certs(),
        RootCerts::PlatformVerifier
    ));
    assert_eq!(agent.config().max_redirects(), 10);

    let mut target = target_for(Url::parse("https://example.com").expect("url"));
    target.fetch.follow_redirects = false;
    let agent = build_agent(&target.fetch);
    assert_eq!(agent.config().max_redirects(), 0);

    assert!(supported_content_type(None));
    assert!(supported_content_type(Some("text/html")));
    assert!(supported_content_type(Some(
        "application/xhtml+xml; charset=utf-8"
    )));
    assert!(!supported_content_type(Some("application/json")));

    assert_eq!(
        read_limited_bytes(Cursor::new(b"demo"), 10).expect("cursor"),
        b"demo"
    );
    assert_eq!(
        read_limited_bytes(Cursor::new(b"toolong"), 3).expect_err("too large"),
        ReasonCode::FetchTooLarge
    );
    assert_eq!(
        read_limited_bytes(BrokenReader, 10).expect_err("io error"),
        ReasonCode::FetchNetworkError
    );

    assert_eq!(
        decode_body("demo".as_bytes(), None).expect("default utf8"),
        "demo"
    );
    assert_eq!(
        decode_body(&[0x80], Some("text/html; charset=utf-8")).expect_err("invalid utf8"),
        ReasonCode::FetchDecodeError
    );
    assert_eq!(
        decode_body(b"demo", Some("text/html; charset=unknown")).expect_err("unknown charset"),
        ReasonCode::FetchDecodeError
    );

    assert_eq!(
        charset_from_content_type(None).expect("default charset"),
        UTF_8
    );
    assert_eq!(
        charset_from_content_type(Some("text/html; charset=utf-8")).expect("utf8"),
        UTF_8
    );
    assert!(charset_from_content_type(Some("text/html; charset=unknown")).is_none());
    assert_eq!(
        charset_from_content_type(Some("text/html; boundary=demo")).expect("defaulted utf8"),
        UTF_8
    );

    assert_eq!(
        map_ureq_error(&ureq::Error::Timeout(ureq::Timeout::Global)),
        ReasonCode::FetchTimeout
    );
    assert_eq!(
        map_ureq_error(&ureq::Error::BodyExceedsLimit(5)),
        ReasonCode::FetchTooLarge
    );
    assert_eq!(
        map_ureq_error(&ureq::Error::StatusCode(500)),
        ReasonCode::FetchHttpServerError
    );
    assert_eq!(
        map_http_status_reason(404),
        ReasonCode::FetchHttpClientError
    );
    assert_eq!(
        map_ureq_error(&ureq::Error::ConnectionFailed),
        ReasonCode::FetchNetworkError
    );
    assert_eq!(
        map_ureq_error(&ureq::Error::Other(Box::new(io::Error::other("other")))),
        ReasonCode::FetchNetworkError
    );
    assert_eq!(
        parse_final_url_or_source(
            "not a url",
            &Url::parse("https://example.com/fallback").expect("fallback url")
        )
        .as_str(),
        "https://example.com/fallback"
    );
}

#[test]
fn fetch_target_covers_file_source_success_and_failure_modes() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source_path = temp.path().join("source.html");
    std::fs::write(&source_path, "<html><main>Hello</main></html>").expect("write source");

    let success = fetch_target(&file_target_for(&source_path)).expect("file success");
    assert_eq!(success.report.engine, FetchEngine::File);
    assert!(success.final_url.as_str().starts_with("file://"));
    assert_eq!(success.report.bytes_read, Some(31));

    let missing_path = temp.path().join("missing.html");
    let missing = fetch_target(&file_target_for(&missing_path)).expect_err("missing file");
    assert_eq!(missing.reason_code, ReasonCode::FetchSourceError);
    assert!(missing.report.bytes_read.is_none());

    let oversized_path = temp.path().join("oversized.html");
    std::fs::write(&oversized_path, "<main>too large</main>").expect("write oversized");
    let mut oversized_target = file_target_for(&oversized_path);
    oversized_target.fetch.max_bytes = 8;
    let oversized = fetch_target(&oversized_target).expect_err("oversized file");
    assert_eq!(oversized.reason_code, ReasonCode::FetchTooLarge);
    assert_eq!(oversized.report.bytes_read, Some(22));

    let invalid_utf8_path = temp.path().join("invalid.bin");
    std::fs::write(&invalid_utf8_path, [0xff, 0xfe, 0xfd]).expect("write invalid utf8");
    let invalid_utf8 =
        fetch_target(&file_target_for(&invalid_utf8_path)).expect_err("decode failure");
    assert_eq!(invalid_utf8.reason_code, ReasonCode::FetchDecodeError);
    assert_eq!(invalid_utf8.report.bytes_read, Some(3));
}

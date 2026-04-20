use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use encoding_rs::{Encoding, UTF_8};
use ureq::ResponseExt;
use ureq::tls::{RootCerts, TlsConfig};
use url::Url;

use crate::canonical::normalize_line_endings;
use crate::{FetchConfig, FetchEngine, ReasonCode, RunFetchSection, TargetDocument};
use crate::model::TargetKind;

/// Successful fetch payload returned to FFHN's extraction stage.
#[derive(Clone, Debug)]
pub struct FetchSuccess {
    /// Final URL after redirects when known.
    pub final_url: Url,
    /// Decoded HTML string passed into HTMLCut.
    pub html: String,
    /// Structured fetch report section.
    pub report: RunFetchSection,
}

/// Structured fetch failure returned before extraction starts.
#[derive(Clone, Debug)]
pub struct FetchFailure {
    /// FFHN reason code for the failure.
    pub reason_code: ReasonCode,
    /// Structured fetch report section.
    pub report: RunFetchSection,
}

/// Fetches one configured FFHN target.
pub fn fetch_target(target: &TargetDocument) -> Result<FetchSuccess, FetchFailure> {
    match target.target.kind {
        TargetKind::Http => fetch_http_target(target),
        TargetKind::File => fetch_file_target(target),
    }
}

fn fetch_http_target(target: &TargetDocument) -> Result<FetchSuccess, FetchFailure> {
    let started = Instant::now();
    let fetch = &target.fetch;
    let agent = build_agent(fetch);

    let mut request = agent.get(
        target
            .target
            .source_url
            .as_ref()
            .expect("validated http source url")
            .as_str(),
    );
    request = request.header("Accept", &fetch.accept);
    request = request.header("User-Agent", &fetch.user_agent);

    for (name, value) in &fetch.headers {
        request = request.header(name, value);
    }

    let mut response = match request.call() {
        Ok(response) => response,
        Err(error) => {
            let duration_ms = started.elapsed().as_millis() as u64;
            let reason_code = map_ureq_error(&error);
            return Err(FetchFailure {
                reason_code,
                report: RunFetchSection {
                    engine: fetch.engine,
                    final_url: None,
                    http_status: None,
                    content_type: None,
                    bytes_read: None,
                    duration_ms,
                },
            });
        }
    };

    let duration_ms = started.elapsed().as_millis() as u64;
    let final_url = Url::parse(&response.get_uri().to_string())
        .unwrap_or_else(|_| {
            target
                .target
                .source_url
                .as_ref()
                .expect("validated http source url")
                .clone()
        });
    let http_status = response.status().as_u16();
    let content_type = header_value(&response, "content-type");

    if !http_status_is_success(http_status) {
        return Err(FetchFailure {
            reason_code: map_http_status_reason(http_status),
            report: RunFetchSection {
                engine: fetch.engine,
                final_url: Some(final_url.to_string()),
                http_status: Some(http_status),
                content_type,
                bytes_read: None,
                duration_ms,
            },
        });
    }

    if !supported_content_type(content_type.as_deref()) {
        return Err(FetchFailure {
            reason_code: ReasonCode::FetchUnsupportedContentType,
            report: RunFetchSection {
                engine: fetch.engine,
                final_url: Some(final_url.to_string()),
                http_status: Some(http_status),
                content_type,
                bytes_read: None,
                duration_ms,
            },
        });
    }

    if content_length_exceeds_limit(&response, fetch.max_bytes) {
        return Err(FetchFailure {
            reason_code: ReasonCode::FetchTooLarge,
            report: RunFetchSection {
                engine: fetch.engine,
                final_url: Some(final_url.to_string()),
                http_status: Some(http_status),
                content_type,
                bytes_read: None,
                duration_ms,
            },
        });
    }

    let bytes = match read_limited_bytes(response.body_mut().as_reader(), fetch.max_bytes) {
        Ok(bytes) => bytes,
        Err(reason_code) => {
            return Err(FetchFailure {
                reason_code,
                report: RunFetchSection {
                    engine: fetch.engine,
                    final_url: Some(final_url.to_string()),
                    http_status: Some(http_status),
                    content_type,
                    bytes_read: None,
                    duration_ms,
                },
            });
        }
    };

    let html = match decode_body(&bytes, content_type.as_deref()) {
        Ok(html) => html,
        Err(reason_code) => {
            return Err(FetchFailure {
                reason_code,
                report: RunFetchSection {
                    engine: fetch.engine,
                    final_url: Some(final_url.to_string()),
                    http_status: Some(http_status),
                    content_type,
                    bytes_read: Some(bytes.len()),
                    duration_ms,
                },
            });
        }
    };

    Ok(FetchSuccess {
        final_url: final_url.clone(),
        html: normalize_line_endings(&html),
        report: RunFetchSection {
            engine: fetch.engine,
            final_url: Some(final_url.to_string()),
            http_status: Some(http_status),
            content_type,
            bytes_read: Some(bytes.len()),
            duration_ms,
        },
    })
}

fn fetch_file_target(target: &TargetDocument) -> Result<FetchSuccess, FetchFailure> {
    let started = Instant::now();
    let fetch = &target.fetch;
    let path = Path::new(
        target
            .target
            .file_path
            .as_deref()
            .expect("validated file path"),
    );

    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(FetchFailure {
                reason_code: ReasonCode::FetchNetworkError,
                report: RunFetchSection {
                    engine: FetchEngine::File,
                    final_url: None,
                    http_status: None,
                    content_type: None,
                    bytes_read: None,
                    duration_ms: started.elapsed().as_millis() as u64,
                },
            });
        }
    };

    if bytes.len() > fetch.max_bytes {
        return Err(FetchFailure {
            reason_code: ReasonCode::FetchTooLarge,
            report: RunFetchSection {
                engine: FetchEngine::File,
                final_url: None,
                http_status: None,
                content_type: None,
                bytes_read: Some(bytes.len()),
                duration_ms: started.elapsed().as_millis() as u64,
            },
        });
    }

    let html = match std::str::from_utf8(&bytes) {
        Ok(text) => normalize_line_endings(text),
        Err(_) => {
            return Err(FetchFailure {
                reason_code: ReasonCode::FetchDecodeError,
                report: RunFetchSection {
                    engine: FetchEngine::File,
                    final_url: None,
                    http_status: None,
                    content_type: None,
                    bytes_read: Some(bytes.len()),
                    duration_ms: started.elapsed().as_millis() as u64,
                },
            });
        }
    };

    let canonical_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let final_url = Url::from_file_path(&canonical_path)
        .map_err(|_| FetchFailure {
            reason_code: ReasonCode::FetchDecodeError,
            report: RunFetchSection {
                engine: FetchEngine::File,
                final_url: None,
                http_status: None,
                content_type: None,
                bytes_read: Some(bytes.len()),
                duration_ms: started.elapsed().as_millis() as u64,
            },
        })?;

    Ok(FetchSuccess {
        final_url: final_url.clone(),
        html,
        report: RunFetchSection {
            engine: FetchEngine::File,
            final_url: Some(final_url.to_string()),
            http_status: None,
            content_type: None,
            bytes_read: Some(bytes.len()),
            duration_ms: started.elapsed().as_millis() as u64,
        },
    })
}

fn build_agent(fetch: &FetchConfig) -> ureq::Agent {
    let tls_config = TlsConfig::builder()
        .root_certs(RootCerts::PlatformVerifier)
        .build();

    ureq::Agent::config_builder()
        .tls_config(tls_config)
        .timeout_global(Some(Duration::from_millis(fetch.timeout_ms)))
        .max_redirects(if fetch.follow_redirects { 10 } else { 0 })
        .max_redirects_will_error(false)
        .save_redirect_history(true)
        .http_status_as_error(false)
        .build()
        .into()
}

fn map_ureq_error(error: &ureq::Error) -> ReasonCode {
    match error {
        ureq::Error::Timeout(_) => ReasonCode::FetchTimeout,
        ureq::Error::BodyExceedsLimit(_) => ReasonCode::FetchTooLarge,
        ureq::Error::StatusCode(status) => map_http_status_reason(*status),
        ureq::Error::ConnectionFailed
        | ureq::Error::TooManyRedirects
        | ureq::Error::ConnectProxyFailed(_)
        | ureq::Error::RequireHttpsOnly(_)
        | ureq::Error::HostNotFound
        | ureq::Error::Io(_)
        | ureq::Error::Tls(_)
        | ureq::Error::Protocol(_)
        | ureq::Error::RedirectFailed
        | ureq::Error::BadUri(_)
        | ureq::Error::LargeResponseHeader(_, _) => ReasonCode::FetchNetworkError,
        _ => ReasonCode::FetchNetworkError,
    }
}

fn map_http_status_reason(status: u16) -> ReasonCode {
    if status >= 500 {
        ReasonCode::FetchHttpServerError
    } else {
        ReasonCode::FetchHttpClientError
    }
}

fn http_status_is_success(status: u16) -> bool {
    (200..300).contains(&status)
}

fn supported_content_type(value: Option<&str>) -> bool {
    value.is_none_or(|value| {
        let media_type = value
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        media_type == "text/html" || media_type == "application/xhtml+xml"
    })
}

fn content_length_exceeds_limit(
    response: &ureq::http::Response<ureq::Body>,
    max_bytes: usize,
) -> bool {
    header_value(response, "content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|value| value > max_bytes)
}

fn header_value(response: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn read_limited_bytes(mut reader: impl Read, max_bytes: usize) -> Result<Vec<u8>, ReasonCode> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| ReasonCode::FetchNetworkError)?;
        if read == 0 {
            break;
        }

        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > max_bytes {
            return Err(ReasonCode::FetchTooLarge);
        }
    }

    Ok(bytes)
}

fn decode_body(bytes: &[u8], content_type: Option<&str>) -> Result<String, ReasonCode> {
    let encoding = charset_from_content_type(content_type).ok_or(ReasonCode::FetchDecodeError)?;
    let (text, _, had_errors) = encoding.decode(bytes);
    if had_errors {
        return Err(ReasonCode::FetchDecodeError);
    }
    Ok(text.into_owned())
}

fn charset_from_content_type(content_type: Option<&str>) -> Option<&'static Encoding> {
    let Some(content_type) = content_type else {
        return Some(UTF_8);
    };

    for parameter in content_type.split(';').skip(1) {
        let mut parts = parameter.trim().splitn(2, '=');
        let name = parts.next()?.trim();
        let value = parts.next()?.trim().trim_matches('"');
        if name.eq_ignore_ascii_case("charset") {
            return Encoding::for_label(value.as_bytes());
        }
    }

    Some(UTF_8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::io::{self, Cursor, Write};
    use std::net::TcpListener;
    use std::thread;

    use crate::{
        CompareBasis, CompareConfig, FetchEngine, HttpMethod, OutputKind, SelectionConfig,
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
            map_ureq_error(&ureq::Error::ConnectionFailed),
            ReasonCode::FetchNetworkError
        );
        assert_eq!(
            map_ureq_error(&ureq::Error::Other(Box::new(io::Error::other("other")))),
            ReasonCode::FetchNetworkError
        );
    }
}

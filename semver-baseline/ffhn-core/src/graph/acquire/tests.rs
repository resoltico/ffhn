use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

use super::*;

fn source(response_url: &str) -> SourceDocument {
    toml::from_str(&format!(
        r#"
schema_name = "ffhn.source"
schema_version = 1
source_id = "demo"
display_name = "Demo"
enabled = true
escalate_after = 2
[fetch]
engine = "http"
source_url = "{response_url}"
user_agent = "ffhn-test"
accept = "application/json"
max_bytes = 1024
follow_redirects = true
max_redirects = 5
[fetch.timeouts]
connect_ms = 1000
read_idle_ms = 1000
total_ms = 1000
[conditional]
enabled = true
[schedule]
interval_ms = 1000
min_interval_ms = 1000
"#,
    ))
    .expect("source document")
}

fn acquire_raw_response(response: &[u8]) -> Result<SourceAcquisition, SourceAcquireError> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let response = response.to_vec();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("request");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request).expect("read request");
        stream.write_all(&response).expect("response");
    });
    let result = acquire_source(&source(&format!("http://{address}/value")));
    worker.join().expect("worker");
    result
}

#[test]
fn http_acquisition_accepts_only_complete_representations_and_preserves_final_url() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let worker = thread::spawn(move || {
        for response in [
            format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{address}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .into_bytes(),
            b"HTTP/1.1 203 Non-Authoritative Information\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_vec(),
        ] {
            let (mut stream, _) = listener.accept().expect("request");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).expect("read request");
            stream.write_all(&response).expect("response");
        }
    });
    let SourceAcquisition::Document(document) =
        acquire_source(&source(&format!("http://{address}/start"))).expect("acquisition")
    else {
        panic!("redirected 203 response must supply a document");
    };
    worker.join().expect("worker");
    assert_eq!(document.body, "{}");
    assert_eq!(
        document.effective_http_url.expect("final URL").as_str(),
        format!("http://{address}/final")
    );
}

#[test]
fn extensible_headers_never_return_after_a_cross_origin_redirect() {
    let origin = TcpListener::bind("127.0.0.1:0").expect("origin listener");
    let origin_address = origin.local_addr().expect("origin address");
    let foreign = TcpListener::bind("127.0.0.1:0").expect("foreign listener");
    let foreign_address = foreign.local_addr().expect("foreign address");
    let origin_worker = thread::spawn(move || {
        let (mut first, _) = origin.accept().expect("initial request");
        let mut request = [0_u8; 2048];
        let first_count = first.read(&mut request).expect("read initial request");
        first
            .write_all(
                format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://{foreign_address}/hop\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .expect("initial redirect");
        let initial = String::from_utf8(request[..first_count].to_vec()).expect("initial text");

        let (mut returned, _) = origin.accept().expect("returned request");
        let returned_count = returned.read(&mut request).expect("read returned request");
        returned
            .write_all(b"HTTP/1.1 200 OK\r\nETag: \"returned\"\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .expect("final response");
        let returned =
            String::from_utf8(request[..returned_count].to_vec()).expect("returned text");
        (initial, returned)
    });
    let foreign_worker = thread::spawn(move || {
        let (mut stream, _) = foreign.accept().expect("cross-origin request");
        let mut request = [0_u8; 2048];
        let count = stream
            .read(&mut request)
            .expect("read cross-origin request");
        stream
            .write_all(
                format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://{origin_address}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .expect("return redirect");
        String::from_utf8(request[..count].to_vec()).expect("foreign text")
    });
    let configured = source(&format!("http://{origin_address}/start"));
    let configured: SourceDocument =
        toml::from_str(&toml::to_string(&configured).expect("source TOML").replace(
            "[conditional]",
            "[fetch.headers]\nX-Scope = \"private\"\n\n[conditional]",
        ))
        .expect("source with extensible header");
    let SourceAcquisition::Document(document) =
        acquire_source(&configured).expect("redirected acquisition")
    else {
        panic!("redirected acquisition must return a document");
    };
    assert!(document.validators.is_none());
    let (initial, returned) = origin_worker.join().expect("origin worker");
    let foreign = foreign_worker.join().expect("foreign worker");
    assert!(initial.to_ascii_lowercase().contains("x-scope: private"));
    assert!(!foreign.to_ascii_lowercase().contains("x-scope:"));
    assert!(!returned.to_ascii_lowercase().contains("x-scope:"));
    assert!(
        foreign
            .to_ascii_lowercase()
            .contains("accept: application/json")
    );
    assert!(
        returned
            .to_ascii_lowercase()
            .contains("accept: application/json")
    );
}

#[test]
fn declared_content_length_equal_to_the_byte_limit_is_accepted() {
    let body = "x".repeat(1_024);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let SourceAcquisition::Document(document) =
        acquire_raw_response(response.as_bytes()).expect("boundary-sized document")
    else {
        panic!("boundary-sized response must provide a document");
    };
    assert_eq!(document.body.len(), 1_024);
}

#[test]
fn http_acquisition_refuses_non_representation_2xx_statuses() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("request");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request).expect("read request");
        stream
            .write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 2\r\n\r\n{}")
            .expect("response");
    });
    let error = acquire_source(&source(&format!("http://{address}/value")))
        .expect_err("206 is not a complete representation");
    worker.join().expect("worker");
    assert_eq!(
        error,
        SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::HttpSuccessNotRepresentation,
            status: Some(206),
            raw_platform_error: None,
        })
    );
}

#[test]
fn complete_representation_requires_each_framing_completion_signal() {
    let SourceAcquisition::Document(document) =
        acquire_raw_response(b"HTTP/1.0 200 OK\r\nConnection: close\r\n\r\n{}")
            .expect("graceful close-delimited response")
    else {
        panic!("HTTP/1.0 graceful close must be a complete representation");
    };
    assert_eq!(document.body, "{}");

    for response in [
        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n{}".as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2\r\n{}\r\n"
            .as_slice(),
    ] {
        let error = acquire_raw_response(response).expect_err("truncated framing");
        assert!(matches!(
            error,
            SourceAcquireError::Fetch(SourceFetchFailure {
                kind: SourceFetchFailureKind::IncompleteBody,
                ..
            })
        ));
    }
}

#[test]
fn direct_conditional_request_sends_provenance_matched_validators_and_accepts_304() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("request");
        let mut request = [0_u8; 1024];
        let count = stream.read(&mut request).expect("read request");
        stream
            .write_all(b"HTTP/1.1 304 Not Modified\r\nETag: \"next\"\r\nContent-Length: 0\r\n\r\n")
            .expect("response");
        String::from_utf8(request[..count].to_vec()).expect("request text")
    });
    let source_url = Url::parse(&format!("http://{address}/value")).expect("source URL");
    let validators = HttpValidators {
        issued_url: source_url.clone(),
        etag: Some("\"prior\"".to_owned()),
        last_modified: Some("Tue, 01 Jan 2030 00:00:00 GMT".to_owned()),
    };
    let result = acquire_source_with_validators(&source(source_url.as_str()), Some(&validators))
        .expect("conditional acquire");
    let request = worker.join().expect("worker").to_ascii_lowercase();
    let SourceAcquisition::NotModified(merged) = result else {
        panic!("direct 304 must produce not_modified");
    };
    assert!(request.contains("if-none-match: \"prior\"\r\n"));
    assert_eq!(merged.issued_url, source_url);
    assert_eq!(merged.etag.as_deref(), Some("\"next\""));
    assert_eq!(
        merged.last_modified.as_deref(),
        Some("Tue, 01 Jan 2030 00:00:00 GMT")
    );
}

#[test]
fn conditional_validators_are_sent_only_to_their_direct_issuing_url() {
    let origin = TcpListener::bind("127.0.0.1:0").expect("origin listener");
    let origin_address = origin.local_addr().expect("origin address");
    let foreign = TcpListener::bind("127.0.0.1:0").expect("foreign listener");
    let foreign_address = foreign.local_addr().expect("foreign address");
    let origin_worker = thread::spawn(move || {
        let (mut stream, _) = origin.accept().expect("origin request");
        let mut request = [0_u8; 2048];
        let count = stream.read(&mut request).expect("origin request bytes");
        stream
            .write_all(
                format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://{foreign_address}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .expect("redirect");
        String::from_utf8(request[..count].to_vec()).expect("origin request text")
    });
    let foreign_worker = thread::spawn(move || {
        let (mut stream, _) = foreign.accept().expect("foreign request");
        let mut request = [0_u8; 2048];
        let count = stream.read(&mut request).expect("foreign request bytes");
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .expect("document");
        String::from_utf8(request[..count].to_vec()).expect("foreign request text")
    });
    let issued_url = Url::parse(&format!("http://{origin_address}/start")).expect("origin URL");
    let validators = HttpValidators {
        issued_url: issued_url.clone(),
        etag: Some("\"origin-etag\"".to_owned()),
        last_modified: None,
    };
    assert!(matches!(
        acquire_source_with_validators(&source(issued_url.as_str()), Some(&validators))
            .expect("redirected document"),
        SourceAcquisition::Document(_)
    ));
    let origin_request = origin_worker.join().expect("origin worker");
    let foreign_request = foreign_worker.join().expect("foreign worker");
    assert!(
        origin_request
            .to_ascii_lowercase()
            .contains("if-none-match: \"origin-etag\"")
    );
    assert!(
        !foreign_request
            .to_ascii_lowercase()
            .contains("if-none-match:")
    );

    assert_eq!(redirect_count_after_hop(0), 1);
    assert_eq!(redirect_count_after_hop(1), 2);
    assert_eq!(redirect_count_after_hop(u8::MAX), u8::MAX);
}

#[path = "tests/failures.rs"]
mod failures;

#[path = "tests/coverage.rs"]
mod coverage;

use super::*;

#[test]
fn file_acquisition_reads_utf8_and_classifies_a_missing_file() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let file = temporary.path().join("source.json");
    fs::write(&file, "{\"price\":7}").expect("source file");
    let document: SourceDocument = toml::from_str(&format!(
        r#"
schema_name = "ffhn.source"
schema_version = 1
source_id = "file"
display_name = "File"
enabled = true
escalate_after = 2
[fetch]
engine = "file"
file_path = {file:?}
max_bytes = 1024
[conditional]
enabled = false
[schedule]
interval_ms = 1000
min_interval_ms = 1000
"#,
        file = file.to_string_lossy(),
    ))
    .expect("file source");
    let SourceAcquisition::Document(bytes) = acquire_source(&document).expect("file acquire")
    else {
        panic!("file acquisition must supply a document");
    };
    assert_eq!(bytes.body, "{\"price\":7}");

    fs::remove_file(&file).expect("remove source");
    assert_eq!(
        acquire_source(&document),
        Err(SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::FileNotFound,
            status: None,
            raw_platform_error: None,
        }))
    );
}

#[test]
fn configured_body_idle_timeout_is_applied_and_classified_separately_from_total_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("request");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request).expect("read request");
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n")
            .expect("response headers");
        thread::sleep(Duration::from_millis(100));
        let _ = stream.write_all(b"{}");
    });
    let delayed: SourceDocument = toml::from_str(&format!(
        r#"
schema_name = "ffhn.source"
schema_version = 1
source_id = "demo"
display_name = "Demo"
enabled = true
escalate_after = 2
[fetch]
engine = "http"
source_url = "http://{address}/value"
user_agent = "ffhn-test"
accept = "application/json"
max_bytes = 1024
follow_redirects = true
max_redirects = 5
[fetch.timeouts]
connect_ms = 1000
read_idle_ms = 10
total_ms = 500
[conditional]
enabled = true
[schedule]
interval_ms = 1000
min_interval_ms = 1000
"#,
    ))
    .expect("delayed source");
    assert_eq!(
        acquire_source(&delayed),
        Err(SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::ReadTimeout,
            status: None,
            raw_platform_error: None,
        }))
    );
    worker.join().expect("worker");
}

#[test]
fn failure_envelope_enforces_the_total_http_complement_and_raw_error_boundary() {
    for (kind, status) in [
        (SourceFetchFailureKind::HttpStatus, 500),
        (SourceFetchFailureKind::HttpSuccessNotRepresentation, 206),
    ] {
        SourceFetchFailure {
            kind,
            status: Some(status),
            raw_platform_error: None,
        }
        .validate()
        .expect("valid complement member");
    }
    assert!(
        SourceFetchFailure {
            kind: SourceFetchFailureKind::HttpStatus,
            status: Some(206),
            raw_platform_error: None,
        }
        .validate()
        .is_err()
    );
    assert!(
        SourceFetchFailure {
            kind: SourceFetchFailureKind::IoUnclassified,
            status: None,
            raw_platform_error: None,
        }
        .validate()
        .is_err()
    );
    let failure = SourceFetchFailure {
        kind: SourceFetchFailureKind::InvalidUtf8,
        status: None,
        raw_platform_error: None,
    };
    let mut wire = serde_json::to_value(&failure).expect("failure wire");
    assert_eq!(wire["reason_class"], "decode");
    wire["reason_class"] = serde_json::json!("network");
    assert!(serde_json::from_value::<SourceFetchFailure>(wire).is_err());
}

#[test]
fn every_fetch_failure_kind_has_one_closed_reason_and_wire_shape() {
    let cases = [
        (
            SourceFetchFailureKind::DnsError,
            SourceFetchFailureReasonClass::Network,
        ),
        (
            SourceFetchFailureKind::ConnectFailed,
            SourceFetchFailureReasonClass::Network,
        ),
        (
            SourceFetchFailureKind::ConnectionReset,
            SourceFetchFailureReasonClass::Network,
        ),
        (
            SourceFetchFailureKind::ConnectTimeout,
            SourceFetchFailureReasonClass::Timeout,
        ),
        (
            SourceFetchFailureKind::ReadTimeout,
            SourceFetchFailureReasonClass::Timeout,
        ),
        (
            SourceFetchFailureKind::TotalTimeout,
            SourceFetchFailureReasonClass::Timeout,
        ),
        (
            SourceFetchFailureKind::TlsError,
            SourceFetchFailureReasonClass::Tls,
        ),
        (
            SourceFetchFailureKind::IncompleteBody,
            SourceFetchFailureReasonClass::Truncation,
        ),
        (
            SourceFetchFailureKind::TooManyRedirects,
            SourceFetchFailureReasonClass::Redirect,
        ),
        (
            SourceFetchFailureKind::RedirectLoop,
            SourceFetchFailureReasonClass::Redirect,
        ),
        (
            SourceFetchFailureKind::RedirectDowngrade,
            SourceFetchFailureReasonClass::Redirect,
        ),
        (
            SourceFetchFailureKind::ContentLengthExceeded,
            SourceFetchFailureReasonClass::Limit,
        ),
        (
            SourceFetchFailureKind::BodyBytesExceeded,
            SourceFetchFailureReasonClass::Limit,
        ),
        (
            SourceFetchFailureKind::InvalidUtf8,
            SourceFetchFailureReasonClass::Decode,
        ),
        (
            SourceFetchFailureKind::HttpStatus,
            SourceFetchFailureReasonClass::HttpStatus,
        ),
        (
            SourceFetchFailureKind::HttpSuccessNotRepresentation,
            SourceFetchFailureReasonClass::HttpStatus,
        ),
        (
            SourceFetchFailureKind::FileNotFound,
            SourceFetchFailureReasonClass::Filesystem,
        ),
        (
            SourceFetchFailureKind::FilePermissionDenied,
            SourceFetchFailureReasonClass::Filesystem,
        ),
        (
            SourceFetchFailureKind::FileNotRegular,
            SourceFetchFailureReasonClass::Filesystem,
        ),
        (
            SourceFetchFailureKind::FileReadError,
            SourceFetchFailureReasonClass::Filesystem,
        ),
        (
            SourceFetchFailureKind::IoUnclassified,
            SourceFetchFailureReasonClass::Network,
        ),
    ];
    for (kind, reason) in cases {
        let failure = SourceFetchFailure {
            kind,
            status: match kind {
                SourceFetchFailureKind::HttpStatus => Some(500),
                SourceFetchFailureKind::HttpSuccessNotRepresentation => Some(206),
                _ => None,
            },
            raw_platform_error: (kind == SourceFetchFailureKind::IoUnclassified)
                .then(|| "platform".to_owned()),
        };
        assert_eq!(failure.reason_class(), reason);
        assert!(!kind.as_str().is_empty());
        assert!(!reason.as_str().is_empty());
        let wire = serde_json::to_value(&failure).expect("failure wire");
        let decoded: SourceFetchFailure = serde_json::from_value(wire).expect("failure roundtrip");
        assert_eq!(decoded, failure);
    }
    for failure in [
        SourceFetchFailure {
            kind: SourceFetchFailureKind::InvalidUtf8,
            status: Some(500),
            raw_platform_error: None,
        },
        SourceFetchFailure {
            kind: SourceFetchFailureKind::HttpStatus,
            status: None,
            raw_platform_error: None,
        },
        SourceFetchFailure {
            kind: SourceFetchFailureKind::IoUnclassified,
            status: None,
            raw_platform_error: None,
        },
        SourceFetchFailure {
            kind: SourceFetchFailureKind::HttpSuccessNotRepresentation,
            status: Some(200),
            raw_platform_error: None,
        },
        SourceFetchFailure {
            kind: SourceFetchFailureKind::HttpSuccessNotRepresentation,
            status: Some(203),
            raw_platform_error: None,
        },
        SourceFetchFailure {
            kind: SourceFetchFailureKind::HttpStatus,
            status: Some(0),
            raw_platform_error: None,
        },
        SourceFetchFailure {
            kind: SourceFetchFailureKind::HttpStatus,
            status: Some(200),
            raw_platform_error: None,
        },
        SourceFetchFailure {
            kind: SourceFetchFailureKind::HttpSuccessNotRepresentation,
            status: Some(500),
            raw_platform_error: None,
        },
        SourceFetchFailure {
            kind: SourceFetchFailureKind::FileReadError,
            status: None,
            raw_platform_error: Some("foreign".to_owned()),
        },
        SourceFetchFailure {
            kind: SourceFetchFailureKind::IoUnclassified,
            status: None,
            raw_platform_error: Some(String::new()),
        },
    ] {
        assert!(failure.validate().is_err());
        assert!(serde_json::to_value(failure).is_err());
    }
}

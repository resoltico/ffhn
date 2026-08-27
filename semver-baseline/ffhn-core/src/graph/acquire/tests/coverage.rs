use super::*;

#[test]
fn validator_documents_and_low_level_classifiers_cover_every_owned_boundary() {
    let valid = HttpValidators {
        issued_url: Url::parse("https://example.test/value").expect("URL"),
        etag: Some("\"v1\"".to_owned()),
        last_modified: None,
    };
    valid.validate().expect("valid validators");
    for wire in [
        serde_json::json!({"issued_url": "ftp://example.test/a", "etag": "x"}),
        serde_json::json!({"issued_url": "https://user@example.test/a", "etag": "x"}),
        serde_json::json!({"issued_url": "https://example.test/a"}),
        serde_json::json!({"issued_url": "https://example.test/a", "etag": ""}),
        serde_json::json!({"issued_url": "https://example.test/a", "etag": "x", "last_modified": ""}),
        serde_json::json!({"issued_url": "https://example.test/a", "etag": "", "last_modified": "now"}),
        serde_json::json!({"issued_url": "https://:pass@example.test/a", "etag": "x"}),
    ] {
        assert!(serde_json::from_value::<HttpValidators>(wire).is_err());
    }
    let both: HttpValidators = serde_json::from_value(serde_json::json!({
        "issued_url": "https://example.test/a",
        "etag": "x",
        "last_modified": "now"
    }))
    .expect("both validators");
    both.validate().expect("both valid");
    let last_only: HttpValidators = serde_json::from_value(serde_json::json!({
        "issued_url": "https://example.test/a",
        "last_modified": "now"
    }))
    .expect("last-modified validator");
    last_only.validate().expect("last-modified valid");
    let direct = Url::parse("https://example.test/a").expect("URL");
    let other = Url::parse("https://other.test/a").expect("URL");
    assert!(is_direct_response(&direct, &direct, 0));
    assert!(!is_direct_response(&direct, &direct, 1));
    assert!(!is_direct_response(&other, &direct, 0));
    assert!(is_direct_not_modified(304, &direct, &direct, 0));
    assert!(!is_direct_not_modified(200, &direct, &direct, 0));
    assert!(!is_direct_not_modified(304, &other, &direct, 1));

    for (error, kind) in [
        (ureq::Error::HostNotFound, SourceFetchFailureKind::DnsError),
        (
            ureq::Error::ConnectionFailed,
            SourceFetchFailureKind::ConnectFailed,
        ),
        (ureq::Error::Tls("tls"), SourceFetchFailureKind::TlsError),
        (
            ureq::Error::BodyStalled,
            SourceFetchFailureKind::IncompleteBody,
        ),
        (
            ureq::Error::Timeout(ureq::Timeout::Resolve),
            SourceFetchFailureKind::ConnectTimeout,
        ),
        (
            ureq::Error::Timeout(ureq::Timeout::Connect),
            SourceFetchFailureKind::ConnectTimeout,
        ),
        (
            ureq::Error::Timeout(ureq::Timeout::RecvResponse),
            SourceFetchFailureKind::ReadTimeout,
        ),
        (
            ureq::Error::Timeout(ureq::Timeout::RecvBody),
            SourceFetchFailureKind::ReadTimeout,
        ),
        (
            ureq::Error::Timeout(ureq::Timeout::Global),
            SourceFetchFailureKind::TotalTimeout,
        ),
        (
            ureq::Error::Timeout(ureq::Timeout::SendBody),
            SourceFetchFailureKind::TotalTimeout,
        ),
    ] {
        assert!(matches!(
            classify_http_error(error),
            SourceAcquireError::Fetch(SourceFetchFailure { kind: actual, .. }) if actual == kind
        ));
    }
    assert!(matches!(
        classify_http_error(ureq::Error::TooManyRedirects),
        SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::IoUnclassified,
            ..
        })
    ));
    for (kind, expected) in [
        (
            std::io::ErrorKind::UnexpectedEof,
            SourceFetchFailureKind::IncompleteBody,
        ),
        (
            std::io::ErrorKind::ConnectionReset,
            SourceFetchFailureKind::ConnectionReset,
        ),
        (
            std::io::ErrorKind::TimedOut,
            SourceFetchFailureKind::ReadTimeout,
        ),
        (
            std::io::ErrorKind::ConnectionRefused,
            SourceFetchFailureKind::ConnectFailed,
        ),
        (
            std::io::ErrorKind::NotConnected,
            SourceFetchFailureKind::ConnectFailed,
        ),
    ] {
        assert!(matches!(
            classify_http_read_error(std::io::Error::from(kind)),
            SourceAcquireError::Fetch(SourceFetchFailure { kind: actual, .. }) if actual == expected
        ));
    }
    assert!(matches!(
        classify_http_read_error(std::io::Error::other("unclassified")),
        SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::IoUnclassified,
            raw_platform_error: Some(_),
            ..
        })
    ));
    assert!(matches!(
        classify_http_error(ureq::Error::Io(std::io::Error::other("unclassified"))),
        SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::IoUnclassified,
            ..
        })
    ));
    assert!(matches!(
        classify_file_read_error(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::FilePermissionDenied,
            ..
        })
    ));
    assert!(matches!(
        classify_file_read_error(std::io::Error::from(std::io::ErrorKind::Other)),
        SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::FileReadError,
            ..
        })
    ));
    for (kind, expected) in [
        (
            std::io::ErrorKind::NotFound,
            SourceFetchFailureKind::FileNotFound,
        ),
        (
            std::io::ErrorKind::PermissionDenied,
            SourceFetchFailureKind::FilePermissionDenied,
        ),
        (
            std::io::ErrorKind::Other,
            SourceFetchFailureKind::FileReadError,
        ),
    ] {
        assert!(matches!(
            classify_file_metadata_error(std::io::Error::from(kind)),
            SourceAcquireError::Fetch(SourceFetchFailure { kind: actual, .. }) if actual == expected
        ));
    }
    for (kind, expected) in [
        (
            std::io::ErrorKind::PermissionDenied,
            SourceFetchFailureKind::FilePermissionDenied,
        ),
        (
            std::io::ErrorKind::Other,
            SourceFetchFailureKind::FileReadError,
        ),
    ] {
        assert!(matches!(
            classify_file_open_error(std::io::Error::from(kind)),
            SourceAcquireError::Fetch(SourceFetchFailure { kind: actual, .. }) if actual == expected
        ));
    }
    assert!(same_origin(
        &Url::parse("https://example.test/a").expect("left"),
        &Url::parse("https://example.test:443/b").expect("right")
    ));
    assert!(!same_origin(
        &Url::parse("https://example.test/a").expect("left"),
        &Url::parse("http://example.test/a").expect("right")
    ));
}

#[test]
fn bounded_read_file_and_http_response_matrix_classifies_each_complete_representation_failure() {
    assert_eq!(
        read_bounded(
            &mut std::io::Cursor::new(b"abc"),
            3,
            SourceFetchFailureKind::BodyBytesExceeded,
            classify_file_read_error,
        )
        .expect("bounded bytes"),
        b"abc"
    );
    assert!(matches!(
        read_bounded(
            &mut std::io::Cursor::new(b"abcd"),
            3,
            SourceFetchFailureKind::BodyBytesExceeded,
            classify_file_read_error,
        ),
        Err(SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::BodyBytesExceeded,
            ..
        }))
    ));
    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }
    assert!(matches!(
        read_bounded(
            &mut BrokenReader,
            3,
            SourceFetchFailureKind::BodyBytesExceeded,
            classify_file_read_error,
        ),
        Err(SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::FilePermissionDenied,
            ..
        }))
    ));

    for (response, expected) in [
        (
            b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_vec(),
            SourceFetchFailureKind::HttpStatus,
        ),
        (
            b"HTTP/1.1 302 Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
            SourceFetchFailureKind::HttpStatus,
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 2048\r\nConnection: close\r\n\r\n".to_vec(),
            SourceFetchFailureKind::ContentLengthExceeded,
        ),
        (
            [
                b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".as_slice(),
                &vec![b'x'; 1_025],
            ]
            .concat(),
            SourceFetchFailureKind::BodyBytesExceeded,
        ),
        (
            [
                b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n".as_slice(),
                &[0xff],
            ]
            .concat(),
            SourceFetchFailureKind::InvalidUtf8,
        ),
    ] {
        assert!(matches!(
            acquire_raw_response(&response),
            Err(SourceAcquireError::Fetch(SourceFetchFailure { kind, .. })) if kind == expected
        ));
    }

    let temporary = tempfile::tempdir().expect("temporary");
    let directory_source: SourceDocument = toml::from_str(&format!(
        r#"
schema_name = "ffhn.source"
schema_version = 1
source_id = "file"
display_name = "File"
enabled = true
escalate_after = 1
[fetch]
engine = "file"
file_path = {:?}
max_bytes = 1024
[conditional]
enabled = false
[schedule]
interval_ms = 1000
min_interval_ms = 1000
"#,
        temporary.path().to_string_lossy()
    ))
    .expect("directory source");
    assert!(matches!(
        acquire_source(&directory_source),
        Err(SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::FileNotRegular,
            ..
        }))
    ));
    let binary = temporary.path().join("binary");
    fs::write(&binary, [0xff]).expect("binary file");
    let binary_source: SourceDocument = toml::from_str(
        &toml::to_string(&directory_source)
            .expect("source TOML")
            .replace(
                temporary.path().to_string_lossy().as_ref(),
                binary.to_string_lossy().as_ref(),
            ),
    )
    .expect("binary source");
    assert!(matches!(
        acquire_source(&binary_source),
        Err(SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::InvalidUtf8,
            ..
        }))
    ));
}

#[test]
fn missing_header_secret_is_a_source_integration_error_before_transport() {
    let configured = source("http://127.0.0.1:9/value");
    let configured: SourceDocument = toml::from_str(
        &toml::to_string(&configured)
            .expect("source TOML")
            .replace(
                "[conditional]",
                "[fetch.header_secrets]\nAuthorization = { env = \"FFHN_TEST_CERTAINLY_MISSING_SECRET_6FC39A\", format = \"Bearer {value}\", revision = 1 }\n\n[conditional]",
            ),
    )
    .expect("secret source");
    assert_eq!(
        acquire_source(&configured),
        Err(SourceAcquireError::SecretUnavailable)
    );
    let secret = crate::graph::FetchHeaderSecret {
        env: "TOKEN".to_owned(),
        format: "Bearer {value}".to_owned(),
        revision: 1,
    };
    assert_eq!(
        resolve_fetch_secret(&secret, |_| Some("value".to_owned())).expect("secret"),
        "Bearer value"
    );
    assert_eq!(
        resolve_fetch_secret(&secret, |_| None),
        Err(SourceAcquireError::SecretUnavailable)
    );
    assert_eq!(
        resolve_fetch_secret(&secret, |_| Some("line\nbreak".to_owned())),
        Err(SourceAcquireError::SecretUnavailable)
    );

    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("request");
        let mut request = [0_u8; 2048];
        let count = stream.read(&mut request).expect("request");
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .expect("response");
        String::from_utf8_lossy(&request[..count]).into_owned()
    });
    let configured = source(&format!("http://{address}/value"));
    let configured: SourceDocument = toml::from_str(
        &toml::to_string(&configured)
            .expect("source TOML")
            .replace(
                "[conditional]",
                "[fetch.header_secrets]\nAuthorization = { env = \"TOKEN\", format = \"Bearer {value}\", revision = 1 }\n\n[conditional]",
            ),
    )
    .expect("secret source");
    assert!(matches!(
        acquire_source_with_secret_lookup(&configured, None, &|_| Some("secret".to_owned()))
            .expect("acquisition"),
        SourceAcquisition::Document(_)
    ));
    assert!(
        worker
            .join()
            .expect("worker")
            .to_ascii_lowercase()
            .contains("authorization: bearer secret")
    );
}

#[test]
fn redirect_policy_rejects_non_http_loops_and_exhausted_bounds() {
    for (location, expected) in [
        (
            "ftp://example.test/value",
            SourceFetchFailureKind::HttpStatus,
        ),
        ("/value", SourceFetchFailureKind::RedirectLoop),
    ] {
        let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        assert!(matches!(
            acquire_raw_response(response.as_bytes()),
            Err(SourceAcquireError::Fetch(SourceFetchFailure { kind, .. })) if kind == expected
        ));
    }

    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("request");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request).expect("request");
        stream
            .write_all(
                b"HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("response");
    });
    let exhausted: SourceDocument = toml::from_str(
        &toml::to_string(&source(&format!("http://{address}/start")))
            .expect("source TOML")
            .replace("max_redirects = 5", "max_redirects = 0"),
    )
    .expect("exhausted source");
    assert!(matches!(
        acquire_source(&exhausted),
        Err(SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::TooManyRedirects,
            ..
        }))
    ));
    worker.join().expect("worker");

    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("request");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request).expect("request");
        stream
            .write_all(
                b"HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("response");
    });
    let no_follow: SourceDocument = toml::from_str(
        &toml::to_string(&source(&format!("http://{address}/start")))
            .expect("source TOML")
            .replace("follow_redirects = true", "follow_redirects = false"),
    )
    .expect("no-follow source");
    assert!(matches!(
        acquire_source(&no_follow),
        Err(SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::HttpStatus,
            status: Some(302),
            ..
        }))
    ));
    worker.join().expect("worker");

    let https = Url::parse("https://example.test/start").expect("HTTPS");
    let http = Url::parse("http://example.test/next").expect("HTTP");
    assert!(matches!(
        validate_redirect(
            &https,
            &http,
            &mut BTreeSet::from([https.to_string()]),
            0,
            5,
            302,
        ),
        Err(SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::RedirectDowngrade,
            ..
        }))
    ));
    let cross = Url::parse("https://other.test/next").expect("cross origin");
    assert!(
        !validate_redirect(
            &https,
            &cross,
            &mut BTreeSet::from([https.to_string()]),
            0,
            5,
            302,
        )
        .expect("cross-origin redirect")
    );
}

#[test]
fn conditional_request_supports_last_modified_without_an_etag() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("request");
        let mut request = [0_u8; 2048];
        let count = stream.read(&mut request).expect("request");
        stream
            .write_all(
                b"HTTP/1.1 304 Not Modified\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("response");
        String::from_utf8_lossy(&request[..count]).into_owned()
    });
    let configured = source(&format!("http://{address}/value"));
    let validators = HttpValidators {
        issued_url: match configured.fetch() {
            SourceFetch::Http { source_url, .. } => source_url.clone(),
            SourceFetch::File { .. } => panic!("HTTP source"),
        },
        etag: None,
        last_modified: Some("Wed, 21 Oct 2015 07:28:00 GMT".to_owned()),
    };
    assert!(matches!(
        acquire_source_with_validators(&configured, Some(&validators)).expect("304"),
        SourceAcquisition::NotModified(_)
    ));
    let request = worker.join().expect("worker").to_ascii_lowercase();
    assert!(request.contains("if-modified-since:"));
    assert!(!request.contains("if-none-match:"));
}

#[cfg(unix)]
#[test]
fn file_acquisition_distinguishes_permission_failures_from_other_file_errors() {
    use std::os::unix::fs::PermissionsExt;

    let temporary = tempfile::tempdir().expect("temporary");
    let file = temporary.path().join("private.json");
    fs::write(&file, "{}").expect("file");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).expect("permissions");
    assert!(matches!(
        acquire_file(file.to_string_lossy().as_ref(), 1024),
        Err(SourceAcquireError::Fetch(SourceFetchFailure {
            kind: SourceFetchFailureKind::FilePermissionDenied,
            ..
        }))
    ));
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).expect("restore");
}

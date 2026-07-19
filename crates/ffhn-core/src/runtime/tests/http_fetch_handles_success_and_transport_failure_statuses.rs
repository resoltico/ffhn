use super::support::*;

#[test]
fn http_fetch_handles_success_and_transport_failure_statuses() {
    assert_eq!(
        fetch_http_from_once("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}", 10)
            .expect("HTTP body"),
        "{}"
    );
    let non_success = fetch_http_from_once(
        "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n",
        10,
    )
    .expect_err("HTTP status must be a typed fetch failure");
    assert_eq!(
        non_success.fetch_failure(),
        Some(&FetchFailureDetails::HttpStatus { status: 503 })
    );
    assert_eq!(
        non_success.message(),
        "the HTTP response returned a non-success status"
    );
    let content_length = fetch_http_from_once("HTTP/1.1 200 OK\r\nContent-Length: 99\r\n\r\n", 10)
        .expect_err("advertised bound must be typed");
    assert_eq!(
        content_length.fetch_failure(),
        Some(&FetchFailureDetails::HttpContentLengthExceeded {
            configured_max_bytes: 10,
            content_length: 99,
        })
    );
    let body_limit = fetch_http_from_once("HTTP/1.1 200 OK\r\n\r\n01234567890", 10)
        .expect_err("streamed bound must be typed");
    assert_eq!(
        body_limit.fetch_failure(),
        Some(&FetchFailureDetails::BodyBytesExceeded {
            configured_max_bytes: 10,
            observed_bytes: 11,
        })
    );
    assert!(fetch_http_from_once("HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\n{}", 10).is_err());
    let invalid_utf8 = fetch_http_bytes_from_once(
        b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n\xff".to_vec(),
        10,
    )
    .expect_err("HTTP UTF-8 failure must be typed");
    assert_eq!(
        invalid_utf8.fetch_failure(),
        Some(&FetchFailureDetails::InvalidUtf8)
    );
}

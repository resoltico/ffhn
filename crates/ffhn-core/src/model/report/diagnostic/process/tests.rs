use super::*;

use base64::engine::general_purpose::STANDARD;

impl StderrCapture {
    pub(crate) fn from_bytes(
        bytes: Vec<u8>,
        original_len_bytes: ExactByteCount,
    ) -> Result<Self, CoreError> {
        let truncated = original_len_bytes.compare_usize(bytes.len()).is_gt();
        let capture = Self {
            retained_bytes_base64: bytes,
            original_len_bytes,
            truncated,
        };
        capture.validate()?;
        Ok(capture)
    }
}

#[test]
fn stderr_capture_preserves_lossy_input_and_rejects_every_incoherent_durable_fact() {
    let lossy = StderrCapture::from_bytes(vec![0xff], ExactByteCount::from_usize(1))
        .expect("valid lossy capture");
    assert_eq!(lossy.encoding(), StderrEncoding::Utf8Lossy);
    assert_eq!(lossy.text(), "�");
    assert_eq!(
        serde_json::to_value(&lossy).expect("stderr capture JSON")["retained_bytes_base64"],
        "/w=="
    );

    let oversized = StderrCapture {
        retained_bytes_base64: vec![b'x'; STDERR_RETAINED_BYTES_LIMIT + 1],
        original_len_bytes: ExactByteCount::from_usize(STDERR_RETAINED_BYTES_LIMIT + 1),
        truncated: false,
    };
    assert!(oversized.validate().is_err());

    let too_short = StderrCapture {
        retained_bytes_base64: b"text".to_vec(),
        original_len_bytes: ExactByteCount::from_usize(3),
        truncated: true,
    };
    assert!(too_short.validate().is_err());

    let unmarked_truncation = StderrCapture {
        retained_bytes_base64: b"text".to_vec(),
        original_len_bytes: ExactByteCount::from_usize(5),
        truncated: false,
    };
    assert!(unmarked_truncation.validate().is_err());

    let invented_truncation = StderrCapture {
        retained_bytes_base64: b"x".to_vec(),
        original_len_bytes: ExactByteCount::from_usize(1),
        truncated: true,
    };
    assert!(invented_truncation.validate().is_err());

    let invented_encoding = serde_json::json!({
        "retained_bytes_base64": "eA==",
        "encoding": "utf8_lossy",
        "original_len_bytes": "1",
        "truncated": false,
    });
    assert!(serde_json::from_value::<StderrCapture>(invented_encoding).is_err());

    let malformed_retained_bytes = serde_json::json!({
        "retained_bytes_base64": "not valid base64",
        "original_len_bytes": "1",
        "truncated": false,
    });
    assert!(serde_json::from_value::<StderrCapture>(malformed_retained_bytes).is_err());

    let impossible_process_attempt = serde_json::json!({
        "terminal": { "kind": "exited", "exit_code": 0 },
        "writer": { "kind": "not_attempted" },
        "stderr": { "kind": "absent" },
    });
    assert!(serde_json::from_value::<DeliveryProcessAttempt>(impossible_process_attempt).is_err());

    assert!(
        DeliveryProcessAttempt::new(
            TerminalOutcome::TimedOut { timeout_ms: 0 },
            WriterOutcome::Completed,
            StderrOutcome::captured(
                StderrCapture::from_bytes(Vec::new(), ExactByteCount::zero())
                    .expect("empty capture"),
            ),
        )
        .validate()
        .is_err()
    );
}

#[test]
fn stderr_outcomes_expose_and_shorten_each_retained_capture_variant() {
    let captured =
        StderrCapture::from_bytes("é".as_bytes().to_vec(), ExactByteCount::from_usize(2))
            .expect("captured UTF-8 bytes");
    let mut outcome = StderrOutcome::captured(captured);
    assert!(outcome.shorten_one_byte());
    assert!(outcome.shorten_one_byte());
    assert!(!outcome.shorten_one_byte());
    assert!(outcome.capture_problem().is_none());

    let mut failed = StderrOutcome::read_failed(
        IoErrorClass::BrokenPipe,
        StderrCapture::from_bytes("x".as_bytes().to_vec(), ExactByteCount::from_usize(1))
            .expect("captured byte"),
    );
    assert!(failed.shorten_one_byte());
    assert!(matches!(
        failed.capture_problem(),
        Some(StderrCaptureProblem::ReadFailed { .. })
    ));

    for outcome in [
        StderrOutcome::Absent,
        StderrOutcome::ReaderUnavailable,
        StderrOutcome::ReaderPanicked,
    ] {
        assert!(!outcome.clone().shorten_one_byte());
    }
    assert!(matches!(
        StderrOutcome::ReaderUnavailable.capture_problem(),
        Some(StderrCaptureProblem::ReaderUnavailable)
    ));
    assert!(matches!(
        StderrOutcome::ReaderPanicked.capture_problem(),
        Some(StderrCaptureProblem::ReaderPanicked)
    ));
}

#[test]
fn exact_byte_count_remains_true_after_crossing_the_platform_usize_limit() {
    let mut count = ExactByteCount::from_usize(usize::MAX);
    count.add_usize(1);
    let expected = ((usize::MAX as u128) + 1).to_string();
    assert_eq!(count.as_decimal(), expected);

    let capture = StderrCapture::from_bytes(Vec::new(), count).expect("exact capture");
    let wire = serde_json::to_value(&capture).expect("capture JSON");
    assert_eq!(wire["original_len_bytes"], expected);
    assert!(wire["truncated"].as_bool().expect("truncated flag"));
    assert!(serde_json::from_value::<StderrCapture>(wire).is_ok());
}

#[test]
fn accumulator_constructs_only_complete_bounded_capture_evidence() {
    let mut accumulator = StderrCapture::accumulator();
    accumulator.record(b"abc");
    accumulator.record(&vec![b'x'; STDERR_RETAINED_BYTES_LIMIT]);
    let capture = accumulator.finish();

    assert_eq!(capture.original_len_bytes().as_decimal(), "2051");
    assert_eq!(capture.text().len(), STDERR_RETAINED_BYTES_LIMIT);
    assert!(capture.truncated());
    assert!(capture.validate().is_ok());
}

#[test]
fn retained_utf8_boundary_is_distinguished_without_discarding_the_raw_byte_prefix() {
    let source = [
        vec![b'x'; STDERR_RETAINED_BYTES_LIMIT - 1],
        "é".repeat(50).into_bytes(),
    ]
    .concat();
    let mut accumulator = StderrCapture::accumulator();
    accumulator.record(&source[..1_024]);
    accumulator.record(&source[1_024..STDERR_RETAINED_BYTES_LIMIT]);
    accumulator.record(&source[STDERR_RETAINED_BYTES_LIMIT..]);
    let capture = accumulator.finish();

    assert_eq!(capture.original_len_bytes().as_decimal(), "2147");
    assert!(capture.truncated());
    assert_eq!(
        capture.encoding(),
        StderrEncoding::Utf8IncompleteAtRetentionBoundary
    );
    assert_eq!(capture.text(), "x".repeat(STDERR_RETAINED_BYTES_LIMIT - 1));
    assert!(!capture.text().contains('\u{fffd}'));
    assert_eq!(
        serde_json::to_value(&capture).expect("stderr capture JSON")["retained_bytes_base64"],
        STANDARD.encode(&source[..STDERR_RETAINED_BYTES_LIMIT])
    );
    assert!(capture.validate().is_ok());

    let malformed_source = [
        vec![b'x'; STDERR_RETAINED_BYTES_LIMIT - 1],
        vec![0xff, b'y'],
    ]
    .concat();
    let mut malformed_accumulator = StderrCapture::accumulator();
    malformed_accumulator.record(&malformed_source);
    let malformed_capture = malformed_accumulator.finish();
    assert!(malformed_capture.truncated());
    assert_eq!(malformed_capture.encoding(), StderrEncoding::Utf8Lossy);
    assert!(malformed_capture.text().ends_with('\u{fffd}'));
}

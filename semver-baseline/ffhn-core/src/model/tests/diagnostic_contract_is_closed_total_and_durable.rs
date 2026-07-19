use super::support::*;
use crate::{CoreError, HtmlcutErrorClass};

fn captured() -> StderrOutcome {
    StderrOutcome::captured(
        StderrCapture::from_bytes(Vec::new(), ExactByteCount::zero()).expect("empty capture"),
    )
}

fn failed_delivery_detail() -> DiagnosticDetail {
    crate::model::delivery_failure_detail(DeliveryProcessAttempt::new(
        TerminalOutcome::Exited { exit_code: Some(1) },
        WriterOutcome::Completed,
        captured(),
    ))
    .expect("failed delivery detail")
}

fn outbox_detail() -> DiagnosticDetail {
    io_detail(
        IoErrorClass::StorageFull,
        DiagnosticOperation::OutboxStateCommit,
        "state write failed",
        None,
    )
}

fn observability_detail() -> DiagnosticDetail {
    crate::model::delivery_observability_detail(StderrCaptureProblem::ReaderPanicked)
}

fn outcome(
    status: DeliveryStatus,
    has_error: bool,
    has_outbox_error: bool,
    observability: Option<DiagnosticDetail>,
) -> Result<DeliveryOutcome, CoreError> {
    DeliveryOutcome::new(
        "event".to_owned(),
        "route".to_owned(),
        DeliveryEventKind::Initialized,
        None,
        status,
        1,
        has_error.then(failed_delivery_detail),
        has_outbox_error.then(outbox_detail),
        observability,
    )
}

#[test]
fn delivery_statuses_accept_exactly_six_of_the_twenty_four_detail_presence_products() {
    let expected = [
        (DeliveryStatus::Delivered, false, false),
        (DeliveryStatus::RetryScheduled, true, false),
        (DeliveryStatus::DeadLettered, true, false),
        (DeliveryStatus::DeliveredUncommitted, false, true),
        (DeliveryStatus::RetryUncommitted, true, true),
        (DeliveryStatus::DeadLetterUncommitted, true, true),
    ];
    let statuses = [
        DeliveryStatus::Delivered,
        DeliveryStatus::RetryScheduled,
        DeliveryStatus::DeadLettered,
        DeliveryStatus::DeliveredUncommitted,
        DeliveryStatus::RetryUncommitted,
        DeliveryStatus::DeadLetterUncommitted,
    ];

    let mut accepted = 0;
    let mut rejected = 0;
    for status in statuses {
        for has_error in [false, true] {
            for has_outbox_error in [false, true] {
                let expected_valid = expected.contains(&(status, has_error, has_outbox_error));
                let actual_valid = outcome(status, has_error, has_outbox_error, None).is_ok();
                assert_eq!(
                    actual_valid, expected_valid,
                    "{status:?} with error={has_error}, outbox_error={has_outbox_error}"
                );
                if actual_valid {
                    accepted += 1;
                } else {
                    rejected += 1;
                }
            }
        }
    }
    assert_eq!((accepted, rejected), (6, 18));
}

#[test]
fn delivery_outcomes_reject_a_zero_attempt_count() {
    assert!(
        DeliveryOutcome::new(
            "event".to_owned(),
            "route".to_owned(),
            DeliveryEventKind::Initialized,
            None,
            DeliveryStatus::Delivered,
            0,
            None,
            None,
            None,
        )
        .is_err()
    );
}

#[test]
fn process_attempt_primary_is_total_for_every_legal_terminal_writer_product() {
    let cases = [
        (
            TerminalOutcome::NotStarted {
                io: IoErrorClass::NotFound,
            },
            WriterOutcome::NotAttempted,
            StderrOutcome::Absent,
            Some(DeliveryFailurePrimary::SpawnFailed),
        ),
        (
            TerminalOutcome::StdinUnavailable,
            WriterOutcome::NotAttempted,
            StderrOutcome::Absent,
            Some(DeliveryFailurePrimary::StdinUnavailable),
        ),
        (
            TerminalOutcome::Exited { exit_code: Some(0) },
            WriterOutcome::Completed,
            captured(),
            None,
        ),
        (
            TerminalOutcome::Exited { exit_code: Some(0) },
            WriterOutcome::IoFailed {
                io: IoErrorClass::BrokenPipe,
            },
            captured(),
            Some(DeliveryFailurePrimary::WriterIoFailed),
        ),
        (
            TerminalOutcome::Exited { exit_code: Some(0) },
            WriterOutcome::Panicked,
            captured(),
            Some(DeliveryFailurePrimary::WriterPanicked),
        ),
        (
            TerminalOutcome::Exited { exit_code: None },
            WriterOutcome::Completed,
            captured(),
            Some(DeliveryFailurePrimary::UnsuccessfulExit),
        ),
        (
            TerminalOutcome::Exited { exit_code: None },
            WriterOutcome::IoFailed {
                io: IoErrorClass::BrokenPipe,
            },
            captured(),
            Some(DeliveryFailurePrimary::WriterIoFailed),
        ),
        (
            TerminalOutcome::Exited { exit_code: None },
            WriterOutcome::Panicked,
            captured(),
            Some(DeliveryFailurePrimary::WriterPanicked),
        ),
        (
            TerminalOutcome::Exited { exit_code: Some(9) },
            WriterOutcome::Completed,
            captured(),
            Some(DeliveryFailurePrimary::UnsuccessfulExit),
        ),
        (
            TerminalOutcome::Exited { exit_code: Some(9) },
            WriterOutcome::IoFailed {
                io: IoErrorClass::BrokenPipe,
            },
            captured(),
            Some(DeliveryFailurePrimary::WriterIoFailed),
        ),
        (
            TerminalOutcome::Exited { exit_code: Some(9) },
            WriterOutcome::Panicked,
            captured(),
            Some(DeliveryFailurePrimary::WriterPanicked),
        ),
        (
            TerminalOutcome::TimedOut { timeout_ms: 10 },
            WriterOutcome::Completed,
            captured(),
            Some(DeliveryFailurePrimary::TimedOut),
        ),
        (
            TerminalOutcome::TimedOut { timeout_ms: 10 },
            WriterOutcome::IoFailed {
                io: IoErrorClass::BrokenPipe,
            },
            captured(),
            Some(DeliveryFailurePrimary::TimedOut),
        ),
        (
            TerminalOutcome::TimedOut { timeout_ms: 10 },
            WriterOutcome::Panicked,
            captured(),
            Some(DeliveryFailurePrimary::TimedOut),
        ),
        (
            TerminalOutcome::WaitFailed {
                io: IoErrorClass::Interrupted,
            },
            WriterOutcome::Completed,
            captured(),
            Some(DeliveryFailurePrimary::WaitFailed),
        ),
        (
            TerminalOutcome::WaitFailed {
                io: IoErrorClass::Interrupted,
            },
            WriterOutcome::IoFailed {
                io: IoErrorClass::BrokenPipe,
            },
            captured(),
            Some(DeliveryFailurePrimary::WaitFailed),
        ),
        (
            TerminalOutcome::WaitFailed {
                io: IoErrorClass::Interrupted,
            },
            WriterOutcome::Panicked,
            captured(),
            Some(DeliveryFailurePrimary::WaitFailed),
        ),
    ];

    for (terminal, writer, stderr, expected_primary) in cases {
        let attempt = DeliveryProcessAttempt::new(terminal, writer, stderr);
        attempt.validate().expect("legal process attempt");
        assert_eq!(attempt.primary(), expected_primary);
        assert_eq!(attempt.is_success(), expected_primary.is_none());
    }
}

#[test]
fn process_attempt_rejects_every_forbidden_reader_and_writer_boundary_combination() {
    for terminal in [
        TerminalOutcome::Exited { exit_code: Some(0) },
        TerminalOutcome::Exited { exit_code: None },
        TerminalOutcome::Exited { exit_code: Some(9) },
        TerminalOutcome::TimedOut { timeout_ms: 10 },
        TerminalOutcome::WaitFailed {
            io: IoErrorClass::Interrupted,
        },
    ] {
        assert!(
            DeliveryProcessAttempt::new(terminal.clone(), WriterOutcome::NotAttempted, captured(),)
                .validate()
                .is_err()
        );
        for writer in [
            WriterOutcome::Completed,
            WriterOutcome::IoFailed {
                io: IoErrorClass::BrokenPipe,
            },
            WriterOutcome::Panicked,
        ] {
            assert!(
                DeliveryProcessAttempt::new(terminal.clone(), writer, StderrOutcome::Absent)
                    .validate()
                    .is_err()
            );
        }
    }
    for terminal in [
        TerminalOutcome::NotStarted {
            io: IoErrorClass::NotFound,
        },
        TerminalOutcome::StdinUnavailable,
    ] {
        for writer in [
            WriterOutcome::Completed,
            WriterOutcome::IoFailed {
                io: IoErrorClass::BrokenPipe,
            },
            WriterOutcome::Panicked,
        ] {
            assert!(
                DeliveryProcessAttempt::new(terminal.clone(), writer, StderrOutcome::Absent)
                    .validate()
                    .is_err()
            );
        }
        for stderr in [
            captured(),
            StderrOutcome::read_failed(
                IoErrorClass::BrokenPipe,
                StderrCapture::from_bytes(Vec::new(), ExactByteCount::zero())
                    .expect("empty capture"),
            ),
            StderrOutcome::ReaderUnavailable,
            StderrOutcome::ReaderPanicked,
        ] {
            assert!(
                DeliveryProcessAttempt::new(terminal.clone(), WriterOutcome::NotAttempted, stderr,)
                    .validate()
                    .is_err()
            );
        }
    }
}

#[test]
fn started_attempts_retain_each_distinct_stderr_observability_fact() {
    for stderr in [
        StderrOutcome::read_failed(
            IoErrorClass::BrokenPipe,
            StderrCapture::from_bytes(b"partial".to_vec(), ExactByteCount::from_usize(7))
                .expect("partial capture"),
        ),
        StderrOutcome::ReaderUnavailable,
        StderrOutcome::ReaderPanicked,
    ] {
        let attempt = DeliveryProcessAttempt::new(
            TerminalOutcome::Exited { exit_code: Some(0) },
            WriterOutcome::Completed,
            stderr,
        );
        attempt.validate().expect("started attempt is valid");
        assert!(attempt.is_success());
        assert!(attempt.stderr().capture_problem().is_some());
    }
}

#[test]
fn diagnostic_deserialization_rejects_invented_derivations_and_illegal_observability_placement() {
    let mut malformed = serde_json::to_value(failed_delivery_detail()).expect("detail JSON");
    malformed["delivery_process"]["primary"] = serde_json::json!("timed_out");
    assert!(serde_json::from_value::<DiagnosticDetail>(malformed).is_err());

    let mut malformed = serde_json::to_value(failed_delivery_detail()).expect("detail JSON");
    malformed["io_error_class"] = serde_json::json!("permission_denied");
    assert!(serde_json::from_value::<DiagnosticDetail>(malformed).is_err());

    let mut malformed = serde_json::to_value(htmlcut_detail(
        "no match",
        HtmlcutFailureDetails::new(HtmlcutErrorClass::NoMatch, None, "a".repeat(64), Vec::new()),
        None,
    ))
    .expect("detail JSON");
    malformed["htmlcut_failure"]["diagnostics"] = serde_json::json!([{
        "level": "info",
        "code": "INVENTED",
        "message": "invented HTMLCut diagnostic"
    }]);
    assert!(serde_json::from_value::<DiagnosticDetail>(malformed).is_err());

    let observability = observability_detail();
    assert!(
        outcome(
            DeliveryStatus::Delivered,
            false,
            false,
            Some(observability.clone())
        )
        .is_ok()
    );
    assert!(
        outcome(
            DeliveryStatus::DeliveredUncommitted,
            false,
            true,
            Some(observability)
        )
        .is_ok()
    );
    assert!(
        outcome(
            DeliveryStatus::RetryScheduled,
            true,
            false,
            Some(observability_detail())
        )
        .is_err()
    );
    assert!(outcome(DeliveryStatus::Delivered, true, false, None).is_err());
}

#[test]
fn diagnostics_reject_evidence_and_operations_that_do_not_share_a_semantic_owner() {
    let mut htmlcut = serde_json::to_value(htmlcut_detail(
        "no match",
        HtmlcutFailureDetails::new(HtmlcutErrorClass::NoMatch, None, "a".repeat(64), Vec::new()),
        None,
    ))
    .expect("HTMLCut diagnostic JSON");
    htmlcut["operation"] = serde_json::json!("target_load");
    assert!(serde_json::from_value::<DiagnosticDetail>(htmlcut).is_err());

    let evidence_free_htmlcut = serde_json::json!({
        "kind": "htmlcut",
        "operation": "html_extraction",
        "message": "HTMLCut extraction failed"
    });
    assert!(serde_json::from_value::<DiagnosticDetail>(evidence_free_htmlcut).is_err());

    let mut integration = serde_json::to_value(htmlcut_detail(
        "HTMLCut boundary invariant failed",
        HtmlcutFailureDetails::new(
            HtmlcutErrorClass::FfhnBoundaryInvariantViolation,
            None,
            "a".repeat(64),
            Vec::new(),
        ),
        Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation),
    ))
    .expect("integration diagnostic JSON");
    integration["integration_fault_code"] = serde_json::json!("ffhn_policy_invariant_violation");
    assert!(serde_json::from_value::<DiagnosticDetail>(integration).is_err());

    let evidence_free_policy_invariant = serde_json::json!({
        "kind": "policy_invariant",
        "operation": "policy_evaluation",
        "message": "fixed-width proof failed"
    });
    assert!(serde_json::from_value::<DiagnosticDetail>(evidence_free_policy_invariant).is_err());
}

#[test]
fn foreign_errors_become_owned_payloads_without_rendered_foreign_prose() {
    let json_error = CoreError::from(
        serde_json::from_str::<serde_json::Value>("{").expect_err("invalid JSON input"),
    );
    let toml_error =
        CoreError::from(toml::from_str::<toml::Value>("=").expect_err("invalid TOML input"));
    let url_error = CoreError::from(url::Url::parse("://not-a-url").expect_err("invalid URL"));
    let time_error = CoreError::from(
        time::OffsetDateTime::parse(
            "not-a-timestamp",
            &time::format_description::well_known::Rfc3339,
        )
        .expect_err("invalid timestamp"),
    );
    let io_error = CoreError::io(
        "/not-a-real-path",
        std::io::Error::from(std::io::ErrorKind::BrokenPipe),
    );

    let cases = [
        (
            json_error,
            DiagnosticKind::Json,
            "the JSON document could not be decoded or encoded",
        ),
        (
            toml_error,
            DiagnosticKind::Toml,
            "the TOML document could not be decoded",
        ),
        (
            url_error,
            DiagnosticKind::Contract,
            "a required URL could not be interpreted",
        ),
        (
            time_error,
            DiagnosticKind::Contract,
            "a required timestamp could not be interpreted",
        ),
        (
            io_error,
            DiagnosticKind::Io,
            "the operating-system I/O operation did not complete",
        ),
    ];
    for (error, kind, message) in cases {
        let detail =
            crate::model::detail_from_core_error(&error, DiagnosticOperation::StateLoad, None);
        assert_eq!(detail.kind(), kind);
        assert_eq!(detail.message(), message);
    }
}

#[test]
fn durable_delivery_detail_preserves_bounded_binary_stderr_without_losing_true_byte_count() {
    let detail = crate::model::delivery_failure_detail(DeliveryProcessAttempt::new(
        TerminalOutcome::Exited { exit_code: Some(1) },
        WriterOutcome::Completed,
        StderrOutcome::captured(
            StderrCapture::from_bytes(vec![1; 2_048], ExactByteCount::from_usize(2_048))
                .expect("bounded capture"),
        ),
    ))
    .expect("fitted failure detail");
    detail
        .validate_durable_delivery_failure()
        .expect("durable detail");
    assert!(
        crate::stable_json::stable_json(&detail)
            .expect("stable JSON")
            .len()
            <= 4_096
    );
    assert!(matches!(
        detail.delivery_failure_attempt().expect("attempt").stderr(),
        StderrOutcome::Captured { capture }
            if !capture.truncated() && capture.original_len_bytes().as_decimal() == "2048"
    ));
}

#[test]
fn htmlcut_boundary_preserves_an_exactly_1024_byte_message_and_never_sanitizes_an_oversized_one() {
    let detail = htmlcut_detail(
        "x".repeat(1_024),
        HtmlcutFailureDetails::new(
            HtmlcutErrorClass::NoMatch,
            Some(0),
            "a".repeat(64),
            Vec::new(),
        ),
        None,
    );
    assert_eq!(detail.message(), "x".repeat(1_024));
    detail.validate().expect("bounded HTMLCut detail");

    let oversized = htmlcut_detail(
        "x".repeat(1_025),
        HtmlcutFailureDetails::new(
            HtmlcutErrorClass::NoMatch,
            Some(0),
            "a".repeat(64),
            Vec::new(),
        ),
        None,
    );
    assert_eq!(oversized.message(), "x".repeat(1_025));
    assert!(oversized.validate().is_err());
}

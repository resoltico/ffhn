use crate::{HtmlcutErrorClass, IntegrationFaultCode, IoErrorClass};

use super::super::{
    DeliveryFailurePrimary, DeliveryProcessAttempt, ExactByteCount, HtmlcutFailureDetails,
    StderrCapture, StderrCaptureProblem, StderrOutcome, TerminalOutcome, WriterOutcome,
};
use super::construction::{
    delivery_failure_detail, delivery_observability_detail, fetch_detail, htmlcut_detail,
    io_detail, plain_detail, unvalidated_detail_for_contract_test,
    unvalidated_fetch_detail_for_contract_test,
};
use super::*;

fn failed_delivery_detail() -> DiagnosticDetail {
    delivery_failure_detail(DeliveryProcessAttempt::new(
        TerminalOutcome::Exited { exit_code: Some(1) },
        WriterOutcome::Completed,
        StderrOutcome::captured(
            StderrCapture::from_bytes(Vec::new(), ExactByteCount::zero()).expect("empty capture"),
        ),
    ))
    .expect("failed delivery detail")
}

fn valid_htmlcut_failure() -> HtmlcutFailureDetails {
    HtmlcutFailureDetails::new(HtmlcutErrorClass::NoMatch, None, "a".repeat(64), Vec::new())
}

fn unvalidated_detail(
    kind: DiagnosticKind,
    operation: DiagnosticOperation,
    htmlcut_failure: Option<HtmlcutFailureDetails>,
    io_error_class: Option<IoErrorClass>,
    integration_fault_code: Option<IntegrationFaultCode>,
    delivery_process: Option<DeliveryProcessDetail>,
) -> DiagnosticDetail {
    unvalidated_detail_for_contract_test(
        kind,
        operation,
        "test diagnostic".to_owned(),
        None,
        io_error_class,
        htmlcut_failure.map(Box::new),
        integration_fault_code,
        delivery_process,
    )
}

#[test]
fn closed_kind_and_operation_vocabularies_have_one_stable_spelling_each() {
    let kinds = [
        (DiagnosticKind::Contract, "contract"),
        (DiagnosticKind::Io, "io"),
        (DiagnosticKind::Json, "json"),
        (DiagnosticKind::Htmlcut, "htmlcut"),
        (DiagnosticKind::Toml, "toml"),
        (DiagnosticKind::ValueUnparseable, "value_unparseable"),
        (DiagnosticKind::PolicyInvariant, "policy_invariant"),
        (DiagnosticKind::Delivery, "delivery"),
    ];
    for (kind, spelling) in kinds {
        assert_eq!(kind.as_str(), spelling);
    }

    let operations = [
        (DiagnosticOperation::TargetLoad, "target_load"),
        (DiagnosticOperation::TargetValidation, "target_validation"),
        (DiagnosticOperation::LockAcquire, "lock_acquire"),
        (DiagnosticOperation::StateLoad, "state_load"),
        (DiagnosticOperation::StateCommit, "state_commit"),
        (DiagnosticOperation::HttpFetch, "http_fetch"),
        (DiagnosticOperation::FileRead, "file_read"),
        (
            DiagnosticOperation::JsonPointerSelection,
            "json_pointer_selection",
        ),
        (DiagnosticOperation::HtmlExtraction, "html_extraction"),
        (DiagnosticOperation::ValueParse, "value_parse"),
        (DiagnosticOperation::PolicyEvaluation, "policy_evaluation"),
        (DiagnosticOperation::DeliveryProcess, "delivery_process"),
        (DiagnosticOperation::OutboxDrain, "outbox_drain"),
        (
            DiagnosticOperation::OutboxStateCommit,
            "outbox_state_commit",
        ),
    ];
    for (operation, spelling) in operations {
        assert_eq!(operation.as_str(), spelling);
    }
}

#[test]
fn sha256_validation_accepts_each_lowercase_hex_class_and_rejects_other_lengths_or_bytes() {
    assert!(is_sha256(&"0".repeat(64)));
    assert!(is_sha256(&"a".repeat(64)));
    assert!(!is_sha256(&"a".repeat(63)));
    assert!(!is_sha256(&"g".repeat(64)));
}

#[test]
fn diagnostic_validation_rejects_each_misowned_evidence_carrier() {
    for message in [String::new(), "x".repeat(DIAGNOSTIC_MESSAGE_LIMIT + 1)] {
        assert!(
            unvalidated_detail_for_contract_test(
                DiagnosticKind::Contract,
                DiagnosticOperation::TargetLoad,
                message,
                None,
                None,
                None,
                None,
                None,
            )
            .validate()
            .is_err()
        );
    }

    let missing_htmlcut = unvalidated_detail(
        DiagnosticKind::Htmlcut,
        DiagnosticOperation::HtmlExtraction,
        None,
        None,
        None,
        None,
    );
    assert!(missing_htmlcut.validate().is_err());

    let ownerless_htmlcut = unvalidated_detail(
        DiagnosticKind::Htmlcut,
        DiagnosticOperation::TargetLoad,
        None,
        None,
        None,
        None,
    );
    assert!(ownerless_htmlcut.validate().is_err());

    let foreign_htmlcut = unvalidated_detail(
        DiagnosticKind::Contract,
        DiagnosticOperation::TargetLoad,
        Some(valid_htmlcut_failure()),
        None,
        None,
        None,
    );
    assert!(foreign_htmlcut.validate().is_err());

    let foreign_io = unvalidated_detail(
        DiagnosticKind::Contract,
        DiagnosticOperation::TargetLoad,
        None,
        Some(IoErrorClass::BrokenPipe),
        None,
        None,
    );
    assert!(foreign_io.validate().is_err());

    let evidence_free_io = unvalidated_detail(
        DiagnosticKind::Io,
        DiagnosticOperation::HttpFetch,
        None,
        None,
        None,
        None,
    );
    assert!(evidence_free_io.validate().is_err());

    let mut mixed_fetch_and_native_io = serde_json::to_value(io_detail(
        IoErrorClass::ConnectionRefused,
        DiagnosticOperation::HttpFetch,
        "test diagnostic",
        None,
    ))
    .expect("native I/O diagnostic JSON");
    mixed_fetch_and_native_io["fetch_failure"] =
        serde_json::json!({ "kind": "http_status", "status": 503 });
    assert!(serde_json::from_value::<DiagnosticDetail>(mixed_fetch_and_native_io).is_err());

    let missing_delivery_evidence = unvalidated_detail(
        DiagnosticKind::Delivery,
        DiagnosticOperation::DeliveryProcess,
        None,
        None,
        None,
        None,
    );
    assert!(missing_delivery_evidence.validate().is_err());

    let ownerless_delivery = unvalidated_detail(
        DiagnosticKind::Delivery,
        DiagnosticOperation::StateLoad,
        None,
        None,
        None,
        None,
    );
    assert!(ownerless_delivery.validate().is_err());

    let foreign_delivery_evidence = unvalidated_detail(
        DiagnosticKind::Contract,
        DiagnosticOperation::TargetLoad,
        None,
        None,
        None,
        Some(DeliveryProcessDetail::Observability {
            stderr_capture_problem: StderrCaptureProblem::ReaderUnavailable,
        }),
    );
    assert!(foreign_delivery_evidence.validate().is_err());

    let foreign_integration_code = unvalidated_detail(
        DiagnosticKind::Contract,
        DiagnosticOperation::TargetLoad,
        None,
        None,
        Some(IntegrationFaultCode::FfhnPolicyInvariantViolation),
        None,
    );
    assert!(foreign_integration_code.validate().is_err());

    let policy_without_owner = unvalidated_detail(
        DiagnosticKind::PolicyInvariant,
        DiagnosticOperation::PolicyEvaluation,
        None,
        None,
        None,
        None,
    );
    assert!(policy_without_owner.validate().is_err());

    let policy_with_wrong_operation = unvalidated_detail(
        DiagnosticKind::PolicyInvariant,
        DiagnosticOperation::TargetLoad,
        None,
        None,
        None,
        None,
    );
    assert!(policy_with_wrong_operation.validate().is_err());
}

#[test]
fn fetch_failure_evidence_is_closed_to_its_owner_operation_and_truthful_measurement() {
    for (operation, evidence) in [
        (
            DiagnosticOperation::HttpFetch,
            FetchFailureDetails::HttpStatus { status: 503 },
        ),
        (
            DiagnosticOperation::HttpFetch,
            FetchFailureDetails::HttpContentLengthExceeded {
                configured_max_bytes: 10,
                content_length: 11,
            },
        ),
        (
            DiagnosticOperation::HttpFetch,
            FetchFailureDetails::BodyBytesExceeded {
                configured_max_bytes: 10,
                observed_bytes: 11,
            },
        ),
        (
            DiagnosticOperation::FileRead,
            FetchFailureDetails::BodyBytesExceeded {
                configured_max_bytes: 10,
                observed_bytes: 11,
            },
        ),
        (
            DiagnosticOperation::HttpFetch,
            FetchFailureDetails::InvalidUtf8,
        ),
        (
            DiagnosticOperation::FileRead,
            FetchFailureDetails::InvalidUtf8,
        ),
    ] {
        fetch_detail(operation, "source acquisition failed", None, evidence)
            .validate()
            .expect("each released fetch evidence shape is owned by its operation");
    }

    for (kind, operation, evidence) in [
        (
            DiagnosticKind::Contract,
            DiagnosticOperation::HttpFetch,
            FetchFailureDetails::HttpStatus { status: 503 },
        ),
        (
            DiagnosticKind::Io,
            DiagnosticOperation::FileRead,
            FetchFailureDetails::HttpContentLengthExceeded {
                configured_max_bytes: 10,
                content_length: 11,
            },
        ),
        (
            DiagnosticKind::Io,
            DiagnosticOperation::HttpFetch,
            FetchFailureDetails::HttpContentLengthExceeded {
                configured_max_bytes: 10,
                content_length: 10,
            },
        ),
        (
            DiagnosticKind::Io,
            DiagnosticOperation::HttpFetch,
            FetchFailureDetails::HttpStatus { status: 200 },
        ),
        (
            DiagnosticKind::Io,
            DiagnosticOperation::FileRead,
            FetchFailureDetails::BodyBytesExceeded {
                configured_max_bytes: 10,
                observed_bytes: 10,
            },
        ),
    ] {
        assert!(
            unvalidated_fetch_detail_for_contract_test(kind, operation, evidence)
                .validate()
                .is_err()
        );
    }
}

#[test]
fn durable_fitting_and_accessors_distinguish_failure_and_observability_facts() {
    let failure = failed_delivery_detail();
    assert!(failure.is_delivery_failure());
    assert!(!failure.is_delivery_observability());
    failure
        .validate_durable_delivery_failure()
        .expect("delivery failure is durable");
    assert!(failure.clone().fit_durable_delivery_failure().is_ok());

    let observability = delivery_observability_detail(StderrCaptureProblem::ReaderPanicked);
    assert!(!observability.is_delivery_failure());
    assert!(observability.is_delivery_observability());
    assert!(observability.validate_durable_delivery_failure().is_err());
    assert!(observability.fit_durable_delivery_failure().is_err());

    let htmlcut = htmlcut_detail("no match", valid_htmlcut_failure(), None);
    htmlcut
        .validate()
        .expect("a source-health HTMLCut class carries no integration-fault code");
    assert!(htmlcut.htmlcut_failure().is_some());
    assert!(htmlcut.delivery_failure_attempt().is_none());
    assert!(htmlcut.delivery_failure_primary().is_none());
    assert!(htmlcut.stderr_capture_problem().is_none());

    let internal_htmlcut = htmlcut_detail(
        "HTMLCut internal error",
        HtmlcutFailureDetails::new(
            HtmlcutErrorClass::InternalError,
            None,
            "a".repeat(64),
            Vec::new(),
        ),
        Some(IntegrationFaultCode::HtmlcutInternalError),
    );
    internal_htmlcut
        .validate()
        .expect("an upstream HTMLCut internal error owns its integration-fault code");

    let crossed_htmlcut = htmlcut_detail(
        "no match",
        valid_htmlcut_failure(),
        Some(IntegrationFaultCode::HtmlcutInternalError),
    );
    assert!(crossed_htmlcut.validate().is_err());

    let missing_internal_htmlcut_code = htmlcut_detail(
        "HTMLCut internal error",
        HtmlcutFailureDetails::new(
            HtmlcutErrorClass::InternalError,
            None,
            "a".repeat(64),
            Vec::new(),
        ),
        None,
    );
    assert!(missing_internal_htmlcut_code.validate().is_err());
}

#[test]
fn bounded_message_evidence_never_splits_utf8_or_encodes_metadata_as_prose() {
    let (short, short_truncation) = bounded_message_evidence("short".to_owned());
    assert_eq!(short, "short");
    assert_eq!(short_truncation, None);
    let mut already_bounded = "short".to_owned();
    let byte_len = already_bounded.len();
    assert!(!truncate_utf8(&mut already_bounded, byte_len));
    assert_eq!(already_bounded, "short");

    let (ascii, ascii_truncation) =
        bounded_message_evidence("x".repeat(DIAGNOSTIC_MESSAGE_LIMIT + 1));
    assert_eq!(ascii.len(), DIAGNOSTIC_MESSAGE_LIMIT);
    assert!(!ascii.ends_with("[truncated]"));
    let ascii_truncation = ascii_truncation.expect("typed truncation evidence");
    assert_eq!(ascii_truncation.original_len_bytes().as_decimal(), "1025");
    assert_eq!(ascii_truncation.original_sha256().len(), 64);

    let (multibyte, multibyte_truncation) =
        bounded_message_evidence("€".repeat(DIAGNOSTIC_MESSAGE_LIMIT));
    assert!(multibyte.is_char_boundary(multibyte.len()));
    assert!(multibyte.len() <= DIAGNOSTIC_MESSAGE_LIMIT);
    assert_eq!(
        multibyte_truncation
            .expect("typed multibyte truncation evidence")
            .original_len_bytes()
            .as_decimal(),
        "3072"
    );

    let plain = plain_detail(
        DiagnosticKind::Contract,
        DiagnosticOperation::TargetLoad,
        "x".repeat(DIAGNOSTIC_MESSAGE_LIMIT + 1),
        None,
    );
    assert_eq!(plain.message().len(), DIAGNOSTIC_MESSAGE_LIMIT);
    let truncation = plain
        .message_truncation()
        .expect("typed truncation evidence");
    assert_eq!(truncation.original_len_bytes().as_decimal(), "1025");

    let mut invented = serde_json::to_value(&plain).expect("diagnostic JSON");
    invented["message_truncation"]["original_len_bytes"] = serde_json::json!("1024");
    assert!(serde_json::from_value::<DiagnosticDetail>(invented).is_err());

    let mut malformed_digest = serde_json::to_value(&plain).expect("diagnostic JSON");
    malformed_digest["message_truncation"]["original_sha256"] = serde_json::json!("A".repeat(64));
    assert!(serde_json::from_value::<DiagnosticDetail>(malformed_digest).is_err());
}

#[test]
fn durable_fitting_preserves_bounded_binary_evidence_and_refuses_unshrinkable_facts() {
    let attempt = DeliveryProcessAttempt::new(
        TerminalOutcome::Exited { exit_code: Some(1) },
        WriterOutcome::Completed,
        StderrOutcome::captured(
            StderrCapture::from_bytes(vec![0; 2_048], ExactByteCount::from_usize(2_048))
                .expect("bounded capture"),
        ),
    );
    let detail = unvalidated_detail_for_contract_test(
        DiagnosticKind::Delivery,
        DiagnosticOperation::DeliveryProcess,
        "delivery process did not complete successfully".to_owned(),
        None,
        None,
        None,
        None,
        Some(DeliveryProcessDetail::Failure {
            primary: DeliveryFailurePrimary::UnsuccessfulExit,
            attempt,
        }),
    );
    detail.validate().expect("valid before durable fitting");
    detail
        .validate_durable_delivery_failure()
        .expect("bounded base64 evidence fits without discarding retained bytes");
    detail
        .fit_durable_delivery_failure()
        .expect("already-fitting captured stderr remains durable")
        .validate_durable_delivery_failure()
        .expect("fitted detail");

    let shrinkable_attempt = DeliveryProcessAttempt::new(
        TerminalOutcome::Exited { exit_code: Some(1) },
        WriterOutcome::Completed,
        StderrOutcome::captured(
            StderrCapture::from_bytes(vec![0; 2_048], ExactByteCount::from_usize(2_048))
                .expect("bounded capture"),
        ),
    );
    let shrinkable = unvalidated_detail_for_contract_test(
        DiagnosticKind::Delivery,
        DiagnosticOperation::DeliveryProcess,
        "delivery process did not complete successfully".to_owned(),
        Some("x".repeat(1_000)),
        None,
        None,
        None,
        Some(DeliveryProcessDetail::Failure {
            primary: DeliveryFailurePrimary::UnsuccessfulExit,
            attempt: shrinkable_attempt,
        }),
    );
    shrinkable
        .validate()
        .expect("oversized durable evidence remains a valid diagnostic");
    assert!(shrinkable.validate_durable_delivery_failure().is_err());
    shrinkable
        .fit_durable_delivery_failure()
        .expect("durable fitting retains as much bounded stderr evidence as the budget permits")
        .validate_durable_delivery_failure()
        .expect("fitted delivery failure is durable");

    let unshrinkable_attempt = DeliveryProcessAttempt::new(
        TerminalOutcome::Exited { exit_code: Some(1) },
        WriterOutcome::Completed,
        StderrOutcome::captured(
            StderrCapture::from_bytes(Vec::new(), ExactByteCount::zero()).expect("empty capture"),
        ),
    );
    let unshrinkable = unvalidated_detail_for_contract_test(
        DiagnosticKind::Delivery,
        DiagnosticOperation::DeliveryProcess,
        "delivery process did not complete successfully".to_owned(),
        Some("x".repeat(DURABLE_DELIVERY_DETAIL_LIMIT)),
        None,
        None,
        None,
        Some(DeliveryProcessDetail::Failure {
            primary: DeliveryFailurePrimary::UnsuccessfulExit,
            attempt: unshrinkable_attempt,
        }),
    );
    assert!(unshrinkable.validate().is_ok());
    assert!(unshrinkable.validate_durable_delivery_failure().is_err());
    assert!(unshrinkable.fit_durable_delivery_failure().is_err());

    let oversized_observability = unvalidated_detail_for_contract_test(
        DiagnosticKind::Delivery,
        DiagnosticOperation::DeliveryProcess,
        "delivery completed but stderr capture was incomplete".to_owned(),
        Some("x".repeat(DURABLE_DELIVERY_DETAIL_LIMIT)),
        None,
        None,
        None,
        Some(DeliveryProcessDetail::Observability {
            stderr_capture_problem: StderrCaptureProblem::ReaderPanicked,
        }),
    );
    assert!(
        oversized_observability
            .fit_durable_delivery_failure()
            .is_err()
    );
}

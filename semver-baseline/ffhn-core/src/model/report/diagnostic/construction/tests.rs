use crate::{CoreError, DiagnosticKind, DiagnosticOperation, IoErrorClass};

use super::super::super::{
    DeliveryProcessAttempt, ExactByteCount, StderrCapture, StderrOutcome, TerminalOutcome,
    WriterOutcome,
};
use super::{
    delivery_failure_detail, detail_from_core_error, plain_detail,
    strip_diagnostic_classification_prefixes,
};

#[test]
fn core_error_translation_covers_every_ffhn_owned_error_family_without_prose_classification() {
    let cases = [
        (
            CoreError::contract("contract error: invalid target"),
            DiagnosticKind::Contract,
            "invalid target",
        ),
        (
            CoreError::internal("internal error: unexpected state"),
            DiagnosticKind::Contract,
            "unexpected state",
        ),
        (
            CoreError::PolicyInvariant("exactness proof failed".to_owned()),
            DiagnosticKind::PolicyInvariant,
            "exactness proof failed",
        ),
    ];

    for (error, expected_kind, expected_message) in cases {
        let detail = detail_from_core_error(
            &error,
            DiagnosticOperation::TargetValidation,
            Some("target.toml".to_owned()),
        );
        assert_eq!(detail.kind(), expected_kind);
        assert_eq!(detail.message(), expected_message);
    }

    let detail = super::io_detail(
        IoErrorClass::BrokenPipe,
        DiagnosticOperation::FileRead,
        "io: broken pipe",
        None,
    );
    assert_eq!(detail.message(), "broken pipe");
}

#[test]
fn public_message_payload_removes_all_nested_classification_prefixes_and_only_prefixes() {
    assert_eq!(
        strip_diagnostic_classification_prefixes(
            "contract error: io: internal error: evidence".to_owned(),
        ),
        "evidence"
    );
    assert_eq!(
        strip_diagnostic_classification_prefixes(
            "evidence: contract error: still evidence".to_owned()
        ),
        "evidence: contract error: still evidence"
    );
    assert_eq!(
        plain_detail(
            DiagnosticKind::Contract,
            DiagnosticOperation::TargetLoad,
            "url parse error: malformed URL",
            None,
        )
        .message(),
        "malformed URL"
    );
}

#[test]
fn successful_process_attempt_cannot_be_represented_as_a_failure_detail() {
    let success = DeliveryProcessAttempt::new(
        TerminalOutcome::Exited { exit_code: Some(0) },
        WriterOutcome::Completed,
        StderrOutcome::captured(
            StderrCapture::from_bytes(Vec::new(), ExactByteCount::zero()).expect("empty capture"),
        ),
    );
    assert!(delivery_failure_detail(success).is_err());
}

use super::support::*;
use crate::CoreError;

fn failure_detail() -> DiagnosticDetail {
    crate::model::delivery_failure_detail(DeliveryProcessAttempt::new(
        TerminalOutcome::Exited { exit_code: Some(1) },
        WriterOutcome::Completed,
        StderrOutcome::captured(
            StderrCapture::from_bytes(Vec::new(), ExactByteCount::zero()).expect("empty capture"),
        ),
    ))
    .expect("delivery failure detail")
}

fn outcome(
    status: DeliveryStatus,
    error_detail: Option<DiagnosticDetail>,
    outbox_error_detail: Option<DiagnosticDetail>,
    delivery_observability_detail: Option<DiagnosticDetail>,
) -> Result<DeliveryOutcome, CoreError> {
    DeliveryOutcome::new(
        "event".to_owned(),
        "route".to_owned(),
        DeliveryEventKind::Initialized,
        None,
        status,
        1,
        error_detail,
        outbox_error_detail,
        delivery_observability_detail,
    )
}

#[test]
fn delivery_report_rejects_every_crossed_diagnostic_ownership_relation() {
    assert!(
        outcome(
            DeliveryStatus::DeliveredUncommitted,
            None,
            Some(plain_detail(
                DiagnosticKind::Contract,
                DiagnosticOperation::TargetLoad,
                "target loading stopped",
                None,
            )),
            None,
        )
        .is_err()
    );

    assert!(
        outcome(
            DeliveryStatus::RetryScheduled,
            Some(plain_detail(
                DiagnosticKind::Contract,
                DiagnosticOperation::TargetLoad,
                "not a process delivery failure",
                None,
            )),
            None,
            None,
        )
        .is_err()
    );

    assert!(
        outcome(
            DeliveryStatus::Delivered,
            None,
            Some(io_detail(
                IoErrorClass::StorageFull,
                DiagnosticOperation::OutboxStateCommit,
                "state commit failed",
                None,
            )),
            None,
        )
        .is_err()
    );

    for operation in [
        DiagnosticOperation::OutboxDrain,
        DiagnosticOperation::OutboxStateCommit,
    ] {
        assert!(
            outcome(
                DeliveryStatus::RetryUncommitted,
                Some(failure_detail()),
                Some(io_detail(
                    IoErrorClass::StorageFull,
                    operation,
                    "outbox operation failed",
                    None,
                )),
                None,
            )
            .is_ok()
        );
    }

    assert!(
        outcome(
            DeliveryStatus::RetryUncommitted,
            Some(failure_detail()),
            Some(io_detail(
                IoErrorClass::StorageFull,
                DiagnosticOperation::TargetLoad,
                "not an outbox operation",
                None,
            )),
            None,
        )
        .is_err()
    );

    assert!(
        outcome(
            DeliveryStatus::Delivered,
            None,
            Some(crate::model::delivery_observability_detail(
                StderrCaptureProblem::ReaderUnavailable,
            )),
            None,
        )
        .is_err()
    );
    assert!(
        outcome(
            DeliveryStatus::RetryScheduled,
            Some(failure_detail()),
            None,
            Some(crate::model::delivery_observability_detail(
                StderrCaptureProblem::ReaderPanicked,
            )),
        )
        .is_err()
    );

    // `Delivered` is one of the two statuses that may retain a delivery observability fact.
    // Supply a different, independently valid detail there to prove the carrier classification,
    // rather than the status permission, rejects it.
    assert!(
        outcome(
            DeliveryStatus::Delivered,
            None,
            None,
            Some(io_detail(
                IoErrorClass::StorageFull,
                DiagnosticOperation::OutboxDrain,
                "not delivery observability",
                None,
            )),
        )
        .is_err()
    );
}

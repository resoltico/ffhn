use super::support::*;

#[test]
fn delivery_report_accessors_expose_attempt_errors_and_queue_overflow() {
    let delivery_failure = || {
        crate::model::delivery_failure_detail(DeliveryProcessAttempt::new(
            TerminalOutcome::Exited { exit_code: Some(1) },
            WriterOutcome::Completed,
            StderrOutcome::captured(
                StderrCapture::from_bytes(Vec::new(), ExactByteCount::zero())
                    .expect("empty capture"),
            ),
        ))
        .expect("delivery failure detail")
    };
    assert_eq!(
        [
            DeliveryStatus::Delivered,
            DeliveryStatus::RetryScheduled,
            DeliveryStatus::DeadLettered,
            DeliveryStatus::DeliveredUncommitted,
            DeliveryStatus::RetryUncommitted,
            DeliveryStatus::DeadLetterUncommitted,
        ]
        .map(DeliveryStatus::as_str),
        [
            "delivered",
            "retry_scheduled",
            "dead_lettered",
            "delivered_uncommitted",
            "retry_uncommitted",
            "dead_letter_uncommitted",
        ]
    );
    let failed = DeliveryOutcome::dead_lettered(
        "a".repeat(64),
        "route".to_owned(),
        DeliveryEventKind::ConditionSatisfied,
        Some("changed".to_owned()),
        2,
        delivery_failure(),
    );
    assert_eq!(failed.event_id(), "a".repeat(64));
    assert_eq!(failed.route_id(), "route");
    assert_eq!(failed.attempt_count(), 2);
    assert_eq!(
        failed
            .error_detail()
            .and_then(DiagnosticDetail::delivery_failure_primary),
        Some(DeliveryFailurePrimary::UnsuccessfulExit)
    );
    let uncommitted = DeliveryOutcome::delivered_uncommitted(
        "c".repeat(64),
        "route".to_owned(),
        DeliveryEventKind::Initialized,
        None,
        3,
        io_detail(
            IoErrorClass::StorageFull,
            DiagnosticOperation::OutboxStateCommit,
            "state write failed",
            None,
        ),
        None,
    );
    assert_eq!(uncommitted.status(), DeliveryStatus::DeliveredUncommitted);
    assert!(
        uncommitted
            .outbox_error_detail()
            .is_some_and(|detail| detail.message() == "state write failed")
    );
    let overflow = OutboxOverflow::new(
        "b".repeat(64),
        RouteId::new("overflow").expect("route"),
        DeliveryEventKind::ConditionSatisfied,
        Some("changed".parse().expect("condition id")),
    );
    let lifecycle: LifecycleSnapshot = serde_json::from_value(serde_json::json!({
        "source_health": {
            "state": "healthy",
            "reason_class": null,
            "consecutive_unresolved": 0,
            "first_unresolved_at": null,
            "last_details": null,
        },
        "permanent_error_episode": null,
        "integration_fault_episode": null,
    }))
    .expect("healthy lifecycle snapshot");
    let run = RunReport::new(RunReportParts {
        target_id: "demo".to_owned(),
        display_name: Some("Demo".to_owned()),
        run_mode: RunMode::Live,
        outcome: RunOutcome::Initialized,
        started: "2026-07-15T00:00:00Z".to_owned(),
        finished: "2026-07-15T00:00:01Z".to_owned(),
        digest: Some("c".repeat(64)),
        observation: None,
        previous: None,
        error: None,
        policy_evaluation: PolicyEvaluation::not_evaluated(),
        lifecycle_before: None,
        lifecycle_after: Some(lifecycle.clone()),
        state_persisted: false,
        delivery_outcomes: vec![failed.clone()],
        outbox_overflow: vec![overflow.clone()],
        outbox_error_detail: Some(io_detail(
            IoErrorClass::StorageFull,
            DiagnosticOperation::OutboxDrain,
            "outbox drain stopped",
            None,
        )),
    })
    .expect("valid report");
    assert!(run.has_delivery_failure());
    assert!(run.has_delivery_problem());
    assert_eq!(run.delivery_outcomes(), &[failed]);
    assert_eq!(run.outbox_overflow(), std::slice::from_ref(&overflow));
    assert_eq!(
        run.outbox_error_detail().map(DiagnosticDetail::message),
        Some("outbox drain stopped")
    );

    let delivered_uncommitted_only = RunReport::new(RunReportParts {
        target_id: "demo".to_owned(),
        display_name: Some("Demo".to_owned()),
        run_mode: RunMode::Live,
        outcome: RunOutcome::Initialized,
        started: "2026-07-15T00:00:00Z".to_owned(),
        finished: "2026-07-15T00:00:01Z".to_owned(),
        digest: Some("c".repeat(64)),
        observation: None,
        previous: None,
        error: None,
        policy_evaluation: PolicyEvaluation::not_evaluated(),
        lifecycle_before: None,
        lifecycle_after: Some(lifecycle),
        state_persisted: false,
        delivery_outcomes: vec![uncommitted.clone()],
        outbox_overflow: Vec::new(),
        outbox_error_detail: Some(io_detail(
            IoErrorClass::StorageFull,
            DiagnosticOperation::OutboxStateCommit,
            "state write failed",
            None,
        )),
    })
    .expect("valid report");
    assert!(!delivered_uncommitted_only.has_delivery_failure());
    assert!(delivered_uncommitted_only.has_delivery_problem());

    assert_eq!(
        serde_json::to_value(&run).expect("run report JSON")["schema_version"],
        17
    );

    let reset = ResetReport::new(
        "demo",
        true,
        Vec::new(),
        vec![overflow],
        Some(io_detail(
            IoErrorClass::StorageFull,
            DiagnosticOperation::OutboxDrain,
            "outbox drain stopped",
            None,
        )),
    );
    assert!(reset.delivery_outcomes().is_empty());
    assert_eq!(reset.outbox_overflow().len(), 1);
    assert_eq!(
        reset.outbox_error_detail().map(DiagnosticDetail::message),
        Some("outbox drain stopped")
    );
    assert!(reset.has_delivery_problem());
    assert_eq!(
        serde_json::to_value(reset).expect("reset report JSON")["schema_version"],
        7
    );
}

use super::*;

#[test]
fn helper_validators_cover_valid_notification_failures_and_excerpt_digest_edges() {
    let mut change = valid_run_report().change.expect("change");
    validate_run_change_section(&RunChangeSection {
        current_text_bytes: 0,
        current_line_count: 0,
        changed_region: None,
        ..change.clone()
    })
    .expect("empty current text can have zero lines");

    validate_run_change_section(&RunChangeSection {
        changed_region: None,
        previous_text_bytes: Some(0),
        previous_line_count: Some(0),
        ..change.clone()
    })
    .expect("change without region");

    change
        .changed_region
        .as_mut()
        .expect("region")
        .current_excerpt_sha256 = Some("bad".to_owned());
    assert!(validate_run_change_section(&change).is_err());

    let mut optional_excerpts = valid_run_report().change.expect("change");
    let region = optional_excerpts.changed_region.as_mut().expect("region");
    region.current_excerpt = None;
    region.current_excerpt_sha256 = None;
    region.previous_excerpt = None;
    region.previous_excerpt_sha256 = None;
    validate_run_change_section(&optional_excerpts).expect("excerpts can be omitted");

    validate_notification_delivery(&RunNotificationDelivery {
        hook_name: "notify".to_owned(),
        event: NotificationEvent::Changed,
        delivered: false,
        timed_out: false,
        exit_code: Some(7),
        duration_ms: 1,
        error: Some("hook exited with status 7".to_owned()),
    })
    .expect("non-delivered hook can carry a nonzero exit code");

    validate_notification_delivery(&RunNotificationDelivery {
        hook_name: "notify".to_owned(),
        event: NotificationEvent::Changed,
        delivered: true,
        timed_out: false,
        exit_code: Some(0),
        duration_ms: 1,
        error: None,
    })
    .expect("delivered hook with exit code 0 is valid");

    let transient_reason_mismatch = RunReport {
        run_outcome: RunOutcome::FailedTransient,
        reason_code: ReasonCode::ConfigInvalid,
        failure_class: Some(FailureClass::Transient),
        current_compare_digest_sha256: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("transient reason mismatch digest");
    assert!(transient_reason_mismatch.validate().is_err());

    let permanent_reason_mismatch = RunReport {
        run_outcome: RunOutcome::FailedPermanent,
        reason_code: ReasonCode::FetchTimeout,
        failure_class: Some(FailureClass::Permanent),
        current_compare_digest_sha256: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("permanent reason mismatch digest");
    assert!(permanent_reason_mismatch.validate().is_err());

    let dry_run_last_run = RunReport {
        run_mode: RunMode::DryRun,
        persist: RunPersistSection {
            duration_ms: 1,
            wrote_state: false,
            wrote_last_run: true,
        },
        ..valid_run_report()
    }
    .with_digest()
    .expect("dry-run last-run digest");
    assert!(dry_run_last_run.validate().is_err());

    let dry_run_notification = RunReport {
        run_mode: RunMode::DryRun,
        persist: RunPersistSection {
            duration_ms: 1,
            wrote_state: false,
            wrote_last_run: false,
        },
        notifications: vec![RunNotificationDelivery {
            hook_name: "notify".to_owned(),
            event: NotificationEvent::Changed,
            delivered: false,
            timed_out: false,
            exit_code: Some(7),
            duration_ms: 1,
            error: Some("failed".to_owned()),
        }],
        ..valid_run_report()
    }
    .with_digest()
    .expect("dry-run notification digest");
    assert!(dry_run_notification.validate().is_err());
}

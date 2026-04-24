use super::*;

#[test]
fn notification_payload_validation_accepts_predelivery_live_snapshots() {
    let mut run_report = valid_run_report();
    run_report.persist.wrote_last_run = false;
    run_report = run_report
        .with_digest()
        .expect("predelivery run report digest");

    NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        event: NotificationEvent::Changed,
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report,
        extensions: None,
    }
    .validate()
    .expect("valid notification payload");
}

#[test]
fn notification_payload_validation_allows_predelivery_persist_errors_but_rejects_postdelivery_state()
 {
    let mut persist_error_report = valid_run_report();
    persist_error_report.run_outcome = RunOutcome::FailedTransient;
    persist_error_report.reason_code = ReasonCode::PersistError;
    persist_error_report.failure_class = Some(FailureClass::Transient);
    persist_error_report.current_compare_digest_sha256 = None;
    persist_error_report.persist.wrote_state = false;
    persist_error_report.persist.wrote_last_run = false;
    persist_error_report.persist.error = Some(valid_process_error());
    persist_error_report = persist_error_report
        .with_digest()
        .expect("persist error report digest");

    NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        event: NotificationEvent::FailedTransient,
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: persist_error_report,
        extensions: None,
    }
    .validate()
    .expect("persist-error notification payload");

    let mut postdelivery_report = valid_run_report();
    postdelivery_report.notifications = vec![RunNotificationDelivery {
        hook_name: "notify".to_owned(),
        event: NotificationEvent::Changed,
        delivered: true,
        timed_out: false,
        exit_code: Some(0),
        duration_ms: 1,
        error: None,
    }];
    postdelivery_report = postdelivery_report
        .with_digest()
        .expect("postdelivery report digest");

    let invalid = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        event: NotificationEvent::Changed,
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: postdelivery_report,
        extensions: None,
    };
    assert!(invalid.validate().is_err());
}

#[test]
fn notification_payload_validation_rejects_invalid_identity_non_live_and_postdelivery_payloads() {
    let mut predelivery_report = valid_run_report();
    predelivery_report.persist.wrote_last_run = false;
    predelivery_report = predelivery_report
        .with_digest()
        .expect("predelivery digest");

    let invalid_identity = NotificationPayload {
        schema_name: "ffhn.other_payload".to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        event: NotificationEvent::Changed,
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: predelivery_report.clone(),
        extensions: None,
    };
    assert!(invalid_identity.validate().is_err());

    let mut dry_run_report = predelivery_report.clone();
    dry_run_report.run_mode = RunMode::DryRun;
    dry_run_report.persist.wrote_state = false;
    dry_run_report = dry_run_report.with_digest().expect("dry-run digest");
    let non_live = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        event: NotificationEvent::Changed,
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: dry_run_report,
        extensions: None,
    };
    assert!(non_live.validate().is_err());

    let mut notification_report = predelivery_report;
    notification_report.notifications = vec![RunNotificationDelivery {
        hook_name: "notify".to_owned(),
        event: NotificationEvent::Changed,
        delivered: true,
        timed_out: false,
        exit_code: Some(0),
        duration_ms: 1,
        error: None,
    }];
    notification_report = notification_report
        .with_digest()
        .expect("notification digest");
    let postdelivery = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        event: NotificationEvent::Changed,
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: notification_report,
        extensions: None,
    };
    assert!(postdelivery.validate().is_err());
}

#[test]
fn notification_payload_deserialization_revalidates_predelivery_live_contract() {
    let mut run_report = valid_run_report();
    run_report.persist.wrote_last_run = false;
    run_report = run_report
        .with_digest()
        .expect("predelivery run report digest");

    let payload = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        event: NotificationEvent::Changed,
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: run_report.clone(),
        extensions: None,
    };
    let json = serde_json::to_string(&payload).expect("payload json");
    let parsed: NotificationPayload =
        serde_json::from_str(&json).expect("deserialize notification payload");
    assert_eq!(parsed, payload);

    let invalid = NotificationPayload {
        run_report: valid_run_report(),
        ..NotificationPayload {
            schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
            schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
            hook_name: "notify".to_owned(),
            event: NotificationEvent::Changed,
            delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
            run_report,
            extensions: None,
        }
    };
    let invalid_json = serde_json::to_string(&invalid).expect("invalid payload json");
    assert!(serde_json::from_str::<NotificationPayload>(&invalid_json).is_err());
}

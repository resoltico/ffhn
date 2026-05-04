use super::*;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn notification_payload_accessors_expose_the_public_contract() {
    let mut run_report = valid_run_report();
    run_report.persist.last_run_write = PersistWriteStatus::NotAttempted;
    run_report = run_report
        .with_digest()
        .expect("predelivery run report digest");

    let payload = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report,
        extensions: Some(BTreeMap::from([(
            "demo".to_owned(),
            json!({"kind": "ext"}),
        )])),
    };

    assert_eq!(
        payload.schema_name(),
        crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME
    );
    assert_eq!(
        payload.schema_version(),
        crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION
    );
    assert_eq!(payload.hook_name(), "notify");
    assert_eq!(payload.delivery_started_at(), "2026-04-05T10:15:31Z");
    assert_eq!(payload.run_report().run_outcome(), RunOutcome::Changed);
    assert_eq!(
        payload.extensions().expect("extensions").get("demo"),
        Some(&json!({"kind": "ext"}))
    );

    let delivered = delivered_notification();
    assert_eq!(delivered.hook_name(), "notify");
    assert_eq!(delivered.duration_ms(), 1);
    assert_eq!(delivered.status(), NotificationDeliveryStatus::Delivered);
    assert_eq!(delivered.outcome().exit_code(), Some(0));
    assert_eq!(delivered.outcome().error(), None);
    assert_eq!(delivered.exit_code(), Some(0));
    assert_eq!(delivered.error(), None);

    let timed_out = RunNotificationDelivery::timed_out("notify", 2, "hook timed out");
    assert_eq!(timed_out.hook_name(), "notify");
    assert_eq!(timed_out.duration_ms(), 2);
    assert_eq!(timed_out.status(), NotificationDeliveryStatus::TimedOut);
    assert_eq!(timed_out.outcome().exit_code(), None);
    assert_eq!(timed_out.outcome().error(), Some("hook timed out"));
    assert_eq!(timed_out.exit_code(), None);
    assert_eq!(timed_out.error(), Some("hook timed out"));

    let failed = failed_notification(Some(7), "hook exited with status 7");
    assert_eq!(failed.hook_name(), "notify");
    assert_eq!(failed.duration_ms(), 1);
    assert_eq!(failed.status(), NotificationDeliveryStatus::Failed);
    assert_eq!(failed.outcome().exit_code(), Some(7));
    assert_eq!(failed.outcome().error(), Some("hook exited with status 7"));
    assert_eq!(failed.exit_code(), Some(7));
    assert_eq!(failed.error(), Some("hook exited with status 7"));
}

#[test]
fn process_error_detail_serde_round_trips_all_wire_kinds() {
    let cases = [
        (
            ProcessErrorDetail::new(
                ProcessErrorKind::Io,
                "permission denied",
                Some("/tmp/watch/demo/last_run.json".to_owned()),
            )
            .expect("io detail"),
            json!({
                "kind": "io",
                "message": "permission denied",
                "path": "/tmp/watch/demo/last_run.json"
            }),
        ),
        (
            ProcessErrorDetail::new(ProcessErrorKind::Json, "bad json", None).expect("json detail"),
            json!({
                "kind": "json",
                "message": "bad json"
            }),
        ),
        (
            ProcessErrorDetail::new(ProcessErrorKind::Toml, "bad toml", None).expect("toml detail"),
            json!({
                "kind": "toml",
                "message": "bad toml"
            }),
        ),
        (
            ProcessErrorDetail::new(ProcessErrorKind::Url, "bad url", None).expect("url detail"),
            json!({
                "kind": "url",
                "message": "bad url"
            }),
        ),
        (
            ProcessErrorDetail::new(ProcessErrorKind::TimeFormat, "bad format", None)
                .expect("time format detail"),
            json!({
                "kind": "time_format",
                "message": "bad format"
            }),
        ),
        (
            ProcessErrorDetail::new(ProcessErrorKind::TimeParse, "bad timestamp", None)
                .expect("time parse detail"),
            json!({
                "kind": "time_parse",
                "message": "bad timestamp"
            }),
        ),
        (
            ProcessErrorDetail::new(ProcessErrorKind::Contract, "bad contract", None)
                .expect("contract detail"),
            json!({
                "kind": "contract",
                "message": "bad contract"
            }),
        ),
        (
            ProcessErrorDetail::new(ProcessErrorKind::HtmlcutInterop, "bad htmlcut", None)
                .expect("htmlcut detail"),
            json!({
                "kind": "htmlcut_interop",
                "message": "bad htmlcut"
            }),
        ),
        (
            ProcessErrorDetail::new(ProcessErrorKind::Internal, "bad state", None)
                .expect("internal detail"),
            json!({
                "kind": "internal",
                "message": "bad state"
            }),
        ),
    ];

    for (detail, expected_json) in cases {
        let encoded = serde_json::to_value(&detail).expect("serialize process error detail");
        assert_eq!(encoded, expected_json);
        let decoded: ProcessErrorDetail =
            serde_json::from_value(encoded).expect("deserialize process error detail");
        assert_eq!(decoded, detail);
    }
}

#[test]
fn notification_delivery_serde_round_trips_all_wire_status_variants() {
    let delivered = delivered_notification();
    let delivered_json = serde_json::to_value(&delivered).expect("serialize delivered");
    assert_eq!(
        delivered_json,
        json!({
            "hook_name": "notify",
            "duration_ms": 1,
            "outcome": {
                "status": "delivered",
                "exit_code": 0
            }
        })
    );
    let parsed_delivered: RunNotificationDelivery =
        serde_json::from_value(delivered_json).expect("deserialize delivered");
    assert_eq!(parsed_delivered, delivered);

    let timed_out = RunNotificationDelivery::timed_out("notify-timeout", 2, "hook timed out");
    let timed_out_json = serde_json::to_value(&timed_out).expect("serialize timed out");
    assert_eq!(
        timed_out_json,
        json!({
            "hook_name": "notify-timeout",
            "duration_ms": 2,
            "outcome": {
                "status": "timed_out",
                "error": "hook timed out"
            }
        })
    );
    let parsed_timed_out: RunNotificationDelivery =
        serde_json::from_value(timed_out_json).expect("deserialize timed out");
    assert_eq!(parsed_timed_out, timed_out);

    let failed = RunNotificationDelivery::failed("notify-failure", 3, Some(7), "hook failed");
    let failed_json = serde_json::to_value(&failed).expect("serialize failed");
    assert_eq!(
        failed_json,
        json!({
            "hook_name": "notify-failure",
            "duration_ms": 3,
            "outcome": {
                "status": "failed",
                "exit_code": 7,
                "error": "hook failed"
            }
        })
    );
    let parsed_failed: RunNotificationDelivery =
        serde_json::from_value(failed_json).expect("deserialize failed");
    assert_eq!(parsed_failed, failed);
}

#[test]
fn notification_payload_validation_accepts_predelivery_live_snapshots() {
    let mut run_report = valid_run_report();
    run_report.persist.last_run_write = PersistWriteStatus::NotAttempted;
    run_report = run_report
        .with_digest()
        .expect("predelivery run report digest");

    NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
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
    persist_error_report.error_detail = Some(valid_process_error());
    persist_error_report.persist.state_write = PersistWriteStatus::Failed {
        error: valid_process_error(),
    };
    persist_error_report.persist.last_run_write = PersistWriteStatus::NotAttempted;
    persist_error_report = persist_error_report
        .with_digest()
        .expect("persist error report digest");

    NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: persist_error_report,
        extensions: None,
    }
    .validate()
    .expect("persist-error notification payload");

    let mut postdelivery_report = valid_run_report();
    postdelivery_report.notifications = vec![delivered_notification()];
    postdelivery_report = postdelivery_report
        .with_digest()
        .expect("postdelivery report digest");

    let invalid = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: postdelivery_report,
        extensions: None,
    };
    assert!(invalid.validate().is_err());

    let mut invalid_last_run_write = valid_run_report();
    invalid_last_run_write.persist.last_run_write = PersistWriteStatus::Failed {
        error: valid_process_error(),
    };
    invalid_last_run_write = invalid_last_run_write
        .with_digest()
        .expect("invalid last-run-write digest");
    let invalid = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: invalid_last_run_write,
        extensions: None,
    };
    assert!(invalid.validate().is_err());
}

#[test]
fn notification_payload_validation_rejects_invalid_identity_non_live_and_postdelivery_payloads() {
    let mut predelivery_report = valid_run_report();
    predelivery_report.persist.last_run_write = PersistWriteStatus::NotAttempted;
    predelivery_report = predelivery_report
        .with_digest()
        .expect("predelivery digest");

    let invalid_identity = NotificationPayload {
        schema_name: "ffhn.other_payload".to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: predelivery_report.clone(),
        extensions: None,
    };
    assert!(invalid_identity.validate().is_err());

    let mut dry_run_report = predelivery_report.clone();
    dry_run_report.run_mode = RunMode::DryRun;
    dry_run_report.persist.state_write = PersistWriteStatus::NotAttempted;
    dry_run_report = dry_run_report.with_digest().expect("dry-run digest");
    let non_live = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: dry_run_report,
        extensions: None,
    };
    assert!(non_live.validate().is_err());

    let mut notification_report = predelivery_report;
    notification_report.notifications = vec![delivered_notification()];
    notification_report = notification_report
        .with_digest()
        .expect("notification digest");
    let postdelivery = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: notification_report,
        extensions: None,
    };
    assert!(postdelivery.validate().is_err());
}

#[test]
fn notification_payload_deserialization_revalidates_predelivery_live_contract() {
    let mut run_report = valid_run_report();
    run_report.persist.last_run_write = PersistWriteStatus::NotAttempted;
    run_report = run_report
        .with_digest()
        .expect("predelivery run report digest");

    let payload = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
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
            delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
            run_report,
            extensions: None,
        }
    };
    let invalid_json = serde_json::to_string(&invalid).expect("invalid payload json");
    assert!(serde_json::from_str::<NotificationPayload>(&invalid_json).is_err());
}

#[test]
fn notification_payload_validation_rejects_delivery_timestamps_before_run_finish() {
    let mut run_report = valid_run_report();
    run_report.persist.last_run_write = PersistWriteStatus::NotAttempted;
    run_report = run_report
        .with_digest()
        .expect("predelivery run report digest");

    let invalid = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: "notify".to_owned(),
        delivery_started_at: "2026-04-05T10:15:30Z".to_owned(),
        run_report,
        extensions: None,
    };
    assert!(invalid.validate().is_err());
}

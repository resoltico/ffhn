use super::*;
use serde_json::json;
use std::collections::BTreeMap;

fn predelivery_report() -> RunReport {
    RunReport {
        persist: persist_section(
            1,
            PersistWriteStatus::Written,
            PersistWriteStatus::NotAttempted,
        ),
        ..valid_run_report()
    }
    .with_digest()
    .expect("predelivery run report digest")
}

#[test]
fn notification_payload_accessors_expose_the_public_contract() {
    let payload = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        route_name: "notify".to_owned(),
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: predelivery_report(),
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
    assert_eq!(payload.route_name(), "notify");
    assert_eq!(payload.delivery_started_at(), "2026-04-05T10:15:31Z");
    assert_eq!(payload.run_report().run_outcome(), RunOutcome::Changed);
    assert_eq!(
        payload.extensions().expect("extensions").get("demo"),
        Some(&json!({"kind": "ext"}))
    );

    let delivered = delivered_notification();
    assert_eq!(delivered.route_name(), "notify");
    assert_eq!(delivered.duration_ms(), 1);
    assert_eq!(delivered.status(), NotificationDeliveryStatus::Delivered);
    assert_eq!(delivered.status().as_str(), "delivered");
    assert_eq!(delivered.outcome().exit_code(), Some(0));
    assert_eq!(delivered.outcome().error(), None);
    assert_eq!(delivered.exit_code(), Some(0));
    assert_eq!(delivered.error(), None);

    let timed_out = RunNotificationDelivery::timed_out("notify", 2, "route timed out");
    assert_eq!(timed_out.route_name(), "notify");
    assert_eq!(timed_out.duration_ms(), 2);
    assert_eq!(timed_out.status(), NotificationDeliveryStatus::TimedOut);
    assert_eq!(timed_out.status().as_str(), "timed_out");
    assert_eq!(timed_out.outcome().exit_code(), None);
    assert_eq!(timed_out.outcome().error(), Some("route timed out"));
    assert_eq!(timed_out.exit_code(), None);
    assert_eq!(timed_out.error(), Some("route timed out"));

    let failed = failed_notification(Some(7), "route exited with status 7");
    assert_eq!(failed.route_name(), "notify");
    assert_eq!(failed.duration_ms(), 1);
    assert_eq!(failed.status(), NotificationDeliveryStatus::Failed);
    assert_eq!(failed.status().as_str(), "failed");
    assert_eq!(failed.outcome().exit_code(), Some(7));
    assert_eq!(failed.outcome().error(), Some("route exited with status 7"));
    assert_eq!(failed.exit_code(), Some(7));
    assert_eq!(failed.error(), Some("route exited with status 7"));
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
        (
            ProcessErrorDetail::new(
                ProcessErrorKind::PersistTransaction,
                "primary persist failure: filesystem error at /tmp/watch/demo/state.json: write failed",
                None,
            )
            .expect("persist transaction detail"),
            json!({
                "kind": "persist_transaction",
                "message": "primary persist failure: filesystem error at /tmp/watch/demo/state.json: write failed"
            }),
        ),
    ];

    for (detail, expected_json) in cases {
        assert_eq!(
            detail.kind().as_str(),
            expected_json["kind"].as_str().expect("kind string")
        );
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
            "route_name": "notify",
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

    let timed_out = RunNotificationDelivery::timed_out("notify-timeout", 2, "route timed out");
    let timed_out_json = serde_json::to_value(&timed_out).expect("serialize timed out");
    assert_eq!(
        timed_out_json,
        json!({
            "route_name": "notify-timeout",
            "duration_ms": 2,
            "outcome": {
                "status": "timed_out",
                "error": "route timed out"
            }
        })
    );
    let parsed_timed_out: RunNotificationDelivery =
        serde_json::from_value(timed_out_json).expect("deserialize timed out");
    assert_eq!(parsed_timed_out, timed_out);

    let failed = RunNotificationDelivery::failed("notify-failure", 3, Some(7), "route failed");
    let failed_json = serde_json::to_value(&failed).expect("serialize failed");
    assert_eq!(
        failed_json,
        json!({
            "route_name": "notify-failure",
            "duration_ms": 3,
            "outcome": {
                "status": "failed",
                "exit_code": 7,
                "error": "route failed"
            }
        })
    );
    let parsed_failed: RunNotificationDelivery =
        serde_json::from_value(failed_json).expect("deserialize failed");
    assert_eq!(parsed_failed, failed);
}

#[test]
fn notification_payload_validation_accepts_predelivery_live_snapshots() {
    NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        route_name: "notify".to_owned(),
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: predelivery_report(),
        extensions: None,
    }
    .validate()
    .expect("predelivery payload");
}

#[test]
fn notification_payload_validation_rejects_invalid_route_name_and_run_snapshot_shapes() {
    let invalid_route_name = NotificationPayload {
        route_name: String::new(),
        ..NotificationPayload {
            schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
            schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
            route_name: "notify".to_owned(),
            delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
            run_report: predelivery_report(),
            extensions: None,
        }
    };
    assert!(invalid_route_name.validate().is_err());

    let dry_run_payload = NotificationPayload {
        run_report: RunReport {
            run_mode: RunMode::DryRun,
            persist: persist_section(
                1,
                PersistWriteStatus::NotAttempted,
                PersistWriteStatus::NotAttempted,
            ),
            ..valid_run_report()
        }
        .with_digest()
        .expect("dry run digest"),
        ..NotificationPayload {
            schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
            schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
            route_name: "notify".to_owned(),
            delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
            run_report: predelivery_report(),
            extensions: None,
        }
    };
    assert!(dry_run_payload.validate().is_err());

    let invalid_last_run_write = NotificationPayload {
        run_report: valid_run_report(),
        ..NotificationPayload {
            schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
            schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
            route_name: "notify".to_owned(),
            delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
            run_report: predelivery_report(),
            extensions: None,
        }
    };
    assert!(invalid_last_run_write.validate().is_err());

    let notification_loopback = NotificationPayload {
        run_report: RunReport {
            notifications: vec![failed_notification(Some(7), "route failed")],
            ..predelivery_report()
        }
        .with_digest()
        .expect("loopback digest"),
        ..NotificationPayload {
            schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
            schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
            route_name: "notify".to_owned(),
            delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
            run_report: predelivery_report(),
            extensions: None,
        }
    };
    assert!(notification_loopback.validate().is_err());
}

#[test]
fn notification_payload_validation_rejects_delivery_timing_and_legacy_wire_shapes() {
    let too_early = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        route_name: "notify".to_owned(),
        delivery_started_at: "2026-04-05T10:15:30Z".to_owned(),
        run_report: predelivery_report(),
        extensions: None,
    };
    assert!(too_early.validate().is_err());

    let payload = NotificationPayload {
        schema_name: crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        route_name: "notify".to_owned(),
        delivery_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_report: predelivery_report(),
        extensions: None,
    };
    let json = serde_json::to_string(&payload).expect("payload json");
    let parsed: NotificationPayload = serde_json::from_str(&json).expect("payload round trip");
    assert_eq!(parsed, payload);

    let legacy_json = serde_json::json!({
        "schema_name": crate::NOTIFICATION_PAYLOAD_SCHEMA_NAME,
        "schema_version": crate::NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        "hook_name": "notify",
        "delivery_started_at": "2026-04-05T10:15:31Z",
        "run_report": serde_json::to_value(predelivery_report()).expect("report json")
    });
    assert!(serde_json::from_value::<NotificationPayload>(legacy_json).is_err());
}

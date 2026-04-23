use super::*;
use time::{Date, Month, OffsetDateTime, format_description::well_known::Rfc3339};
use url::Url;

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
            error: None,
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
            error: None,
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

    let dry_run_persist_error = RunReport {
        run_mode: RunMode::DryRun,
        persist: RunPersistSection {
            duration_ms: 1,
            wrote_state: false,
            wrote_last_run: false,
            error: Some(valid_process_error()),
        },
        ..valid_run_report()
    }
    .with_digest()
    .expect("dry-run persist error digest");
    assert!(dry_run_persist_error.validate().is_err());

    let invalid_persist_error = RunReport {
        persist: RunPersistSection {
            duration_ms: 1,
            wrote_state: true,
            wrote_last_run: true,
            error: Some(valid_process_error()),
        },
        ..valid_run_report()
    }
    .with_digest()
    .expect("invalid persist error digest");
    assert!(invalid_persist_error.validate().is_err());
}

#[test]
fn process_error_detail_conversions_cover_each_error_kind_and_validation_edges() {
    let json_error = serde_json::from_str::<serde_json::Value>("{").expect_err("json error");
    let json_detail = ProcessErrorDetail::from(&CoreError::from(json_error));
    assert_eq!(json_detail.kind, ProcessErrorKind::Json);
    assert!(json_detail.path.is_none());

    let toml_error = toml::from_str::<toml::Value>("=").expect_err("toml error");
    let toml_detail = ProcessErrorDetail::from(&CoreError::from(toml_error));
    assert_eq!(toml_detail.kind, ProcessErrorKind::Toml);
    assert!(toml_detail.path.is_none());

    let url_error = Url::parse("not a url").expect_err("url error");
    let url_detail = ProcessErrorDetail::from(&CoreError::from(url_error));
    assert_eq!(url_detail.kind, ProcessErrorKind::Url);
    assert!(url_detail.path.is_none());

    let offset_only = time::format_description::parse("[offset_hour]").expect("format description");
    let format_error = Date::from_calendar_date(2026, Month::April, 5)
        .expect("date")
        .midnight()
        .format(&offset_only)
        .expect_err("format error");
    let format_detail = ProcessErrorDetail::from(&CoreError::from(format_error));
    assert_eq!(format_detail.kind, ProcessErrorKind::TimeFormat);
    assert!(format_detail.path.is_none());

    let parse_error = OffsetDateTime::parse("not-a-timestamp", &Rfc3339).expect_err("parse error");
    let parse_detail = ProcessErrorDetail::from(&CoreError::from(parse_error));
    assert_eq!(parse_detail.kind, ProcessErrorKind::TimeParse);
    assert!(parse_detail.path.is_none());

    let htmlcut_detail = ProcessErrorDetail::from(&CoreError::htmlcut("bad plan"));
    assert_eq!(htmlcut_detail.kind, ProcessErrorKind::Htmlcut);
    assert_eq!(htmlcut_detail.message, "bad plan");
    assert!(htmlcut_detail.path.is_none());

    assert!(
        ProcessErrorDetail {
            kind: ProcessErrorKind::Io,
            message: String::new(),
            path: None,
        }
        .validate()
        .is_err()
    );
    assert!(
        ProcessErrorDetail {
            kind: ProcessErrorKind::Io,
            message: "denied".to_owned(),
            path: Some(String::new()),
        }
        .validate()
        .is_err()
    );
}

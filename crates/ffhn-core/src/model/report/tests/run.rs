use super::*;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn run_report_accessors_expose_the_public_contract() {
    let mut report = valid_run_report();
    report.notifications = vec![
        RunNotificationDelivery::delivered("notify-success", 4, 0),
        RunNotificationDelivery::timed_out("notify-timeout", 9, "timed out"),
        RunNotificationDelivery::failed("notify-failure", 7, Some(3), "hook failed"),
    ];
    report.extensions = Some(BTreeMap::from([(
        "demo".to_owned(),
        json!({"kind": "ext"}),
    )]));

    assert_eq!(report.schema_name(), RUN_REPORT_SCHEMA_NAME);
    assert_eq!(report.schema_version(), RUN_REPORT_SCHEMA_VERSION);
    assert_eq!(report.run_report_digest_sha256().len(), 64);
    assert_eq!(report.run_mode(), RunMode::Live);
    assert_eq!(report.run_outcome(), RunOutcome::Changed);
    assert_eq!(report.reason_code(), ReasonCode::Ok);
    assert_eq!(report.target_id(), "demo");
    assert_eq!(report.run_started_at(), "2026-04-05T10:15:30Z");
    assert_eq!(report.run_finished_at(), "2026-04-05T10:15:31Z");
    assert_eq!(report.failure_class(), None);
    assert!(report.error_detail().is_none());
    assert_eq!(report.target_status_after_run(), TargetStatus::Ready);
    assert_eq!(report.compare_basis(), CompareBasis::CanonicalTextSha256);
    assert_eq!(report.previous_compare_digest_sha256(), Some(DIGEST));
    assert_eq!(report.current_compare_digest_sha256(), Some(DIGEST));
    assert_eq!(report.state_phase_before_run(), StatePhase::HasBaseline);
    assert_eq!(report.state_phase_after_run(), StatePhase::HasBaseline);
    let fetch = report.fetch().expect("fetch");
    assert_eq!(fetch.engine(), FetchEngine::Http);
    assert_eq!(fetch.final_url(), Some("https://example.com/final"));
    assert_eq!(fetch.http_status(), Some(200));
    assert_eq!(fetch.content_type(), Some("text/html"));
    assert_eq!(fetch.bytes_read(), Some(42));
    assert_eq!(fetch.duration_ms(), 12);

    let extraction = report.extraction().expect("extraction");
    assert_eq!(extraction.interop_profile(), HTMLCUT_INTEROP_PROFILE);
    assert_eq!(extraction.htmlcut_plan_digest_sha256(), DIGEST);
    assert_eq!(extraction.htmlcut_result_digest_sha256(), DIGEST);
    assert_eq!(extraction.comparison_input_sha256(), DIGEST);
    assert_eq!(extraction.outer_html_sha256(), DIGEST);
    assert_eq!(extraction.strategy_kind(), SelectionKind::CssSelector);
    assert_eq!(extraction.selection_mode(), SelectionMatch::Single);
    assert_eq!(extraction.output_kind(), OutputKind::OuterHtml);
    assert_eq!(extraction.candidate_count(), 1);
    assert_eq!(extraction.selected_candidate_index(), 1);
    assert!(extraction.warning_codes().is_empty());
    assert_eq!(extraction.duration_ms(), 8);

    let compare = report.compare().expect("compare");
    assert_eq!(compare.canonicalizers(), ["trim"]);
    assert_eq!(compare.duration_ms(), 3);

    let change = report.change().expect("change");
    assert_eq!(change.kind(), ChangeKind::Changed);
    assert_eq!(change.previous_text_bytes(), Some(6));
    assert_eq!(change.current_text_bytes(), 7);
    assert_eq!(change.previous_line_count(), Some(1));
    assert_eq!(change.current_line_count(), 1);
    assert_eq!(change.common_prefix_lines(), 0);
    assert_eq!(change.common_suffix_lines(), 0);
    let changed_region = change.changed_region().expect("changed region");
    assert_eq!(changed_region.previous_start_line(), 1);
    assert_eq!(changed_region.previous_line_count(), 1);
    assert_eq!(changed_region.current_start_line(), 1);
    assert_eq!(changed_region.current_line_count(), 1);
    assert_eq!(changed_region.previous_excerpt(), Some("Before"));
    assert_eq!(changed_region.current_excerpt(), Some("Changed"));
    assert_eq!(changed_region.previous_excerpt_sha256(), Some(DIGEST));
    assert_eq!(changed_region.current_excerpt_sha256(), Some(DIGEST));

    let notifications = report.notifications().collect::<Vec<_>>();
    assert_eq!(notifications.len(), 3);
    assert_eq!(notifications[0].hook_name(), "notify-success");
    assert_eq!(notifications[0].duration_ms(), 4);
    assert_eq!(
        notifications[0].status(),
        NotificationDeliveryStatus::Delivered
    );
    assert_eq!(notifications[0].exit_code(), Some(0));
    assert!(notifications[0].error().is_none());
    assert_eq!(notifications[1].hook_name(), "notify-timeout");
    assert_eq!(notifications[1].duration_ms(), 9);
    assert_eq!(
        notifications[1].status(),
        NotificationDeliveryStatus::TimedOut
    );
    assert_eq!(notifications[1].exit_code(), None);
    assert_eq!(notifications[1].error(), Some("timed out"));
    assert_eq!(notifications[2].hook_name(), "notify-failure");
    assert_eq!(notifications[2].duration_ms(), 7);
    assert_eq!(
        notifications[2].status(),
        NotificationDeliveryStatus::Failed
    );
    assert_eq!(notifications[2].exit_code(), Some(3));
    assert_eq!(notifications[2].error(), Some("hook failed"));
    assert_eq!(report.persist().duration_ms(), 2);
    assert!(matches!(
        report.persist().state_write(),
        PersistWriteStatus::Written
    ));
    assert!(matches!(
        report.persist().last_run_write(),
        PersistWriteStatus::Written
    ));
    assert!(!report.persist().has_failure());
    assert!(report.persist().error().is_none());
    assert_eq!(
        report.extensions().expect("extensions").get("demo"),
        Some(&json!({"kind": "ext"}))
    );
}

#[test]
fn run_report_validation_accepts_a_digest_checked_success_report() {
    let report = valid_run_report();
    assert_eq!(report.run_mode(), RunMode::Live);
    assert_eq!(report.run_outcome(), RunOutcome::Changed);
    assert_eq!(report.reason_code(), ReasonCode::Ok);
    assert_eq!(report.target_id(), "demo");
    report.validate().expect("run report");
}

#[test]
fn run_report_validation_rejects_invalid_reason_and_digest_combinations() {
    let report = RunReport {
        reason_code: ReasonCode::Disabled,
        ..valid_run_report()
    }
    .with_digest()
    .expect("disabled digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        run_outcome: RunOutcome::SkippedDisabled,
        reason_code: ReasonCode::Ok,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        run_outcome: RunOutcome::FailedPermanent,
        reason_code: ReasonCode::FetchHttpClientError,
        failure_class: Some(FailureClass::Permanent),
        error_detail: Some(valid_process_error()),
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("failed digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        current_compare_digest_sha256: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("missing current digest");
    assert!(report.validate().is_err());
}

#[test]
fn run_report_validation_requires_error_detail_for_failed_outcomes() {
    let transient = RunReport {
        run_outcome: RunOutcome::FailedTransient,
        reason_code: ReasonCode::FetchTimeout,
        failure_class: Some(FailureClass::Transient),
        current_compare_digest_sha256: None,
        change: None,
        error_detail: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("transient missing-detail digest");
    assert!(transient.validate().is_err());

    let permanent = RunReport {
        run_outcome: RunOutcome::FailedPermanent,
        reason_code: ReasonCode::ConfigInvalid,
        failure_class: Some(FailureClass::Permanent),
        current_compare_digest_sha256: None,
        change: None,
        error_detail: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("permanent missing-detail digest");
    assert!(permanent.validate().is_err());
}

#[test]
fn run_report_validation_accepts_failed_reports_without_compare_digests() {
    let report = RunReport {
        run_outcome: RunOutcome::FailedPermanent,
        reason_code: ReasonCode::FetchHttpClientError,
        failure_class: Some(FailureClass::Permanent),
        error_detail: Some(valid_process_error()),
        current_compare_digest_sha256: None,
        fetch: None,
        extraction: None,
        compare: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("failed digest");

    report.validate().expect("failed report");
}

#[test]
fn run_report_validation_accepts_skipped_disabled_reports_with_optional_fields() {
    let report = RunReport {
        run_outcome: RunOutcome::SkippedDisabled,
        reason_code: ReasonCode::Disabled,
        previous_compare_digest_sha256: None,
        current_compare_digest_sha256: None,
        fetch: None,
        extraction: None,
        compare: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped-disabled digest");

    report.validate().expect("skipped-disabled report");
}

#[test]
fn run_report_validation_accepts_fetch_sections_without_final_url() {
    let mut report = valid_run_report();
    report.fetch.as_mut().expect("fetch").final_url = None;

    report
        .with_digest()
        .expect("missing final-url digest")
        .validate()
        .expect("run report without redirect url");
}

#[test]
fn run_report_validation_rejects_stale_report_digests() {
    let mut report = valid_run_report();
    report.reason_code = ReasonCode::Disabled;

    assert!(report.validate().is_err());
}

#[test]
fn run_report_validation_checks_nested_fetch_and_extraction_fields() {
    let mut report = valid_run_report();
    report.fetch.as_mut().expect("fetch").final_url = Some("not a url".to_owned());
    let report = report.with_digest().expect("fetch digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        fetch: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("missing fetch digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        extraction: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("missing extraction digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        compare: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("missing compare digest");
    assert!(report.validate().is_err());

    let mut report = valid_run_report();
    report
        .extraction
        .as_mut()
        .expect("extraction")
        .interop_profile = "wrong".to_owned();
    let report = report.with_digest().expect("interop digest");
    assert!(report.validate().is_err());

    let mut report = valid_run_report();
    report
        .extraction
        .as_mut()
        .expect("extraction")
        .outer_html_sha256 = "bad".to_owned();
    let report = report.with_digest().expect("extraction digest");
    assert!(report.validate().is_err());

    let mut report = valid_run_report();
    report
        .extraction
        .as_mut()
        .expect("extraction")
        .selected_candidate_index = 0;
    let report = report.with_digest().expect("zero-selected digest");
    assert!(report.validate().is_err());

    let mut report = valid_run_report();
    report
        .extraction
        .as_mut()
        .expect("extraction")
        .selected_candidate_index = 2;
    let report = report.with_digest().expect("candidate digest");
    assert!(report.validate().is_err());

    let mut report = valid_run_report();
    report
        .extraction
        .as_mut()
        .expect("extraction")
        .candidate_count = 0;
    let report = report.with_digest().expect("zero-candidate digest");
    assert!(report.validate().is_err());

    let mut report = valid_run_report();
    report.previous_compare_digest_sha256 = Some("bad".to_owned());
    let report = report.with_digest().expect("previous digest");
    assert!(report.validate().is_err());
}

#[test]
fn run_report_validation_rejects_skipped_disabled_payload_sections() {
    let report = RunReport {
        run_outcome: RunOutcome::SkippedDisabled,
        reason_code: ReasonCode::Disabled,
        previous_compare_digest_sha256: None,
        current_compare_digest_sha256: None,
        fetch: Some(RunFetchSection {
            engine: FetchEngine::Http,
            final_url: None,
            http_status: None,
            content_type: None,
            bytes_read: None,
            duration_ms: 1,
        }),
        extraction: None,
        compare: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped fetch digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        run_outcome: RunOutcome::SkippedDisabled,
        reason_code: ReasonCode::Disabled,
        previous_compare_digest_sha256: None,
        current_compare_digest_sha256: None,
        fetch: None,
        extraction: valid_run_report().extraction,
        compare: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped extraction digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        run_outcome: RunOutcome::SkippedDisabled,
        reason_code: ReasonCode::Disabled,
        previous_compare_digest_sha256: None,
        current_compare_digest_sha256: None,
        fetch: None,
        extraction: None,
        compare: valid_run_report().compare,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped compare digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        run_outcome: RunOutcome::SkippedDisabled,
        reason_code: ReasonCode::Disabled,
        previous_compare_digest_sha256: None,
        current_compare_digest_sha256: Some(DIGEST.to_owned()),
        fetch: None,
        extraction: None,
        compare: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped current digest");
    assert!(report.validate().is_err());
}

#[test]
fn run_report_validation_rejects_dry_run_mutation_and_failure_class_mismatches() {
    let report = RunReport {
        run_mode: RunMode::DryRun,
        persist: persist_section(
            1,
            PersistWriteStatus::Written,
            PersistWriteStatus::NotAttempted,
        ),
        ..valid_run_report()
    }
    .with_digest()
    .expect("dry-run mutate digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        failure_class: Some(FailureClass::Transient),
        ..valid_run_report()
    }
    .with_digest()
    .expect("success failure-class digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        error_detail: Some(valid_process_error()),
        ..valid_run_report()
    }
    .with_digest()
    .expect("success error-detail digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        run_outcome: RunOutcome::FailedTransient,
        reason_code: ReasonCode::FetchTimeout,
        failure_class: Some(FailureClass::Permanent),
        error_detail: Some(valid_process_error()),
        current_compare_digest_sha256: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("transient mismatch digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        run_outcome: RunOutcome::FailedPermanent,
        reason_code: ReasonCode::ConfigInvalid,
        failure_class: Some(FailureClass::Transient),
        error_detail: Some(valid_process_error()),
        current_compare_digest_sha256: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("permanent mismatch digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        run_outcome: RunOutcome::SkippedDisabled,
        reason_code: ReasonCode::Disabled,
        failure_class: Some(FailureClass::Permanent),
        previous_compare_digest_sha256: None,
        current_compare_digest_sha256: None,
        fetch: None,
        extraction: None,
        compare: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped failure-class digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        run_outcome: RunOutcome::SkippedDisabled,
        reason_code: ReasonCode::Disabled,
        previous_compare_digest_sha256: None,
        current_compare_digest_sha256: None,
        fetch: None,
        extraction: None,
        compare: None,
        change: None,
        error_detail: Some(valid_process_error()),
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped error-detail digest");
    assert!(report.validate().is_err());
}

#[test]
fn run_report_validation_checks_change_and_notification_details() {
    let report = RunReport {
        change: Some(RunChangeSection {
            current_line_count: 0,
            ..valid_run_report().change.expect("change")
        }),
        ..valid_run_report()
    }
    .with_digest()
    .expect("zero current line digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        change: Some(RunChangeSection {
            previous_line_count: Some(0),
            ..valid_run_report().change.expect("change")
        }),
        ..valid_run_report()
    }
    .with_digest()
    .expect("zero previous line digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        change: Some(RunChangeSection {
            changed_region: Some(RunChangeRegion {
                current_excerpt_sha256: None,
                ..valid_run_report()
                    .change
                    .expect("change")
                    .changed_region
                    .expect("region")
            }),
            ..valid_run_report().change.expect("change")
        }),
        ..valid_run_report()
    }
    .with_digest()
    .expect("missing current excerpt digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        change: Some(RunChangeSection {
            changed_region: Some(RunChangeRegion {
                previous_excerpt_sha256: None,
                ..valid_run_report()
                    .change
                    .expect("change")
                    .changed_region
                    .expect("region")
            }),
            ..valid_run_report().change.expect("change")
        }),
        ..valid_run_report()
    }
    .with_digest()
    .expect("missing previous excerpt digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        notifications: vec![RunNotificationDelivery {
            hook_name: "demo".to_owned(),
            duration_ms: 1,
            outcome: NotificationDeliveryOutcome::TimedOut {
                error: String::new(),
            },
        }],
        ..valid_run_report()
    }
    .with_digest()
    .expect("timed-out delivered digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        notifications: vec![RunNotificationDelivery {
            hook_name: "demo".to_owned(),
            duration_ms: 1,
            outcome: NotificationDeliveryOutcome::Delivered { exit_code: 7 },
        }],
        ..valid_run_report()
    }
    .with_digest()
    .expect("nonzero delivered digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        persist: persist_section(
            1,
            PersistWriteStatus::NotAttempted,
            PersistWriteStatus::Failed {
                error: valid_process_error(),
            },
        ),
        run_outcome: RunOutcome::FailedTransient,
        reason_code: ReasonCode::PersistError,
        failure_class: Some(FailureClass::Transient),
        error_detail: Some(valid_process_error()),
        ..valid_run_report()
    }
    .with_digest()
    .expect("persist error detail digest");
    report.validate().expect("persist error detail report");

    let report = RunReport {
        persist: persist_section(
            1,
            PersistWriteStatus::Failed {
                error: valid_process_error(),
            },
            PersistWriteStatus::Written,
        ),
        run_outcome: RunOutcome::FailedTransient,
        reason_code: ReasonCode::PersistError,
        failure_class: Some(FailureClass::Transient),
        error_detail: Some(valid_process_error()),
        current_compare_digest_sha256: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("persist error digest");
    report
        .validate()
        .expect("persist error with last_run write");

    let report = RunReport {
        persist: persist_section(1, PersistWriteStatus::Written, PersistWriteStatus::Written),
        run_outcome: RunOutcome::FailedTransient,
        reason_code: ReasonCode::PersistError,
        failure_class: Some(FailureClass::Transient),
        error_detail: Some(valid_process_error()),
        current_compare_digest_sha256: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("persist error without failed write digest");
    assert!(report.validate().is_err());
}

#[test]
fn run_report_validation_rejects_reversed_run_timestamps() {
    let report = RunReport {
        run_started_at: "2026-04-05T10:15:31Z".to_owned(),
        run_finished_at: "2026-04-05T10:15:30Z".to_owned(),
        ..valid_run_report()
    }
    .with_digest()
    .expect("reversed timestamp digest");
    assert!(report.validate().is_err());
}

#[test]
fn persist_section_error_reports_last_run_failures_when_state_write_succeeds() {
    let persist = persist_section(
        1,
        PersistWriteStatus::Written,
        PersistWriteStatus::Failed {
            error: valid_process_error(),
        },
    );

    let error = persist.error().expect("persist error");
    assert_eq!(error.kind(), ProcessErrorKind::Io);
    assert_eq!(error.message(), "permission denied");
    assert_eq!(error.path(), Some("/tmp/watch/demo/last_run.json"));
}

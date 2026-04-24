use super::*;

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
fn run_report_validation_accepts_failed_reports_without_compare_digests() {
    let report = RunReport {
        run_outcome: RunOutcome::FailedPermanent,
        reason_code: ReasonCode::FetchHttpClientError,
        failure_class: Some(FailureClass::Permanent),
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
        persist: RunPersistSection {
            duration_ms: 1,
            wrote_state: true,
            wrote_last_run: false,
            error: None,
        },
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
        run_outcome: RunOutcome::FailedTransient,
        reason_code: ReasonCode::FetchTimeout,
        failure_class: Some(FailureClass::Permanent),
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
            event: NotificationEvent::Changed,
            delivered: true,
            timed_out: true,
            exit_code: Some(0),
            duration_ms: 1,
            error: None,
        }],
        ..valid_run_report()
    }
    .with_digest()
    .expect("timed-out delivered digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        notifications: vec![RunNotificationDelivery {
            hook_name: "demo".to_owned(),
            event: NotificationEvent::Changed,
            delivered: true,
            timed_out: false,
            exit_code: Some(7),
            duration_ms: 1,
            error: None,
        }],
        ..valid_run_report()
    }
    .with_digest()
    .expect("nonzero delivered digest");
    assert!(report.validate().is_err());

    let report = RunReport {
        persist: RunPersistSection {
            duration_ms: 1,
            wrote_state: false,
            wrote_last_run: true,
            error: Some(valid_process_error()),
        },
        run_outcome: RunOutcome::FailedTransient,
        reason_code: ReasonCode::PersistError,
        failure_class: Some(FailureClass::Transient),
        current_compare_digest_sha256: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("persist error detail digest");
    report.validate().expect("persist error detail report");

    let report = RunReport {
        persist: RunPersistSection {
            duration_ms: 1,
            wrote_state: true,
            wrote_last_run: true,
            error: Some(valid_process_error()),
        },
        ..valid_run_report()
    }
    .with_digest()
    .expect("persist error digest");
    assert!(report.validate().is_err());
}

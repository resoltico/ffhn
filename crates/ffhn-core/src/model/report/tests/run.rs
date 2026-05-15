use super::*;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn run_report_accessors_expose_the_public_contract() {
    let mut report = valid_run_report();
    report.notifications = vec![
        RunNotificationDelivery::delivered("notify-success", 4, 0),
        RunNotificationDelivery::timed_out("notify-timeout", 9, "timed out"),
        RunNotificationDelivery::failed("notify-failure", 7, Some(3), "route failed"),
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
    assert!(matches!(report.result(), RunResult::Changed));
    assert!(!report.result().is_failure());
    assert_eq!(report.failure_cause(), None);
    assert_eq!(report.target_id(), "demo");
    assert_eq!(report.run_started_at(), "2026-04-05T10:15:30Z");
    assert_eq!(report.run_finished_at(), "2026-04-05T10:15:31Z");
    assert_eq!(report.failure_class(), None);
    assert!(report.error_detail().is_none());
    assert_eq!(report.compare_basis(), CompareBasis::CanonicalTextSha256);
    assert_eq!(report.previous_compare_digest_sha256(), Some(DIGEST));
    assert_eq!(report.current_compare_digest_sha256(), Some(DIGEST));
    assert_eq!(
        report.baseline_phase_before_run(),
        BaselinePhase::HasBaseline
    );
    assert_eq!(
        report.baseline_phase_after_run(),
        BaselinePhase::HasBaseline
    );
    let successful = report.successful().expect("successful view");
    let body = successful.body();
    assert_eq!(body.fetch().engine(), FetchEngine::Http);
    assert_eq!(body.extraction().comparison_input_sha256(), DIGEST);
    assert_eq!(body.compare().canonicalizers(), ["trim"]);
    assert_eq!(body.change().kind(), ChangeKind::Changed);
    assert_eq!(body.previous_compare_digest_sha256(), Some(DIGEST));
    assert_eq!(body.current_compare_digest_sha256(), DIGEST);

    let fetch = report.fetch().expect("fetch");
    assert_eq!(fetch.engine(), FetchEngine::Http);
    assert_eq!(fetch.final_url(), Some("https://example.com/final"));
    assert_eq!(fetch.http_status(), Some(200));
    assert_eq!(fetch.content_type(), Some("text/html"));
    assert_eq!(fetch.bytes_read(), Some(42));
    assert_eq!(fetch.duration_ms(), 12);

    let extraction = report.extraction().expect("extraction");
    assert_eq!(extraction.comparison_input_sha256(), DIGEST);
    assert_eq!(extraction.outer_html_sha256(), DIGEST);
    assert_eq!(extraction.selection_kind(), SelectionKind::CssSelector);
    assert_eq!(extraction.selection_match(), SelectionMatch::Single);
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
    assert_eq!(notifications[0].route_name(), "notify-success");
    assert_eq!(notifications[0].duration_ms(), 4);
    assert_eq!(
        notifications[0].status(),
        NotificationDeliveryStatus::Delivered
    );
    assert_eq!(notifications[0].exit_code(), Some(0));
    assert!(notifications[0].error().is_none());
    assert_eq!(notifications[1].route_name(), "notify-timeout");
    assert_eq!(notifications[1].duration_ms(), 9);
    assert_eq!(
        notifications[1].status(),
        NotificationDeliveryStatus::TimedOut
    );
    assert_eq!(notifications[1].exit_code(), None);
    assert_eq!(notifications[1].error(), Some("timed out"));
    assert_eq!(notifications[2].route_name(), "notify-failure");
    assert_eq!(notifications[2].duration_ms(), 7);
    assert_eq!(
        notifications[2].status(),
        NotificationDeliveryStatus::Failed
    );
    assert_eq!(notifications[2].exit_code(), Some(3));
    assert_eq!(notifications[2].error(), Some("route failed"));

    assert_eq!(report.persist().state_commit_duration_ms(), 2);
    assert_eq!(report.persist().last_run_write_duration_ms(), 2);
    assert_eq!(report.persist().total_duration_ms(), 4);
    assert_eq!(report.persist().state_commit().as_str(), "written");
    assert_eq!(report.persist().last_run_write().as_str(), "written");
    assert!(matches!(
        report.persist().state_commit(),
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

    let failed_result = RunResult::FailedTransient {
        cause: RunFailureCause::FetchTimeout,
        error_detail: valid_process_error(),
    };
    assert!(failed_result.is_failure());
    assert_eq!(failed_result.failure_class(), Some(FailureClass::Transient));
    assert_eq!(
        failed_result.error_detail().expect("error detail").kind(),
        ProcessErrorKind::Io
    );
    assert_eq!(
        failed_result.failure_cause(),
        Some(RunFailureCause::FetchTimeout)
    );

    let permanent_result = RunResult::FailedPermanent {
        cause: RunFailureCause::CompareError,
        error_detail: valid_process_error(),
    };
    assert!(permanent_result.is_failure());
    assert_eq!(
        permanent_result.failure_class(),
        Some(FailureClass::Permanent)
    );
    assert_eq!(
        permanent_result.failure_cause(),
        Some(RunFailureCause::CompareError)
    );

    for result in [
        RunResult::Initialized,
        RunResult::Changed,
        RunResult::Unchanged,
        RunResult::SkippedDisabled,
    ] {
        assert!(!result.is_failure());
        assert!(result.failure_class().is_none());
        assert!(result.error_detail().is_none());
        assert!(result.failure_cause().is_none());
    }
    assert_eq!(RunResult::Initialized.outcome().as_str(), "initialized");
    assert_eq!(RunResult::Changed.outcome().as_str(), "changed");
    assert_eq!(RunResult::Unchanged.outcome().as_str(), "unchanged");
    assert_eq!(
        RunResult::SkippedDisabled.outcome().as_str(),
        "skipped_disabled"
    );

    let failed_report = RunReport {
        result: RunResult::FailedTransient {
            cause: RunFailureCause::FetchTimeout,
            error_detail: valid_process_error(),
        },
        current_compare_digest_sha256: None,
        extraction: None,
        compare: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("failed report digest");
    let failed = failed_report.failed().expect("failed view");
    assert_eq!(failed.failure_cause(), RunFailureCause::FetchTimeout);
    assert_eq!(failed.error_detail().kind(), ProcessErrorKind::Io);
    match failed.body() {
        RunBodyView::Fetch { fetch } => {
            assert_eq!(fetch.engine(), FetchEngine::Http);
        }
        other => panic!("unexpected failed body shape: {other:?}"),
    }
}

#[test]
fn run_report_validation_accepts_success_skipped_and_persist_error_shapes() {
    valid_run_report().validate().expect("changed report");

    RunReport {
        result: RunResult::Initialized,
        previous_compare_digest_sha256: None,
        baseline_phase_before_run: BaselinePhase::NeverSucceeded,
        change: Some(RunChangeSection {
            kind: ChangeKind::Initialized,
            previous_text_bytes: None,
            current_text_bytes: 7,
            previous_line_count: None,
            current_line_count: 1,
            common_prefix_lines: 0,
            common_suffix_lines: 0,
            changed_region: None,
        }),
        ..valid_run_report()
    }
    .with_digest()
    .expect("initialized digest")
    .validate()
    .expect("initialized report");

    RunReport {
        result: RunResult::SkippedDisabled,
        previous_compare_digest_sha256: Some(DIGEST.to_owned()),
        current_compare_digest_sha256: None,
        fetch: None,
        extraction: None,
        compare: None,
        change: None,
        baseline_phase_after_run: BaselinePhase::HasBaseline,
        persist: persist_section(1, PersistWriteStatus::Written, PersistWriteStatus::Written),
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped digest")
    .validate()
    .expect("skipped report");

    let persist_error_report = RunReport {
        result: RunResult::FailedTransient {
            cause: RunFailureCause::PersistError,
            error_detail: valid_process_error(),
        },
        current_compare_digest_sha256: Some(DIGEST.to_owned()),
        persist: persist_section(
            1,
            PersistWriteStatus::Failed {
                error: valid_process_error(),
            },
            PersistWriteStatus::NotAttempted,
        ),
        change: None,
        notifications: vec![failed_notification(Some(7), "route failed")],
        ..valid_run_report()
    }
    .with_digest()
    .expect("persist error digest");
    assert_eq!(
        persist_error_report.persist().state_commit().as_str(),
        "failed"
    );
    assert_eq!(
        persist_error_report.persist().last_run_write().as_str(),
        "not_attempted"
    );
    persist_error_report
        .validate()
        .expect("persist error report");

    RunReport {
        result: RunResult::FailedPermanent {
            cause: RunFailureCause::CompareError,
            error_detail: valid_process_error(),
        },
        current_compare_digest_sha256: None,
        change: None,
        fetch: Some(valid_run_report().fetch().expect("fetch").0.clone()),
        extraction: Some(valid_run_report().extraction().expect("extraction").clone()),
        compare: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("permanent failure digest")
    .validate()
    .expect("permanent failure report");
}

#[test]
fn run_report_validation_rejects_invalid_result_and_stage_contracts() {
    let transient_reason_mismatch = RunReport {
        result: RunResult::FailedTransient {
            cause: RunFailureCause::ConfigInvalid,
            error_detail: valid_process_error(),
        },
        current_compare_digest_sha256: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("transient mismatch digest");
    assert!(transient_reason_mismatch.validate().is_err());

    let permanent_reason_mismatch = RunReport {
        result: RunResult::FailedPermanent {
            cause: RunFailureCause::FetchTimeout,
            error_detail: valid_process_error(),
        },
        current_compare_digest_sha256: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("permanent mismatch digest");
    assert!(permanent_reason_mismatch.validate().is_err());

    let missing_current_digest = RunReport {
        current_compare_digest_sha256: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("missing current digest");
    assert!(missing_current_digest.validate().is_err());

    let successful_missing_current_digest_without_compare = RunReport {
        current_compare_digest_sha256: None,
        compare: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("successful missing current digest without compare");
    assert!(
        successful_missing_current_digest_without_compare
            .validate()
            .is_err()
    );

    let successful_missing_compare_section = RunReport {
        compare: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("successful missing compare section");
    assert!(successful_missing_compare_section.validate().is_err());

    let persist_error_missing_current_digest = RunReport {
        result: RunResult::FailedTransient {
            cause: RunFailureCause::PersistError,
            error_detail: valid_process_error(),
        },
        current_compare_digest_sha256: None,
        persist: persist_section(
            1,
            PersistWriteStatus::Failed {
                error: valid_process_error(),
            },
            PersistWriteStatus::NotAttempted,
        ),
        ..valid_run_report()
    }
    .with_digest()
    .expect("persist error missing current digest");
    assert!(persist_error_missing_current_digest.validate().is_err());

    let config_invalid_with_display_name = RunReport {
        result: RunResult::FailedPermanent {
            cause: RunFailureCause::ConfigInvalid,
            error_detail: valid_process_error(),
        },
        current_compare_digest_sha256: None,
        fetch: None,
        extraction: None,
        compare: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("config invalid with display name digest");
    assert!(config_invalid_with_display_name.validate().is_err());

    let missing_display_name_for_trusted_target = RunReport {
        display_name: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("missing display name digest");
    assert!(missing_display_name_for_trusted_target.validate().is_err());

    let digest_mismatch = RunReport {
        run_report_digest_sha256:
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
        ..valid_run_report()
    };
    assert!(digest_mismatch.validate().is_err());

    let missing_success_sections = RunReport {
        fetch: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("missing success sections");
    assert!(missing_success_sections.validate().is_err());

    let missing_extraction_section = RunReport {
        extraction: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("missing extraction section");
    assert!(missing_extraction_section.validate().is_err());

    let missing_compare_section = RunReport {
        compare: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("missing compare section");
    assert!(missing_compare_section.validate().is_err());

    let skipped_with_fetch = RunReport {
        result: RunResult::SkippedDisabled,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped with fetch");
    assert!(skipped_with_fetch.validate().is_err());

    let skipped_with_extraction = RunReport {
        result: RunResult::SkippedDisabled,
        fetch: None,
        compare: None,
        current_compare_digest_sha256: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped with extraction");
    assert!(skipped_with_extraction.validate().is_err());

    let skipped_with_compare = RunReport {
        result: RunResult::SkippedDisabled,
        fetch: None,
        extraction: None,
        current_compare_digest_sha256: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped with compare");
    assert!(skipped_with_compare.validate().is_err());

    let skipped_with_current_digest = RunReport {
        result: RunResult::SkippedDisabled,
        fetch: None,
        extraction: None,
        compare: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped with current digest");
    assert!(skipped_with_current_digest.validate().is_err());

    let failed_with_current_digest = RunReport {
        result: RunResult::FailedTransient {
            cause: RunFailureCause::FetchTimeout,
            error_detail: valid_process_error(),
        },
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("failed with current digest");
    assert!(failed_with_current_digest.validate().is_err());

    let invalid_extraction_index = RunReport {
        extraction: Some(RunExtractionSection {
            selected_candidate_index: 2,
            ..valid_run_report().extraction.expect("extraction")
        }),
        ..valid_run_report()
    }
    .with_digest()
    .expect("invalid extraction index");
    assert!(invalid_extraction_index.validate().is_err());

    let invalid_zero_extraction_index = RunReport {
        extraction: Some(RunExtractionSection {
            selected_candidate_index: 0,
            ..valid_run_report().extraction.expect("extraction")
        }),
        ..valid_run_report()
    }
    .with_digest()
    .expect("invalid zero extraction index");
    assert!(invalid_zero_extraction_index.validate().is_err());

    let persist_failure_without_persist_cause = RunReport {
        result: RunResult::FailedTransient {
            cause: RunFailureCause::FetchTimeout,
            error_detail: valid_process_error(),
        },
        current_compare_digest_sha256: None,
        change: None,
        persist: persist_section(
            1,
            PersistWriteStatus::Failed {
                error: valid_process_error(),
            },
            PersistWriteStatus::NotAttempted,
        ),
        ..valid_run_report()
    }
    .with_digest()
    .expect("persist mismatch");
    assert!(persist_failure_without_persist_cause.validate().is_err());

    let persist_cause_without_persist_failure = RunReport {
        result: RunResult::FailedTransient {
            cause: RunFailureCause::PersistError,
            error_detail: valid_process_error(),
        },
        current_compare_digest_sha256: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("persist cause without persist failure");
    assert!(persist_cause_without_persist_failure.validate().is_err());

    let dry_run_phase_transition = RunReport {
        run_mode: RunMode::DryRun,
        baseline_phase_before_run: BaselinePhase::NeverSucceeded,
        baseline_phase_after_run: BaselinePhase::HasBaseline,
        persist: persist_section(
            1,
            PersistWriteStatus::NotAttempted,
            PersistWriteStatus::NotAttempted,
        ),
        notifications: Vec::new(),
        ..valid_run_report()
    }
    .with_digest()
    .expect("dry-run phase transition");
    assert!(dry_run_phase_transition.validate().is_err());

    let dry_run_skipped_disabled = RunReport {
        run_mode: RunMode::DryRun,
        result: RunResult::SkippedDisabled,
        fetch: None,
        extraction: None,
        compare: None,
        current_compare_digest_sha256: None,
        change: None,
        persist: persist_section(
            1,
            PersistWriteStatus::NotAttempted,
            PersistWriteStatus::NotAttempted,
        ),
        notifications: Vec::new(),
        ..valid_run_report()
    }
    .with_digest()
    .expect("dry-run skipped disabled");
    dry_run_skipped_disabled
        .validate()
        .expect("dry-run skipped disabled");

    let skipped_disabled_with_phase_transition = RunReport {
        result: RunResult::SkippedDisabled,
        fetch: None,
        extraction: None,
        compare: None,
        current_compare_digest_sha256: None,
        change: None,
        baseline_phase_before_run: BaselinePhase::NeverSucceeded,
        baseline_phase_after_run: BaselinePhase::HasBaseline,
        persist: persist_section(1, PersistWriteStatus::Written, PersistWriteStatus::Written),
        notifications: Vec::new(),
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped disabled phase transition");
    assert!(skipped_disabled_with_phase_transition.validate().is_err());

    let skipped_disabled_with_fetch = RunReport {
        result: RunResult::SkippedDisabled,
        extraction: None,
        compare: None,
        current_compare_digest_sha256: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped disabled with fetch");
    assert!(skipped_disabled_with_fetch.validate().is_err());

    let skipped_disabled_with_extraction = RunReport {
        result: RunResult::SkippedDisabled,
        fetch: None,
        compare: None,
        current_compare_digest_sha256: None,
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped disabled with extraction");
    assert!(skipped_disabled_with_extraction.validate().is_err());

    let skipped_disabled_with_compare = RunReport {
        result: RunResult::SkippedDisabled,
        fetch: None,
        extraction: None,
        current_compare_digest_sha256: Some(DIGEST.to_owned()),
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("skipped disabled with compare");
    assert!(skipped_disabled_with_compare.validate().is_err());

    let live_success_without_ready_status = RunReport {
        baseline_phase_after_run: BaselinePhase::NeverSucceeded,
        ..valid_run_report()
    }
    .with_digest()
    .expect("live success without baseline");
    assert!(live_success_without_ready_status.validate().is_err());

    let initialized_with_ready_baseline_before = RunReport {
        result: RunResult::Initialized,
        previous_compare_digest_sha256: None,
        change: Some(RunChangeSection {
            kind: ChangeKind::Initialized,
            previous_text_bytes: None,
            current_text_bytes: 7,
            previous_line_count: None,
            current_line_count: 1,
            common_prefix_lines: 0,
            common_suffix_lines: 0,
            changed_region: None,
        }),
        baseline_phase_before_run: BaselinePhase::HasBaseline,
        ..valid_run_report()
    }
    .with_digest()
    .expect("initialized with ready baseline before");
    assert!(initialized_with_ready_baseline_before.validate().is_err());

    let changed_without_existing_baseline = RunReport {
        baseline_phase_before_run: BaselinePhase::NeverSucceeded,
        ..valid_run_report()
    }
    .with_digest()
    .expect("changed without existing baseline");
    assert!(changed_without_existing_baseline.validate().is_err());
}

#[test]
fn run_report_validation_rejects_dry_run_mutation_and_notification_shapes() {
    let dry_run_last_run = RunReport {
        run_mode: RunMode::DryRun,
        persist: persist_section(
            1,
            PersistWriteStatus::NotAttempted,
            PersistWriteStatus::Written,
        ),
        ..valid_run_report()
    }
    .with_digest()
    .expect("dry-run last-run");
    assert!(dry_run_last_run.validate().is_err());

    let dry_run_notification = RunReport {
        run_mode: RunMode::DryRun,
        persist: persist_section(
            1,
            PersistWriteStatus::NotAttempted,
            PersistWriteStatus::NotAttempted,
        ),
        notifications: vec![failed_notification(Some(7), "failed")],
        ..valid_run_report()
    }
    .with_digest()
    .expect("dry-run notification");
    assert!(dry_run_notification.validate().is_err());

    let invalid_notification = RunReport {
        notifications: vec![RunNotificationDelivery::failed("notify", 1, Some(0), "bad")],
        ..valid_run_report()
    }
    .with_digest()
    .expect("invalid notification");
    assert!(invalid_notification.validate().is_err());
}

#[test]
fn run_report_validation_rejects_invalid_fetch_extraction_and_change_sections() {
    let invalid_url = RunReport {
        fetch: Some(RunFetchSection {
            final_url: Some("not a url".to_owned()),
            ..valid_run_report().fetch().expect("fetch").0.clone()
        }),
        ..valid_run_report()
    }
    .with_digest()
    .expect("invalid url digest");
    assert!(invalid_url.validate().is_err());

    let invalid_extraction = RunReport {
        extraction: Some(RunExtractionSection {
            candidate_count: 0,
            ..valid_run_report().extraction().expect("extraction").clone()
        }),
        ..valid_run_report()
    }
    .with_digest()
    .expect("invalid extraction digest");
    assert!(invalid_extraction.validate().is_err());

    let invalid_change = RunReport {
        change: Some(RunChangeSection {
            kind: ChangeKind::Changed,
            previous_text_bytes: Some(1),
            current_text_bytes: 0,
            previous_line_count: Some(1),
            current_line_count: 0,
            common_prefix_lines: 2,
            common_suffix_lines: 0,
            changed_region: None,
        }),
        ..valid_run_report()
    }
    .with_digest()
    .expect("invalid change digest");
    assert!(invalid_change.validate().is_err());
}

#[test]
fn run_report_deserialization_revalidates_and_rejects_legacy_shapes() {
    let report = valid_run_report();
    let json = serde_json::to_string(&report).expect("run json");
    let parsed: RunReport = serde_json::from_str(&json).expect("run report");
    assert_eq!(parsed, report);

    let legacy_json = serde_json::json!({
        "schema_name": RUN_REPORT_SCHEMA_NAME,
        "schema_version": RUN_REPORT_SCHEMA_VERSION,
        "run_report_digest_sha256": DIGEST,
        "target_id": "demo",
        "run_started_at": "2026-04-05T10:15:30Z",
        "run_finished_at": "2026-04-05T10:15:31Z",
        "run_mode": "live",
        "run_outcome": "changed",
        "reason_code": "ok",
        "compare_basis": "canonical_text_sha256",
        "current_compare_digest_sha256": DIGEST,
        "baseline_phase_before_run": "has_baseline",
        "baseline_phase_after_run": "has_baseline",
        "persist": {
            "duration_ms": 1,
            "state_write": {"status": "written"},
            "last_run_write": {"status": "written"}
        }
    });
    assert!(serde_json::from_value::<RunReport>(legacy_json).is_err());
}

#[test]
fn run_report_body_views_cover_partial_empty_and_incoherent_shapes() {
    let report = valid_run_report();
    let fetch = report.fetch().expect("fetch").0.clone();
    let extraction = report.extraction().expect("extraction").clone();
    let compare = report.compare().expect("compare").0.clone();

    let fetch_extraction_compare = RunReport {
        change: None,
        ..report.clone()
    }
    .with_digest()
    .expect("fetch extraction compare digest");
    match fetch_extraction_compare.body() {
        RunBodyView::FetchExtractionCompare {
            fetch,
            extraction,
            compare,
        } => {
            assert_eq!(fetch.engine(), FetchEngine::Http);
            assert_eq!(extraction.selection_kind(), SelectionKind::CssSelector);
            assert_eq!(compare.canonicalizers(), ["trim"]);
        }
        other => panic!("unexpected body view: {other:?}"),
    }

    let fetch_and_extraction = RunReport {
        compare: None,
        change: None,
        ..report.clone()
    }
    .with_digest()
    .expect("fetch and extraction digest");
    match fetch_and_extraction.body() {
        RunBodyView::FetchAndExtraction { fetch, extraction } => {
            assert_eq!(fetch.engine(), FetchEngine::Http);
            assert_eq!(extraction.output_kind(), OutputKind::OuterHtml);
        }
        other => panic!("unexpected body view: {other:?}"),
    }

    let fetch_only = RunReport {
        extraction: None,
        compare: None,
        change: None,
        current_compare_digest_sha256: None,
        ..report.clone()
    }
    .with_digest()
    .expect("fetch only digest");
    match fetch_only.body() {
        RunBodyView::Fetch { fetch } => {
            assert_eq!(fetch.engine(), FetchEngine::Http);
        }
        other => panic!("unexpected body view: {other:?}"),
    }

    let no_body = RunReport {
        result: RunResult::SkippedDisabled,
        fetch: None,
        extraction: None,
        compare: None,
        change: None,
        current_compare_digest_sha256: None,
        previous_compare_digest_sha256: Some(DIGEST.to_owned()),
        persist: persist_section(1, PersistWriteStatus::Written, PersistWriteStatus::Written),
        ..report.clone()
    }
    .with_digest()
    .expect("no body digest");
    assert!(matches!(no_body.body(), RunBodyView::None));
    assert!(no_body.failed().is_none());
    assert!(no_body.successful().is_none());

    let incoherent = RunReport {
        fetch: Some(fetch),
        extraction: None,
        compare: Some(compare.clone()),
        change: None,
        current_compare_digest_sha256: None,
        ..report
    }
    .with_digest()
    .expect("incoherent digest");
    assert!(matches!(incoherent.body(), RunBodyView::None));

    let failed_report = RunReport {
        result: RunResult::FailedPermanent {
            cause: RunFailureCause::CompareError,
            error_detail: valid_process_error(),
        },
        fetch: Some(fetch_only.fetch().expect("failed fetch").0.clone()),
        extraction: Some(extraction),
        compare: Some(compare),
        change: None,
        current_compare_digest_sha256: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("failed digest");
    assert!(failed_report.successful().is_none());
    assert!(failed_report.failed().is_some());
}

#[test]
#[should_panic(expected = "successful runs always carry a full reportable body")]
fn successful_run_view_panics_when_the_body_is_not_reportable() {
    let report = RunReport {
        change: None,
        ..valid_run_report()
    }
    .with_digest()
    .expect("digest");

    let _ = report.successful().expect("successful view").body();
}

#[test]
fn run_report_validation_rejects_nonzero_not_attempted_persist_durations() {
    let invalid_state_commit_duration = RunReport {
        persist: RunPersistSection::from_writes(
            1,
            PersistWriteStatus::NotAttempted,
            0,
            PersistWriteStatus::NotAttempted,
        ),
        run_mode: RunMode::DryRun,
        notifications: Vec::new(),
        ..valid_run_report()
    }
    .with_digest()
    .expect("invalid state commit duration");
    assert!(invalid_state_commit_duration.validate().is_err());

    let invalid_last_run_duration = RunReport {
        persist: RunPersistSection::from_writes(
            0,
            PersistWriteStatus::NotAttempted,
            1,
            PersistWriteStatus::NotAttempted,
        ),
        run_mode: RunMode::DryRun,
        notifications: Vec::new(),
        ..valid_run_report()
    }
    .with_digest()
    .expect("invalid last-run duration");
    assert!(invalid_last_run_duration.validate().is_err());
}

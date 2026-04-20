use super::*;
use crate::{
    HTMLCUT_INTEROP_PROFILE, RUN_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_VERSION,
    STATUS_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_VERSION,
};

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn valid_run_report() -> RunReport {
    RunReport {
        schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: RUN_REPORT_SCHEMA_VERSION,
        run_report_digest_sha256: String::new(),
        target_id: "demo".to_owned(),
        run_started_at: "2026-04-05T10:15:30Z".to_owned(),
        run_finished_at: "2026-04-05T10:15:31Z".to_owned(),
        run_mode: RunMode::Live,
        run_outcome: RunOutcome::Changed,
        reason_code: ReasonCode::Ok,
        failure_class: None,
        target_status_after_run: TargetStatus::Ready,
        compare_basis: CompareBasis::CanonicalTextSha256,
        previous_compare_digest_sha256: Some(DIGEST.to_owned()),
        current_compare_digest_sha256: Some(DIGEST.to_owned()),
        state_phase_before_run: StatePhase::HasBaseline,
        state_phase_after_run: StatePhase::HasBaseline,
        fetch: Some(RunFetchSection {
            engine: FetchEngine::Http,
            final_url: Some("https://example.com/final".to_owned()),
            http_status: Some(200),
            content_type: Some("text/html".to_owned()),
            bytes_read: Some(42),
            duration_ms: 12,
        }),
        extraction: Some(RunExtractionSection {
            interop_profile: HTMLCUT_INTEROP_PROFILE.to_owned(),
            htmlcut_plan_digest_sha256: DIGEST.to_owned(),
            htmlcut_result_digest_sha256: DIGEST.to_owned(),
            comparison_input_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            strategy_kind: SelectionKind::CssSelector,
            selection_mode: SelectionMatch::Single,
            output_kind: OutputKind::OuterHtml,
            candidate_count: 1,
            selected_candidate_index: 1,
            warning_codes: Vec::new(),
            duration_ms: 8,
        }),
        compare: Some(RunCompareSection {
            canonicalizers: vec!["trim".to_owned()],
            duration_ms: 3,
        }),
        change: Some(RunChangeSection {
            kind: ChangeKind::Changed,
            previous_text_bytes: Some(6),
            current_text_bytes: 7,
            previous_line_count: Some(1),
            current_line_count: 1,
            common_prefix_lines: 0,
            common_suffix_lines: 0,
            changed_region: Some(RunChangeRegion {
                previous_start_line: 1,
                previous_line_count: 1,
                current_start_line: 1,
                current_line_count: 1,
                previous_excerpt: Some("Before".to_owned()),
                current_excerpt: Some("Changed".to_owned()),
                previous_excerpt_sha256: Some(DIGEST.to_owned()),
                current_excerpt_sha256: Some(DIGEST.to_owned()),
            }),
        }),
        persist: RunPersistSection {
            duration_ms: 2,
            wrote_state: true,
            wrote_last_run: true,
        },
        notifications: Vec::new(),
        extensions: None,
    }
    .with_digest()
    .expect("digest")
}

fn valid_batch_report() -> BatchRunReport {
    BatchRunReport {
        schema_name: BATCH_RUN_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: BATCH_RUN_REPORT_SCHEMA_VERSION,
        run_mode: RunMode::Live,
        watch_root: "watchlist".to_owned(),
        requested_targets: vec!["demo".to_owned(), "fatal_target".to_owned()],
        run_started_at: "2026-04-05T10:15:30Z".to_owned(),
        run_finished_at: "2026-04-05T10:15:31Z".to_owned(),
        max_concurrency: 2,
        entries: vec![
            BatchRunEntry {
                target_id: "demo".to_owned(),
                run_report: Some(valid_run_report()),
                fatal_error: None,
            },
            BatchRunEntry {
                target_id: "fatal_target".to_owned(),
                run_report: None,
                fatal_error: Some("filesystem error".to_owned()),
            },
        ],
        outcome_counts: BatchOutcomeCounts {
            initialized: 0,
            changed: 1,
            unchanged: 0,
            failed_transient: 0,
            failed_permanent: 0,
            skipped_disabled: 0,
            fatal_error: 1,
        },
        extensions: None,
    }
}

#[test]
fn run_report_validation_accepts_a_digest_checked_success_report() {
    valid_run_report().validate().expect("run report");
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
}

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

#[test]
fn batch_run_report_validation_covers_success_fatal_and_mismatch_cases() {
    valid_batch_report().validate().expect("valid batch report");

    let invalid = BatchRunReport {
        max_concurrency: 0,
        ..valid_batch_report()
    };
    assert!(invalid.validate().is_err());

    let invalid = BatchRunReport {
        requested_targets: vec!["demo".to_owned(), "demo".to_owned()],
        ..valid_batch_report()
    };
    assert!(invalid.validate().is_err());

    let invalid = BatchRunReport {
        entries: vec![valid_batch_report().entries[0].clone()],
        ..valid_batch_report()
    };
    assert!(invalid.validate().is_err());

    let mut wrong_target = valid_batch_report();
    wrong_target.entries[0]
        .run_report
        .as_mut()
        .expect("run report")
        .target_id = "other".to_owned();
    wrong_target.entries[0].run_report = Some(
        wrong_target.entries[0]
            .run_report
            .take()
            .expect("run report")
            .with_digest()
            .expect("wrong-target digest"),
    );
    assert!(wrong_target.validate().is_err());

    let invalid = BatchRunReport {
        entries: vec![BatchRunEntry {
            target_id: "demo".to_owned(),
            run_report: Some(valid_run_report()),
            fatal_error: Some("both present".to_owned()),
        }],
        requested_targets: vec!["demo".to_owned()],
        outcome_counts: BatchOutcomeCounts {
            initialized: 0,
            changed: 1,
            unchanged: 0,
            failed_transient: 0,
            failed_permanent: 0,
            skipped_disabled: 0,
            fatal_error: 0,
        },
        ..valid_batch_report()
    };
    assert!(invalid.validate().is_err());

    let invalid = BatchRunReport {
        outcome_counts: BatchOutcomeCounts {
            changed: 0,
            ..valid_batch_report().outcome_counts
        },
        ..valid_batch_report()
    };
    assert!(invalid.validate().is_err());
}

#[test]
fn status_report_validation_enforces_state_phase_rules() {
    StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: "demo".to_owned(),
        target_status: TargetStatus::Invalid,
        reason_code: ReasonCode::ConfigInvalid,
        state_phase: None,
        artifacts: ArtifactStatus {
            current_valid: false,
            previous_valid: false,
        },
        current_snapshot: None,
        snapshot_history: Vec::new(),
        extensions: None,
    }
    .validate()
    .expect("config invalid report");

    let invalid = StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: "demo".to_owned(),
        target_status: TargetStatus::Ready,
        reason_code: ReasonCode::Ok,
        state_phase: None,
        artifacts: ArtifactStatus {
            current_valid: true,
            previous_valid: true,
        },
        current_snapshot: None,
        snapshot_history: Vec::new(),
        extensions: None,
    };
    assert!(invalid.validate().is_err());

    let wrong_target_status = StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: "demo".to_owned(),
        target_status: TargetStatus::Ready,
        reason_code: ReasonCode::ConfigInvalid,
        state_phase: None,
        artifacts: ArtifactStatus {
            current_valid: false,
            previous_valid: false,
        },
        current_snapshot: None,
        snapshot_history: Vec::new(),
        extensions: None,
    };
    assert!(wrong_target_status.validate().is_err());

    let invalid_identity = StatusReport {
        schema_name: "wrong".to_owned(),
        ..StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            target_status: TargetStatus::Ready,
            reason_code: ReasonCode::Ok,
            state_phase: Some(StatePhase::HasBaseline),
            artifacts: ArtifactStatus {
                current_valid: true,
                previous_valid: true,
            },
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        }
    };
    assert!(invalid_identity.validate().is_err());

    let mut invalid_snapshot = StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: "demo".to_owned(),
        target_status: TargetStatus::Ready,
        reason_code: ReasonCode::Ok,
        state_phase: Some(StatePhase::HasBaseline),
        artifacts: ArtifactStatus {
            current_valid: true,
            previous_valid: true,
        },
        current_snapshot: Some(SnapshotDigestSummary {
            canonical_text_sha256: "bad".to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            captured_at: "2026-04-05T10:15:30Z".to_owned(),
        }),
        snapshot_history: Vec::new(),
        extensions: None,
    };
    assert!(invalid_snapshot.validate().is_err());

    invalid_snapshot.current_snapshot = Some(SnapshotDigestSummary {
        canonical_text_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        captured_at: "2026-04-05T10:15:30Z".to_owned(),
    });
    invalid_snapshot.snapshot_history = vec![SnapshotDigestSummary {
        canonical_text_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        captured_at: "2026-04-05T10:14:30Z".to_owned(),
    }];
    invalid_snapshot.validate().expect("ready status report");
}

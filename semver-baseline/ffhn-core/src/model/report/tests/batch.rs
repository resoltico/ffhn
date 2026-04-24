use super::*;
use std::collections::BTreeMap;

use serde_json::json;

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
        .target_id = target_id("other");
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
            fatal_error: Some(valid_process_error()),
        }],
        requested_targets: vec!["demo".to_owned()],
        outcome_counts: BatchOutcomeCounts {
            initialized: 0,
            changed: 1,
            unchanged: 0,
            failed_transient: 0,
            failed_permanent: 0,
            skipped_disabled: 0,
            persist_error: 0,
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

    let mut persist_error = valid_batch_report();
    persist_error.entries[0].run_report = Some(
        RunReport {
            run_outcome: RunOutcome::FailedTransient,
            reason_code: ReasonCode::PersistError,
            failure_class: Some(FailureClass::Transient),
            current_compare_digest_sha256: None,
            persist: RunPersistSection {
                duration_ms: 1,
                wrote_state: false,
                wrote_last_run: true,
                error: Some(valid_process_error()),
            },
            ..valid_run_report()
        }
        .with_digest()
        .expect("persist error digest"),
    );
    persist_error.outcome_counts.changed = 0;
    persist_error.outcome_counts.failed_transient = 1;
    persist_error.outcome_counts.persist_error = 1;
    persist_error
        .validate()
        .expect("batch report with persist-error entry");
}

#[test]
fn batch_run_report_constructor_and_deserialization_preserve_ordered_contract_inputs() {
    let report = BatchRunReport::new(
        BatchRunReportInput::new(
            RunMode::DryRun,
            "watchlist".to_owned(),
            vec!["demo".to_owned()],
            "2026-04-05T10:15:30Z".to_owned(),
            "2026-04-05T10:15:31Z".to_owned(),
            1,
            vec![BatchRunEntry {
                target_id: "demo".to_owned(),
                run_report: Some(
                    RunReport {
                        run_mode: RunMode::DryRun,
                        persist: RunPersistSection {
                            duration_ms: 1,
                            wrote_state: false,
                            wrote_last_run: false,
                            error: None,
                        },
                        ..valid_run_report()
                    }
                    .with_digest()
                    .expect("dry-run report digest"),
                ),
                fatal_error: None,
            }],
        )
        .expect("batch report input"),
    )
    .expect("constructed batch report");

    assert_eq!(report.watch_root(), "watchlist");
    assert_eq!(report.requested_targets(), ["demo"]);
    assert_eq!(report.run_started_at(), "2026-04-05T10:15:30Z");
    assert_eq!(report.run_finished_at(), "2026-04-05T10:15:31Z");
    assert_eq!(report.max_concurrency(), 1);
    assert_eq!(report.entries().len(), 1);
    assert_eq!(report.entries()[0].target_id(), "demo");

    let json = serde_json::to_string(&valid_batch_report()).expect("batch json");
    let parsed: BatchRunReport = serde_json::from_str(&json).expect("deserialize batch report");
    assert_eq!(parsed.requested_targets(), ["demo", "fatal_target"]);
    assert_eq!(parsed.entries().len(), 2);

    let input = BatchRunReportInput::new(
        RunMode::DryRun,
        "watchlist".to_owned(),
        vec!["demo".to_owned()],
        "2026-04-05T10:15:30Z".to_owned(),
        "2026-04-05T10:15:31Z".to_owned(),
        1,
        vec![BatchRunEntry {
            target_id: "demo".to_owned(),
            run_report: Some(
                RunReport {
                    run_mode: RunMode::DryRun,
                    persist: RunPersistSection {
                        duration_ms: 1,
                        wrote_state: false,
                        wrote_last_run: false,
                        error: None,
                    },
                    ..valid_run_report()
                }
                .with_digest()
                .expect("dry-run report digest"),
            ),
            fatal_error: None,
        }],
    )
    .expect("batch report input")
    .with_extensions(Some(BTreeMap::from([(
        "demo".to_owned(),
        json!({"kind": "extension"}),
    )])));
    assert!(BatchRunReport::new(input).is_ok());
}

#[test]
fn batch_run_report_deserialization_rejects_entry_order_mismatches() {
    let mut invalid = valid_batch_report();
    invalid.requested_targets = vec!["fatal_target".to_owned(), "demo".to_owned()];

    let json = serde_json::to_string(&invalid).expect("invalid batch json");
    assert!(serde_json::from_str::<BatchRunReport>(&json).is_err());
}

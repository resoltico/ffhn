use super::*;

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

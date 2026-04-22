use std::collections::BTreeSet;

use super::*;
use crate::{RUN_REPORT_SCHEMA_NAME, RunMode, STATUS_REPORT_SCHEMA_NAME};

#[test]
fn cli_contract_catalog_keeps_ids_unique() {
    let operation_ids = cli_contract()
        .operations
        .iter()
        .map(|operation| operation.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(operation_ids.len(), cli_contract().operations.len());

    let invocation_ids = cli_contract()
        .operations
        .iter()
        .flat_map(|operation| operation.invocations.iter().map(|invocation| invocation.id))
        .collect::<BTreeSet<_>>();
    assert_eq!(invocation_ids.len(), 4);

    let document_ids = cli_contract()
        .documents
        .iter()
        .map(|document| document.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(document_ids.len(), cli_contract().documents.len());
}

#[test]
fn cli_contract_lookups_cover_present_and_missing_ids() {
    assert_eq!(
        cli_operation(CLI_OPERATION_RUN_ID)
            .expect("run operation")
            .display_label,
        "Run"
    );
    assert!(cli_operation("bogus").is_none());

    assert_eq!(
        cli_document(RUN_REPORT_SCHEMA_NAME)
            .expect("run report document")
            .display_label,
        "Run Report"
    );
    assert_eq!(
        cli_document(RUN_REPORT_SCHEMA_NAME)
            .expect("run report document")
            .render_cli_write_error(),
        "could not write run report"
    );
    assert!(cli_document("ffhn.unknown_report").is_none());

    assert_eq!(
        cli_hard_limit(CLI_LIMIT_IMMEDIATE_DISCOVERY_DEPTH_ID)
            .expect("watch-root discovery limit")
            .display_label,
        "Immediate Watch-Root Discovery"
    );
    assert!(cli_hard_limit("bogus-limit").is_none());
}

#[test]
fn execution_modes_and_usage_errors_use_the_canonical_contract() {
    let live = cli_execution_mode(RunMode::Live);
    let dry_run = cli_execution_mode(RunMode::DryRun);

    assert_eq!(live.id, "live");
    assert!(live.writes_state);
    assert!(live.writes_last_run);
    assert!(live.delivers_notifications);

    assert_eq!(dry_run.id, "dry_run");
    assert!(!dry_run.writes_state);
    assert!(!dry_run.writes_last_run);
    assert!(!dry_run.delivers_notifications);

    assert_eq!(
        positive_batch_concurrency_usage_error(),
        "must be a positive integer"
    );
    assert_eq!(
        duplicate_target_ids_usage_error("demo"),
        "duplicate --target values are not allowed: demo"
    );
    assert_eq!(
        document_write_error(STATUS_REPORT_SCHEMA_NAME).expect("status write error"),
        "could not write status report"
    );
    assert!(document_write_error("ffhn.unknown_report").is_none());
}

#[test]
fn hard_limit_rendering_covers_plain_and_placeholder_templates() {
    let plain = cli_hard_limit(CLI_LIMIT_POSITIVE_BATCH_CONCURRENCY_ID)
        .expect("positive batch concurrency limit");
    let placeholder =
        cli_hard_limit(CLI_LIMIT_UNIQUE_TARGET_IDS_ID).expect("unique target ids limit");

    assert_eq!(
        plain.render_cli_usage_error(Some("ignored")),
        "must be a positive integer"
    );
    assert_eq!(
        placeholder.render_cli_usage_error(None),
        "duplicate --target values are not allowed: {detail}"
    );
    assert_eq!(
        placeholder.render_cli_usage_error(Some("demo")),
        "duplicate --target values are not allowed: demo"
    );
}

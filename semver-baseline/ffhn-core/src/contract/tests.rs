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
    assert_eq!(run_operation().id, CLI_OPERATION_RUN_ID);
    assert_eq!(status_operation().id, CLI_OPERATION_STATUS_ID);
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
    assert_eq!(run_report_document().id, RUN_REPORT_SCHEMA_NAME);
    assert!(cli_document("ffhn.unknown_report").is_none());

    assert_eq!(
        cli_hard_limit(CLI_LIMIT_IMMEDIATE_DISCOVERY_DEPTH_ID)
            .expect("watch-root discovery limit")
            .display_label,
        "Immediate Watch Root Discovery"
    );
    assert_eq!(
        cli_hard_limit(CLI_LIMIT_DURABLE_TARGET_IDS_ID)
            .expect("durable target id limit")
            .display_label,
        "Durable Explicit Target Ids"
    );
    assert!(cli_hard_limit("bogus-limit").is_none());
}

#[test]
fn execution_modes_and_usage_errors_use_the_canonical_contract() {
    let live = cli_execution_mode(RunMode::Live);
    let dry_run = cli_execution_mode(RunMode::DryRun);
    let run = run_operation();
    let status = status_operation();

    assert_eq!(
        run.usage,
        "ffhn run (--target <ID>... | --all) [--watch-root <PATH>] [--jobs <N>] [--dry-run] [--format <FORMAT>]"
    );
    assert_eq!(run.examples[0], "ffhn run --target demo");
    assert_eq!(
        run.output_notes[0],
        "`ffhn run --target <ID>` produces one `ffhn.run_report` result in the selected format."
    );
    assert_eq!(
        run.operational_notes[0],
        "The watch root must already exist and be a directory."
    );
    assert_eq!(
        status.usage,
        "ffhn status --target <ID> [--watch-root <PATH>] [--format <FORMAT>]"
    );
    assert_eq!(status.examples[0], "ffhn status --target demo");
    assert_eq!(
        status.output_notes[0],
        "Produces one `ffhn.status_report` result in the selected format."
    );
    assert_eq!(
        status.operational_notes[1],
        "Status waits behind any active live run so it can inspect one stable target view."
    );

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
        "`--jobs <N>` must be a positive integer"
    );
    assert_eq!(
        positive_batch_concurrency_limit().id,
        CLI_LIMIT_POSITIVE_BATCH_CONCURRENCY_ID
    );
    assert_eq!(
        run_target_selection_usage_error(),
        "one of '--target <ID>' or '--all' is required"
    );
    assert_eq!(unique_target_ids_limit().id, CLI_LIMIT_UNIQUE_TARGET_IDS_ID);
    assert_eq!(
        duplicate_target_ids_usage_error("demo"),
        "duplicate --target values are not allowed: demo"
    );
    assert_eq!(
        document_write_error(STATUS_REPORT_SCHEMA_NAME).expect("status write error"),
        "could not write status report"
    );
    assert_eq!(status_report_document().id, STATUS_REPORT_SCHEMA_NAME);
    assert_eq!(run_report_write_error(), "could not write run report");
    assert_eq!(
        batch_run_report_write_error(),
        "could not write batch run report"
    );
    assert_eq!(status_report_write_error(), "could not write status report");
    assert!(document_write_error("ffhn.unknown_report").is_none());
}

#[test]
fn hard_limit_rendering_covers_plain_and_placeholder_templates() {
    let plain = cli_hard_limit(CLI_LIMIT_POSITIVE_BATCH_CONCURRENCY_ID)
        .expect("positive batch concurrency limit");
    let run_selection =
        cli_hard_limit(CLI_LIMIT_RUN_TARGET_SELECTION_ID).expect("run target selection limit");
    let placeholder =
        cli_hard_limit(CLI_LIMIT_UNIQUE_TARGET_IDS_ID).expect("unique target ids limit");

    assert_eq!(
        plain.render_cli_usage_error(Some("ignored")),
        "`--jobs <N>` must be a positive integer"
    );
    assert_eq!(
        run_selection.render_cli_usage_error(None),
        "one of '--target <ID>' or '--all' is required"
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

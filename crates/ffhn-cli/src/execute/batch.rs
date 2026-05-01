use std::io::Write;
use std::path::Path;

use ffhn_core::{
    BatchRunEntry, BatchRunReport, BatchRunReportInput, CoreError, NotificationDeliveryStatus,
    ProcessErrorDetail, ProcessErrorKind, RunMode, RunOutcome, batch_run_report_write_error,
    run_batch,
};

use crate::error::write_cli_error;
use crate::render::render_json_document;
use crate::{EXIT_CODE_FATAL, EXIT_CODE_RUN_FAILED};

use super::discovery::DiscoveredTarget;

pub(super) fn run_report_requires_failed_exit(report: &ffhn_core::RunReport) -> bool {
    if matches!(
        report.run_outcome(),
        RunOutcome::FailedTransient | RunOutcome::FailedPermanent
    ) {
        return true;
    }
    if report.run_mode() != RunMode::Live {
        return false;
    }
    run_report_has_notification_failures(report)
}

fn batch_report_has_notification_failures(report: &ffhn_core::BatchRunReport) -> bool {
    report.outcome_counts().notification_failure() > 0
}

fn run_report_has_notification_failures(report: &ffhn_core::RunReport) -> bool {
    report
        .notifications()
        .any(|delivery| delivery.status() != NotificationDeliveryStatus::Delivered)
}

pub(super) fn requested_run_mode(dry_run: bool) -> RunMode {
    if dry_run {
        RunMode::DryRun
    } else {
        RunMode::Live
    }
}

pub(super) fn render_batch_result(
    batch_result: Result<BatchRunReport, CoreError>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    match batch_result {
        Ok(report) => {
            let outcome_counts = report.outcome_counts();
            let has_structured_failures = outcome_counts.failed_transient()
                + outcome_counts.failed_permanent()
                + outcome_counts.persist_error()
                + outcome_counts.fatal_error()
                > 0;
            match render_json_document(stdout, &report) {
                Err(_) => {
                    let _ = write_cli_error(stderr, &batch_run_report_write_error());
                    EXIT_CODE_FATAL
                }
                Ok(()) => {
                    let failure_signal_count = usize::from(has_structured_failures)
                        + usize::from(batch_report_has_notification_failures(&report));
                    if failure_signal_count > 0 {
                        EXIT_CODE_RUN_FAILED
                    } else {
                        0
                    }
                }
            }
        }
        Err(error) => {
            let _ = write_cli_error(stderr, &error.to_string());
            EXIT_CODE_FATAL
        }
    }
}

pub(super) fn run_discovered_batch(
    watch_root: &Path,
    discovered_targets: Vec<DiscoveredTarget>,
    run_mode: RunMode,
    jobs: usize,
) -> Result<BatchRunReport, CoreError> {
    let valid_targets = discovered_targets
        .iter()
        .filter_map(|target| target.validated_id.clone())
        .collect::<Vec<_>>();
    let requested_targets = discovered_targets
        .iter()
        .map(|target| target.requested_id.clone())
        .collect::<Vec<_>>();
    let base_report = run_batch(watch_root, &valid_targets, run_mode, jobs)?;
    let entries = merge_discovered_entries(discovered_targets, base_report.entries().to_vec());

    BatchRunReport::new(BatchRunReportInput::new(
        run_mode,
        base_report.watch_root().to_owned(),
        requested_targets,
        base_report.run_started_at().to_owned(),
        base_report.run_finished_at().to_owned(),
        base_report.max_concurrency(),
        entries,
    )?)
}

pub(super) fn merge_discovered_entries(
    discovered_targets: Vec<DiscoveredTarget>,
    valid_entries: Vec<BatchRunEntry>,
) -> Vec<BatchRunEntry> {
    let mut valid_entries = valid_entries.into_iter();

    discovered_targets
        .into_iter()
        .map(|target| match target.validated_id {
            Some(_) => valid_entries
                .next()
                .expect("valid batch entries should align with discovered targets"),
            None => BatchRunEntry::fatal(
                target.requested_id,
                ProcessErrorDetail::new(
                    ProcessErrorKind::Contract,
                    target.validation_message.unwrap_or_else(|| {
                        "target_id violates FFHN's durable target-id contract".to_owned()
                    }),
                    None,
                )
                .expect("discovered fatal detail must validate"),
            )
            .expect("discovered fatal entries must accept non-empty target labels"),
        })
        .collect()
}

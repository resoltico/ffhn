use std::path::Path;
use std::thread;

use crate::model::validate_batch_request_contract;
use crate::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, BatchOutcomeCounts,
    BatchRunEntry, BatchRunReport, CoreError, RunOutcome, TargetPaths,
};

use super::super::storage::now_utc;
use super::execute::{RunOptions, run_once_with_options};

pub(crate) fn run_batch(
    watch_root: &Path,
    targets: &[String],
    options: RunOptions,
    jobs: usize,
) -> Result<BatchRunReport, CoreError> {
    validate_batch_request_contract(targets, jobs)?;
    let run_started_at = now_utc()?;
    let max_concurrency = jobs;
    let mut entries = Vec::with_capacity(targets.len());

    for chunk in targets.chunks(max_concurrency) {
        let mut handles = Vec::with_capacity(chunk.len());
        for target_id in chunk {
            let watch_root = watch_root.to_path_buf();
            let target_id = target_id.clone();
            handles.push(thread::spawn(move || {
                let paths = TargetPaths::new(watch_root, target_id.clone());
                let entry = match run_once_with_options(&paths, options) {
                    Ok(run_report) => BatchRunEntry {
                        target_id,
                        run_report: Some(run_report),
                        fatal_error: None,
                    },
                    Err(error) => BatchRunEntry {
                        target_id,
                        run_report: None,
                        fatal_error: Some(error.to_string()),
                    },
                };
                (entry.target_id.clone(), entry)
            }));
        }

        let mut completed = handles
            .into_iter()
            .map(join_batch_handle)
            .collect::<Result<Vec<_>, _>>()?;
        completed.sort_by(|left, right| left.0.cmp(&right.0));
        entries.extend(completed.into_iter().map(|(_, entry)| entry));
    }

    entries.sort_by_key(|entry| {
        targets
            .iter()
            .position(|target_id| target_id == &entry.target_id)
            .unwrap_or(usize::MAX)
    });

    let mut outcome_counts = BatchOutcomeCounts {
        initialized: 0,
        changed: 0,
        unchanged: 0,
        failed_transient: 0,
        failed_permanent: 0,
        skipped_disabled: 0,
        fatal_error: 0,
    };
    for entry in &entries {
        match entry.run_report.as_ref().map(|report| report.run_outcome) {
            Some(RunOutcome::Initialized) => outcome_counts.initialized += 1,
            Some(RunOutcome::Changed) => outcome_counts.changed += 1,
            Some(RunOutcome::Unchanged) => outcome_counts.unchanged += 1,
            Some(RunOutcome::FailedTransient) => outcome_counts.failed_transient += 1,
            Some(RunOutcome::FailedPermanent) => outcome_counts.failed_permanent += 1,
            Some(RunOutcome::SkippedDisabled) => outcome_counts.skipped_disabled += 1,
            None => outcome_counts.fatal_error += 1,
        }
    }

    let report = BatchRunReport {
        schema_name: BATCH_RUN_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: BATCH_RUN_REPORT_SCHEMA_VERSION,
        run_mode: options.mode,
        watch_root: watch_root.to_string_lossy().into_owned(),
        requested_targets: targets.to_vec(),
        run_started_at,
        run_finished_at: now_utc()?,
        max_concurrency,
        entries,
        outcome_counts,
        extensions: None,
    };
    report.validate()?;
    Ok(report)
}

pub(super) fn join_batch_handle(
    handle: thread::JoinHandle<(String, BatchRunEntry)>,
) -> Result<(String, BatchRunEntry), CoreError> {
    handle
        .join()
        .map_err(|_| CoreError::htmlcut("batch worker panicked before emitting a target result"))
}

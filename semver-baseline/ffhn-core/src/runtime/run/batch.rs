use std::any::Any;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;

use crate::model::validate_batch_request_contract;
use crate::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, BatchOutcomeCounts,
    BatchRunEntry, BatchRunReport, CoreError, ProcessErrorDetail, RunOutcome, TargetPaths,
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
    let entries = collect_batch_entries(watch_root, targets, options, jobs)?;

    let mut outcome_counts = BatchOutcomeCounts {
        initialized: 0,
        changed: 0,
        unchanged: 0,
        failed_transient: 0,
        failed_permanent: 0,
        skipped_disabled: 0,
        persist_error: 0,
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
        if entry.run_report.as_ref().is_some_and(|report| {
            report.reason_code == crate::ReasonCode::PersistError || report.persist.error.is_some()
        }) {
            outcome_counts.persist_error += 1;
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

fn collect_batch_entries(
    watch_root: &Path,
    targets: &[String],
    options: RunOptions,
    jobs: usize,
) -> Result<Vec<BatchRunEntry>, CoreError> {
    let worker_count = jobs.min(targets.len());
    if worker_count == 0 {
        return Ok(Vec::new());
    }

    let requested_targets = Arc::new(targets.to_vec());
    let next_index = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) = mpsc::channel();
    let mut handles = Vec::with_capacity(worker_count);

    for _ in 0..worker_count {
        handles.push(spawn_batch_worker(
            watch_root.to_path_buf(),
            Arc::clone(&requested_targets),
            Arc::clone(&next_index),
            sender.clone(),
            options,
        ));
    }
    drop(sender);

    let mut entries = vec![None; targets.len()];
    while let Ok((index, entry)) = receiver.recv() {
        record_received_entry(&mut entries, index, entry);
    }

    for handle in handles {
        join_batch_handle(handle)?;
    }

    finalize_received_entries(entries, requested_targets.as_slice())
}

fn record_received_entry(
    entries: &mut [Option<BatchRunEntry>],
    index: usize,
    entry: BatchRunEntry,
) {
    if index < entries.len() {
        entries[index] = Some(entry);
    }
}

fn finalize_received_entries(
    entries: Vec<Option<BatchRunEntry>>,
    requested_targets: &[String],
) -> Result<Vec<BatchRunEntry>, CoreError> {
    entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            entry.ok_or_else(|| {
                CoreError::htmlcut(format!(
                    "batch worker channel closed before target result {} was emitted",
                    requested_targets[index]
                ))
            })
        })
        .collect()
}

fn spawn_batch_worker(
    watch_root: PathBuf,
    targets: Arc<Vec<String>>,
    next_index: Arc<AtomicUsize>,
    sender: mpsc::Sender<(usize, BatchRunEntry)>,
    options: RunOptions,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        loop {
            let index = next_index.fetch_add(1, Ordering::Relaxed);
            if index >= targets.len() {
                return;
            }

            let target_id = targets[index].clone();
            let entry = batch_entry_for_target(&watch_root, target_id, options);
            if !send_batch_entry(&sender, index, entry) {
                return;
            }
        }
    })
}

fn send_batch_entry(
    sender: &mpsc::Sender<(usize, BatchRunEntry)>,
    index: usize,
    entry: BatchRunEntry,
) -> bool {
    sender.send((index, entry)).is_ok()
}

fn batch_entry_for_target(
    watch_root: &Path,
    target_id: String,
    options: RunOptions,
) -> BatchRunEntry {
    let paths = TargetPaths::new(watch_root, target_id.clone());
    match run_once_with_options(&paths, options) {
        Ok(run_report) => BatchRunEntry {
            target_id,
            run_report: Some(run_report),
            fatal_error: None,
        },
        Err(error) => BatchRunEntry {
            target_id,
            run_report: None,
            fatal_error: Some(ProcessErrorDetail::from(&error)),
        },
    }
}

pub(super) fn join_batch_handle(handle: thread::JoinHandle<()>) -> Result<(), CoreError> {
    handle.join().map_err(batch_worker_panic_error)
}

fn batch_worker_panic_error(payload: Box<dyn Any + Send>) -> CoreError {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return CoreError::htmlcut(format!("batch worker panicked: {message}"));
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return CoreError::htmlcut(format!("batch worker panicked: {message}"));
    }
    CoreError::htmlcut("batch worker panicked with a non-string payload")
}

#[cfg(test)]
#[path = "tests/batch_internal.rs"]
mod tests;

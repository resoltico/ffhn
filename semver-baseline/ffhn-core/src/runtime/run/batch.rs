use std::any::Any;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;

use crate::{
    BatchRunEntry, BatchRunReport, BatchRunReportInput, CoreError, ProcessErrorDetail, TargetId,
    TargetPaths,
};

use super::super::storage::now_utc;
use super::execute::{RunOptions, run_once_with_options};

pub(crate) fn run_batch(
    watch_root: &Path,
    targets: &[TargetId],
    options: RunOptions,
    jobs: usize,
) -> Result<BatchRunReport, CoreError> {
    let run_started_at = now_utc()?;
    let max_concurrency = jobs;
    let entries = collect_batch_entries(watch_root, targets, options, jobs)?;
    BatchRunReport::new(BatchRunReportInput::new(
        options.mode,
        watch_root.to_string_lossy().into_owned(),
        targets.iter().map(ToString::to_string).collect(),
        run_started_at,
        now_utc()?,
        max_concurrency,
        entries,
    )?)
}

fn collect_batch_entries(
    watch_root: &Path,
    targets: &[TargetId],
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
    requested_targets: &[TargetId],
) -> Result<Vec<BatchRunEntry>, CoreError> {
    entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            entry.ok_or_else(|| {
                CoreError::internal(format!(
                    "batch worker channel closed before target result {} was emitted",
                    requested_targets[index]
                ))
            })
        })
        .collect()
}

fn spawn_batch_worker(
    watch_root: PathBuf,
    targets: Arc<Vec<TargetId>>,
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
    target_id: TargetId,
    options: RunOptions,
) -> BatchRunEntry {
    let paths = TargetPaths::new(watch_root, target_id.clone());
    match run_once_with_options(&paths, options) {
        Ok(run_report) => BatchRunEntry::from_run_report(run_report),
        Err(error) => BatchRunEntry::fatal(target_id, ProcessErrorDetail::from(&error))
            .expect("fatal batch entries should accept validated explicit target ids"),
    }
}

pub(super) fn join_batch_handle(handle: thread::JoinHandle<()>) -> Result<(), CoreError> {
    handle.join().map_err(batch_worker_panic_error)
}

fn batch_worker_panic_error(payload: Box<dyn Any + Send>) -> CoreError {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return CoreError::internal(format!("batch worker panicked: {message}"));
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return CoreError::internal(format!("batch worker panicked: {message}"));
    }
    CoreError::internal("batch worker panicked with a non-string payload")
}

#[cfg(test)]
#[path = "tests/batch_internal.rs"]
mod tests;

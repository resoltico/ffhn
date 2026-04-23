use super::*;
use std::panic;
use tempfile::tempdir;

fn fatal_entry(target_id: &str) -> BatchRunEntry {
    BatchRunEntry {
        target_id: target_id.to_owned(),
        run_report: None,
        fatal_error: Some(ProcessErrorDetail {
            kind: crate::ProcessErrorKind::Io,
            message: "permission denied".to_owned(),
            path: Some(format!("/tmp/watch/{target_id}/lock")),
        }),
    }
}

#[test]
fn batch_helpers_cover_out_of_range_missing_entry_and_closed_receiver_paths() {
    let demo = fatal_entry("demo");

    let mut entries = vec![None];
    record_received_entry(&mut entries, 1, demo.clone());
    assert!(entries[0].is_none());

    record_received_entry(&mut entries, 0, demo.clone());
    assert_eq!(entries[0], Some(demo.clone()));

    let finalized =
        finalize_received_entries(vec![Some(demo.clone())], &["demo".to_owned()]).expect("ok");
    assert_eq!(finalized, vec![demo.clone()]);

    let missing = finalize_received_entries(vec![None], &["demo".to_owned()]).expect_err("missing");
    assert!(
        missing
            .to_string()
            .contains("batch worker channel closed before target result demo was emitted")
    );

    let (sender, receiver) = mpsc::channel();
    drop(receiver);
    assert!(!send_batch_entry(&sender, 0, demo));
}

#[test]
fn batch_worker_panic_error_covers_string_and_non_string_payloads() {
    let string_panic = join_batch_handle(thread::spawn(|| {
        panic::panic_any(String::from("boom string"))
    }))
    .expect_err("string panic");
    assert!(
        string_panic
            .to_string()
            .contains("batch worker panicked: boom string")
    );

    let non_string =
        join_batch_handle(thread::spawn(|| panic::panic_any(42usize))).expect_err("non-string");
    assert!(
        non_string
            .to_string()
            .contains("batch worker panicked with a non-string payload")
    );
}

#[test]
fn spawn_batch_worker_returns_early_when_the_receiver_is_gone() {
    let temp = tempdir().expect("tempdir");
    let targets = Arc::new(vec!["demo".to_owned()]);
    let next_index = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) = mpsc::channel();
    drop(receiver);

    let handle = spawn_batch_worker(
        temp.path().to_path_buf(),
        targets,
        next_index.clone(),
        sender,
        RunOptions::LIVE,
    );

    join_batch_handle(handle).expect("worker join");
    assert_eq!(next_index.load(Ordering::Relaxed), 1);
}

#[test]
fn collect_batch_entries_returns_empty_when_no_targets_are_requested() {
    let temp = tempdir().expect("tempdir");
    let entries =
        collect_batch_entries(temp.path(), &[], RunOptions::LIVE, 4).expect("empty entries");
    assert!(entries.is_empty());
}

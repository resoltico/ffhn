//! Bounded continuously fed execution for independent source-turn tasks.

use std::{
    collections::VecDeque,
    sync::{Mutex, mpsc},
};

use crate::CoreError;

pub(super) fn run_bounded<T, R, F>(
    pending: Vec<T>,
    jobs: usize,
    operation: F,
) -> Result<Vec<R>, CoreError>
where
    T: Send,
    R: Send,
    F: Fn(T) -> Result<R, CoreError> + Sync,
{
    if jobs == 0 {
        return Err(CoreError::contract("bounded runner jobs must be positive"));
    }
    let task_count = pending.len();
    let queue = Mutex::new(pending.into_iter().enumerate().collect::<VecDeque<_>>());
    let (results_sender, results_receiver) = mpsc::channel();
    let worker_count = jobs.min(task_count);
    let panicked = std::thread::scope(|scope| {
        let handles = (0..worker_count)
            .map(|_| {
                let results_sender = results_sender.clone();
                let queue = &queue;
                let operation = &operation;
                scope.spawn(move || {
                    loop {
                        let task = queue.lock().expect("task queue lock poisoned").pop_front();
                        let Some((index, task)) = task else {
                            break;
                        };
                        let result = operation(task);
                        let _ = results_sender.send((index, result));
                    }
                })
            })
            .collect::<Vec<_>>();
        let mut panicked = false;
        for handle in handles {
            panicked |= handle.join().is_err();
        }
        panicked
    });
    if panicked {
        return Err(CoreError::internal("agent source worker panicked"));
    }
    drop(results_sender);
    let mut results = results_receiver.into_iter().collect::<Vec<_>>();
    results.sort_unstable_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, result)| result).collect()
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    use std::time::Duration;

    use super::*;

    #[test]
    fn continuously_feeds_bounded_workers_and_preserves_input_order() {
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let slow_finished = Arc::new(AtomicBool::new(false));
        let third_started_before_slow_finished = Arc::new(AtomicBool::new(false));
        let completed = run_bounded(vec![0, 1, 2, 3], 2, {
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            let slow_finished = Arc::clone(&slow_finished);
            let third_started_before_slow_finished =
                Arc::clone(&third_started_before_slow_finished);
            move |id| {
                let concurrent = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(concurrent, Ordering::SeqCst);
                match id {
                    0 => {
                        std::thread::sleep(Duration::from_millis(80));
                        slow_finished.store(true, Ordering::SeqCst);
                    }
                    2 => {
                        third_started_before_slow_finished
                            .store(!slow_finished.load(Ordering::SeqCst), Ordering::SeqCst);
                    }
                    _ => std::thread::sleep(Duration::from_millis(10)),
                }
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(id)
            }
        })
        .expect("bounded work");
        assert_eq!(completed, [0, 1, 2, 3]);
        assert_eq!(peak.load(Ordering::SeqCst), 2);
        assert!(third_started_before_slow_finished.load(Ordering::SeqCst));

        let serial_peak = AtomicUsize::new(0);
        let serial_active = AtomicUsize::new(0);
        run_bounded(vec![0, 1], 1, |_| {
            let concurrent = serial_active.fetch_add(1, Ordering::SeqCst) + 1;
            serial_peak.fetch_max(concurrent, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(10));
            serial_active.fetch_sub(1, Ordering::SeqCst);
            Ok(())
        })
        .expect("serial work");
        assert_eq!(serial_peak.load(Ordering::SeqCst), 1);
        assert!(run_bounded::<u8, (), _>(Vec::new(), 0, |_| Ok(())).is_err());
        assert!(
            run_bounded::<u8, u8, _>(Vec::new(), 2, Ok)
                .expect("empty")
                .is_empty()
        );
        let error = run_bounded(vec![0_u8, 1], 2, |id| {
            if id == 1 {
                Err(CoreError::contract("failed"))
            } else {
                Ok(id)
            }
        });
        assert!(error.is_err());
        assert!(
            run_bounded(vec![0_u8], 1, |_| -> Result<(), CoreError> {
                panic!("worker panic")
            })
            .is_err()
        );
    }
}

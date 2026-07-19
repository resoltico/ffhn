use super::super::execution::status_with_shared_lock_for_test;
use super::super::lock::lock_shared_with_retry_observer_for_test;
use super::support::*;

#[test]
fn shared_status_lock_waits_for_a_live_run_lock_to_be_released() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":1}"#).expect("source");
    run_once(&paths).expect("seed durable state");
    let (ready_sender, ready_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let lock_paths = paths.clone();
    let worker = thread::spawn(move || {
        let lock = lock_exclusive(&lock_paths).expect("exclusive lock");
        ready_sender.send(()).expect("ready");
        release_receiver.recv().expect("release exclusive lock");
        drop(lock);
    });
    ready_receiver.recv().expect("lock acquired");

    let (retry_sender, retry_receiver) = mpsc::channel();
    let (status_sender, status_receiver) = mpsc::channel();
    let status_paths = paths.clone();
    let status_worker = thread::spawn(move || {
        let result = status_with_shared_lock_for_test(&status_paths, |paths| {
            lock_shared_with_retry_observer_for_test(paths, || {
                retry_sender
                    .send(())
                    .expect("status retried a blocked shared-lock acquisition");
            })
        });
        status_sender.send(result).expect("status result");
    });
    retry_receiver
        .recv()
        .expect("exclusive run lock forced status to retry shared acquisition");
    assert!(matches!(
        status_receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));

    release_sender.send(()).expect("release status reader");
    let status = status_receiver
        .recv()
        .expect("status completed after lock release")
        .expect("waited status");
    assert_eq!(status.kind(), StatusKind::Ready);
    assert!(status.lifecycle().is_some());
    status_worker.join().expect("status worker");
    worker.join().expect("worker");
}

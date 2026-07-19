use super::support::*;

#[test]
fn non_lockable_lock_nodes_surface_io_errors() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.lock_root()).expect("lock root");
    let status = std::process::Command::new("mkfifo")
        .arg(paths.run_lock_file())
        .status()
        .expect("invoke mkfifo");
    assert!(status.success());

    assert!(matches!(lock_exclusive(&paths), Err(LockError::Io(_))));
    assert!(lock_shared(&paths).is_err());
}

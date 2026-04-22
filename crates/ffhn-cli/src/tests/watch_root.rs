use super::*;

#[test]
fn discover_watch_root_targets_covers_missing_disabled_invalid_and_non_utf_dirs() {
    let temp = tempdir().expect("tempdir");
    let missing = temp.path().join("missing");
    assert!(
        discover_watch_root_targets(&missing)
            .expect("missing watch root")
            .is_empty()
    );

    let watch_root = temp.path().join("watchlist");
    write_named_http_target(&watch_root, "demo", "demo", "https://example.com", true);
    write_named_http_target(
        &watch_root,
        "disabled",
        "disabled",
        "https://example.com",
        false,
    );
    write_named_http_target(&watch_root, "invalid", "other", "https://example.com", true);

    assert_eq!(
        discover_watch_root_targets(&watch_root).expect("discover targets"),
        vec!["demo".to_owned(), "invalid".to_owned()]
    );
}

#[test]
fn collect_watch_root_directories_surfaces_iteration_and_metadata_errors() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    fs::create_dir_all(&watch_root).expect("create watch root");
    let missing_path = watch_root.join("missing");

    let entry_error =
        collect_watch_root_directories(&watch_root, vec![Err(io::Error::other("boom"))])
            .expect_err("entry iteration error");
    assert!(matches!(
        entry_error,
        ffhn_core::CoreError::Io { path, .. } if path == watch_root
    ));

    let metadata_error =
        collect_watch_root_directories(&watch_root, vec![Ok(missing_path.clone())])
            .expect_err("metadata error");
    assert!(matches!(
        metadata_error,
        ffhn_core::CoreError::Io { path, .. } if path == missing_path
    ));
}

#[test]
fn collect_watch_root_directories_keeps_only_directory_entries() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let directory_path = watch_root.join("demo");
    let file_path = watch_root.join("note.txt");
    fs::create_dir_all(&directory_path).expect("create target directory");
    fs::write(&file_path, "ignore").expect("write non-directory entry");

    let directories = collect_watch_root_directories(
        &watch_root,
        vec![Ok(directory_path.clone()), Ok(file_path)],
    )
    .expect("directory entries");

    assert_eq!(directories, vec![directory_path]);
}

#[cfg(unix)]
#[test]
fn run_command_returns_fatal_when_target_discovery_fails() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    let watch_root_string = watch_root.to_string_lossy().into_owned();
    fs::create_dir_all(&watch_root).expect("create watch root");

    let original = fs::metadata(&watch_root)
        .expect("watch root metadata")
        .permissions();
    let mut denied = original.clone();
    denied.set_mode(0o000);
    fs::set_permissions(&watch_root, denied).expect("deny watch root access");

    let (exit_code, stdout, stderr) = run_vec(vec![
        "ffhn".to_owned(),
        "run".to_owned(),
        "--watch-root".to_owned(),
        watch_root_string,
        "--all".to_owned(),
    ]);

    fs::set_permissions(&watch_root, original).expect("restore watch root access");

    assert_eq!(exit_code, EXIT_CODE_FATAL);
    assert!(stdout.is_empty());
    assert!(stderr.contains("filesystem error"));
}

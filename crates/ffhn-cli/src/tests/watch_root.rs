use super::*;
use crate::execute::DiscoveredTarget;
use ffhn_core::TargetId;

fn discovered_target(requested_id: &str) -> DiscoveredTarget {
    DiscoveredTarget {
        requested_id: requested_id.to_owned(),
        validated_id: Some(TargetId::new(requested_id).expect("target id")),
        validation_message: None,
    }
}

#[test]
fn discover_watch_root_targets_requires_a_real_watch_root_and_ignores_non_targets() {
    let temp = tempdir().expect("tempdir");
    let missing = temp.path().join("missing");
    let missing_error = discover_watch_root_targets(&missing).expect_err("missing watch root");
    assert!(matches!(missing_error, ffhn_core::CoreError::Io { path, .. } if path == missing));

    let not_directory = temp.path().join("not-a-directory");
    fs::write(&not_directory, "file").expect("write non-directory watch root");
    let not_directory_error =
        discover_watch_root_targets(&not_directory).expect_err("non-directory watch root");
    assert!(matches!(
        not_directory_error,
        ffhn_core::CoreError::Io { path, .. } if path == not_directory
    ));

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
    fs::create_dir_all(watch_root.join("notes")).expect("create unrelated directory");

    assert_eq!(
        discover_watch_root_targets(&watch_root).expect("discover targets"),
        vec![
            discovered_target("demo"),
            discovered_target("disabled"),
            discovered_target("invalid"),
        ]
    );
}

#[test]
fn discover_watch_root_targets_preserves_invalid_directory_labels_as_contract_failures() {
    let temp = tempdir().expect("tempdir");
    let watch_root = temp.path().join("watchlist");
    write_named_http_target(&watch_root, "Demo", "demo", "https://example.com", true);

    let discovered = discover_watch_root_targets(&watch_root).expect("discover targets");
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].requested_id, "Demo");
    assert!(discovered[0].validated_id.is_none());
    assert!(
        discovered[0]
            .validation_message
            .as_deref()
            .expect("validation message")
            .contains("target_id must")
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

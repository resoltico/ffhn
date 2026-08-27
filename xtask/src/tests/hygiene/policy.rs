use super::*;

#[cfg(unix)]
use std::os::unix::net::UnixListener;

#[test]
fn report_violations_helpers_cover_missing_markers_budget_and_tmp_cargo_cases() {
    let repo_root = tempdir().expect("repo tempdir");
    let managed_root = repo_root.path().join("managed");
    fs::create_dir_all(&managed_root).expect("create managed root");

    let violations = report_violations_for_tests(&[
        HygieneEntry {
            id: "managed-workspace-target".to_owned(),
            kind: "workspace-target".to_owned(),
            path: managed_root.display().to_string(),
            present: true,
            bytes: 0,
            budget_bytes: None,
            managed: true,
            safe_to_delete: true,
            details: Vec::new(),
        },
        HygieneEntry {
            id: "managed-workspace-build".to_owned(),
            kind: "workspace-build".to_owned(),
            path: repo_root.path().join("budget").display().to_string(),
            present: true,
            bytes: 2,
            budget_bytes: Some(1),
            managed: false,
            safe_to_delete: true,
            details: Vec::new(),
        },
        HygieneEntry {
            id: "at-budget".to_owned(),
            kind: "at-budget".to_owned(),
            path: repo_root.path().join("at-budget").display().to_string(),
            present: true,
            bytes: 1,
            budget_bytes: Some(1),
            managed: false,
            safe_to_delete: true,
            details: Vec::new(),
        },
        HygieneEntry {
            id: "repo-tmp-cargo-targets".to_owned(),
            kind: "repo-tmp-cargo-targets".to_owned(),
            path: repo_root.path().join("tmp").display().to_string(),
            present: true,
            bytes: 0,
            budget_bytes: None,
            managed: false,
            safe_to_delete: true,
            details: vec![
                "tmp/cargo-target-a".to_owned(),
                "tmp/cargo-target-b".to_owned(),
            ],
        },
    ]);

    assert!(violations.iter().any(|violation| {
        violation.id == "managed-workspace-target"
            && violation
                .message
                .contains("missing managed-artifact markers")
    }));
    assert!(violations.iter().any(|violation| {
        violation.id == "managed-workspace-build"
            && violation.message.contains("exceeds its 1 B budget")
    }));
    assert!(violations.iter().any(|violation| {
        violation.id == "repo-tmp-cargo-targets"
            && violation
                .message
                .contains("repository tmp contains 2 cargo target roots")
    }));
    assert_eq!(violations.len(), 3);
    assert!(
        !violations
            .iter()
            .any(|violation| violation.id == "at-budget")
    );
}

#[cfg(unix)]
#[test]
fn repo_tmp_cargo_roots_reports_unreadable_tmp_root() {
    let repo_root = tempdir().expect("repo tempdir");
    let tmp_root = repo_root.path().join("tmp");
    fs::create_dir_all(&tmp_root).expect("create tmp root");

    let mut permissions = fs::metadata(&tmp_root).expect("metadata").permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&tmp_root, permissions.clone()).expect("chmod tmp root");

    let error = repo_tmp_cargo_roots_for_tests(repo_root.path())
        .expect_err("unreadable tmp root should fail");

    permissions.set_mode(0o700);
    fs::set_permissions(&tmp_root, permissions).expect("restore tmp root permissions");

    assert!(
        error
            .to_string()
            .contains("failed to inspect repository temporary root")
    );
}

#[cfg(unix)]
#[test]
fn dir_size_bytes_handles_symlinks_special_files_and_permission_failures() {
    let repo_root = tempdir().expect("repo tempdir");
    let file_path = repo_root.path().join("payload.bin");
    fs::write(&file_path, vec![0u8; 8]).expect("write payload");

    let symlink_path = repo_root.path().join("payload.link");
    std::os::unix::fs::symlink(&file_path, &symlink_path).expect("create symlink");
    assert_eq!(dir_size_bytes_for_tests(&symlink_path), 0);

    let socket_path = repo_root.path().join("payload.sock");
    let _listener = UnixListener::bind(&socket_path).expect("create unix socket");
    assert_eq!(dir_size_bytes_for_tests(&socket_path), 0);

    let blocked_parent = repo_root.path().join("blocked-parent");
    fs::create_dir_all(&blocked_parent).expect("create blocked parent");
    fs::write(blocked_parent.join("child.bin"), vec![0u8; 4]).expect("write blocked child");

    let mut parent_permissions = fs::metadata(&blocked_parent)
        .expect("metadata")
        .permissions();
    parent_permissions.set_mode(0o000);
    fs::set_permissions(&blocked_parent, parent_permissions.clone()).expect("chmod blocked parent");

    let metadata_error = dir_size_bytes_result_for_tests(&blocked_parent.join("child.bin"))
        .expect_err("unreadable child metadata should fail");
    let directory_error =
        dir_size_bytes_result_for_tests(&blocked_parent).expect_err("unreadable dir should fail");

    parent_permissions.set_mode(0o700);
    fs::set_permissions(&blocked_parent, parent_permissions)
        .expect("restore blocked parent permissions");

    assert!(
        metadata_error
            .to_string()
            .contains("failed to read hygiene metadata")
    );
    assert!(
        directory_error
            .to_string()
            .contains("failed to read hygiene directory")
    );
}

#[test]
fn missing_managed_markers_reports_both_markers() {
    let repo_root = tempdir().expect("repo tempdir");
    let managed_root = repo_root.path().join("managed");
    fs::create_dir_all(&managed_root).expect("create managed root");

    assert_eq!(
        missing_managed_markers_for_tests(&managed_root),
        vec!["CACHEDIR.TAG".to_owned(), ".ffhn-artifact.toml".to_owned()]
    );
}

#[test]
fn coverage_entries_require_nested_llvm_cov_markers_only_for_coverage_kinds() {
    let repo_root = tempdir().expect("repo tempdir");

    for id in ["managed-coverage-target", "managed-coverage-build"] {
        let root = repo_root.path().join(id);
        fs::create_dir_all(&root).expect("create coverage root");
        fs::write(root.join("CACHEDIR.TAG"), "marker").expect("write cache marker");
        fs::write(root.join(".ffhn-artifact.toml"), "marker").expect("write artifact marker");
        let entry = HygieneEntry {
            id: id.to_owned(),
            kind: "coverage".to_owned(),
            path: root.display().to_string(),
            present: true,
            bytes: 0,
            budget_bytes: None,
            managed: true,
            safe_to_delete: true,
            details: Vec::new(),
        };
        assert_eq!(
            missing_managed_markers_for_entry_for_tests(&entry),
            vec![
                "llvm-cov-target/CACHEDIR.TAG".to_owned(),
                "llvm-cov-target/.ffhn-artifact.toml".to_owned(),
            ]
        );
    }

    let ordinary_root = repo_root.path().join("managed-workspace-target");
    fs::create_dir_all(&ordinary_root).expect("create ordinary managed root");
    fs::write(ordinary_root.join("CACHEDIR.TAG"), "marker").expect("write cache marker");
    fs::write(ordinary_root.join(".ffhn-artifact.toml"), "marker").expect("write artifact marker");
    let ordinary_entry = HygieneEntry {
        id: "managed-workspace-target".to_owned(),
        kind: "workspace-target".to_owned(),
        path: ordinary_root.display().to_string(),
        present: true,
        bytes: 0,
        budget_bytes: None,
        managed: true,
        safe_to_delete: true,
        details: Vec::new(),
    };
    assert!(missing_managed_markers_for_entry_for_tests(&ordinary_entry).is_empty());
}

#[test]
fn helper_functions_classify_sizes_and_cargo_target_shapes() {
    let repo_root = tempdir().expect("repo tempdir");
    let sized_dir = repo_root.path().join("sized");
    let ordinary_dir = repo_root.path().join("ordinary");
    let aggregate_root = repo_root.path().join("aggregate");
    let aggregate_child_a = aggregate_root.join("a");
    let aggregate_child_b = aggregate_root.join("b");
    fs::create_dir_all(&sized_dir).expect("create sized dir");
    fs::create_dir_all(&ordinary_dir).expect("create ordinary dir");
    fs::create_dir_all(&aggregate_child_a).expect("create aggregate child a");
    fs::create_dir_all(&aggregate_child_b).expect("create aggregate child b");
    fs::write(sized_dir.join("payload.bin"), vec![0u8; 2048]).expect("write payload");
    fs::write(aggregate_child_a.join("one.bin"), vec![0u8; 512]).expect("write child a");
    fs::write(aggregate_child_b.join("two.bin"), vec![0u8; 256]).expect("write child b");
    fs::write(sized_dir.join(".rustc_info.json"), "{}").expect("write rustc info");
    fs::write(ordinary_dir.join("payload.bin"), b"not a cargo target")
        .expect("write ordinary payload");

    assert_eq!(dir_size_bytes_for_tests(&sized_dir), 2050);
    assert_eq!(format_bytes_for_tests(2048), "2.0 KiB");
    assert_eq!(format_bytes_for_tests(1_048_576), "1.0 MiB");
    assert_eq!(format_bytes_for_tests(1_073_741_824), "1.0 GiB");
    assert!(looks_like_cargo_target_dir_for_tests(&sized_dir));
    assert!(!looks_like_cargo_target_dir_for_tests(&ordinary_dir));
    assert_eq!(
        aggregate_entry_for_tests(
            &aggregate_root,
            &[aggregate_child_a.clone(), aggregate_child_b.clone()]
        )
        .expect("aggregate entry")
        .bytes,
        768
    );
    assert!(
        dir_size_bytes_result_for_tests(&repo_root.path().join("missing"))
            .expect("missing size should be zero")
            == 0
    );
}

#[test]
fn directory_size_exclusion_skips_only_the_named_subtree() {
    let repo_root = tempdir().expect("repo tempdir");
    let retained = repo_root.path().join("retained");
    let skipped = repo_root.path().join("skipped");
    fs::create_dir_all(&retained).expect("create retained subtree");
    fs::create_dir_all(&skipped).expect("create skipped subtree");
    fs::write(retained.join("payload.bin"), vec![0u8; 13]).expect("write retained payload");
    fs::write(skipped.join("payload.bin"), vec![0u8; 29]).expect("write skipped payload");

    assert_eq!(
        dir_size_bytes_excluding_roots_for_tests(repo_root.path(), std::slice::from_ref(&skipped),)
            .expect("size with skipped root"),
        13
    );
}

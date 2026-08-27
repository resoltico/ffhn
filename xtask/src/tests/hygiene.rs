use super::*;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn prepare_artifact_layout_creates_managed_roots_and_marker_files() {
    let repo_root = tempdir().expect("repo tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        let (workspace_target, workspace_build) =
            prepare_artifact_layout(repo_root.path(), CommandArtifactLayout::ManagedWorkspace)
                .expect("prepare workspace layout")
                .expect("workspace dirs");
        let (coverage_target, coverage_build) =
            prepare_artifact_layout(repo_root.path(), CommandArtifactLayout::ManagedCoverage)
                .expect("prepare coverage layout")
                .expect("coverage dirs");
        let mutation_reports =
            prepare_mutation_report_root(repo_root.path()).expect("prepare mutation reports");

        for path in [
            workspace_target,
            workspace_build,
            coverage_target,
            coverage_build,
            coverage_cargo_target_dir(repo_root.path()),
            coverage_cargo_build_dir(repo_root.path()),
            mutation_reports,
        ] {
            assert!(path.is_dir(), "{} should exist", path.display());
            assert!(path.join("CACHEDIR.TAG").is_file());
            assert!(path.join(".ffhn-artifact.toml").is_file());
        }
    });
}

#[test]
fn prepare_artifact_layout_inherit_leaves_artifact_env_unmanaged() {
    let repo_root = tempdir().expect("repo tempdir");

    assert_eq!(
        prepare_artifact_layout(repo_root.path(), CommandArtifactLayout::Inherit)
            .expect("inherit layout"),
        None
    );
}

#[test]
fn hygiene_report_publishes_exact_byte_budgets() {
    let repo_root = tempdir().expect("repo tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        let report = hygiene_report(repo_root.path()).expect("hygiene report");
        let budgets = report
            .entries
            .iter()
            .map(|entry| (entry.id.as_str(), entry.budget_bytes))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(budgets["managed-workspace-target"], Some(4_294_967_296));
        assert_eq!(budgets["managed-workspace-build"], Some(25_769_803_776));
        assert_eq!(budgets["managed-coverage-target"], Some(2_147_483_648));
        assert_eq!(budgets["managed-coverage-build"], Some(8_589_934_592));
        assert_eq!(budgets["managed-mutation-reports"], Some(8_589_934_592));
        assert_eq!(budgets["legacy-repo-target"], Some(536_870_912));
        assert_eq!(budgets["legacy-repo-fuzz-target"], Some(536_870_912));
        assert_eq!(budgets["repo-tmp"], Some(268_435_456));
        assert_eq!(budgets["repo-tmp-cargo-targets"], None);
    });
}

#[test]
fn hygiene_report_repairs_missing_markers_for_existing_managed_roots() {
    let repo_root = tempdir().expect("repo tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        for path in [
            cargo_target_root(repo_root.path()),
            cargo_build_root(repo_root.path()),
            coverage_target_root(repo_root.path()),
            coverage_build_root(repo_root.path()),
            coverage_cargo_target_dir(repo_root.path()),
            coverage_cargo_build_dir(repo_root.path()),
            mutation_report_root(repo_root.path()),
        ] {
            fs::create_dir_all(&path).expect("create managed artifact root");
        }

        let report = hygiene_report(repo_root.path()).expect("hygiene report");

        for path in [
            cargo_target_root(repo_root.path()),
            cargo_build_root(repo_root.path()),
            coverage_target_root(repo_root.path()),
            coverage_build_root(repo_root.path()),
            coverage_cargo_target_dir(repo_root.path()),
            coverage_cargo_build_dir(repo_root.path()),
            mutation_report_root(repo_root.path()),
        ] {
            assert!(path.join("CACHEDIR.TAG").is_file());
            assert!(path.join(".ffhn-artifact.toml").is_file());
        }
        assert!(!report.violations.iter().any(|violation| {
            matches!(
                violation.id.as_str(),
                "managed-workspace-target"
                    | "managed-workspace-build"
                    | "managed-coverage-target"
                    | "managed-coverage-build"
                    | "managed-mutation-reports"
            )
        }));
    });
}

#[test]
fn hygiene_report_detects_legacy_target_roots_and_repo_tmp_cargo_roots() {
    let repo_root = tempdir().expect("repo tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        prepare_artifact_layout(repo_root.path(), CommandArtifactLayout::ManagedWorkspace)
            .expect("prepare workspace layout");
        let legacy_target = repo_root.path().join("target");
        let legacy_fuzz_target = repo_root.path().join("fuzz").join("target");
        let tmp_cargo_target = repo_root.path().join("tmp").join("cargo-target-audit");
        fs::create_dir_all(legacy_target.join("debug")).expect("create legacy target");
        fs::create_dir_all(legacy_fuzz_target.join("debug")).expect("create legacy fuzz target");
        fs::create_dir_all(tmp_cargo_target.join("debug")).expect("create tmp cargo target");
        fs::write(legacy_target.join(".rustc_info.json"), "{}").expect("write target rustc info");
        fs::write(legacy_fuzz_target.join(".rustc_info.json"), "{}")
            .expect("write fuzz rustc info");
        fs::write(tmp_cargo_target.join(".rustc_info.json"), "{}").expect("write tmp rustc info");

        let report = hygiene_report(repo_root.path()).expect("hygiene report");
        let rendered = render_hygiene_report(&report);

        let repo_tmp = report
            .entries
            .iter()
            .find(|entry| entry.id == "repo-tmp")
            .expect("repo tmp entry");
        assert_eq!(
            repo_tmp.details,
            vec![
                "Repository-local scratch root for temporary investigations.".to_owned(),
                "Excludes 1 repo-local Cargo target roots reported separately under repo-tmp-cargo-targets."
                    .to_owned(),
            ]
        );

        assert!(rendered.contains("repo-tmp-cargo-targets"));
        assert!(
            report
                .violations
                .iter()
                .any(|violation| violation.id == "repo-tmp-cargo-targets")
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.id == "legacy-repo-target" && entry.present)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.id == "legacy-repo-fuzz-target" && entry.present)
        );
        assert!(ensure_hygiene(repo_root.path()).is_err());
    });
}

#[test]
fn hygiene_report_does_not_claim_empty_repo_tmp_exclusions() {
    let repo_root = tempdir().expect("repo tempdir");
    fs::create_dir_all(repo_root.path().join("tmp")).expect("create tmp root");

    with_test_artifact_roots(repo_root.path(), || {
        let report = hygiene_report(repo_root.path()).expect("hygiene report");
        let repo_tmp = report
            .entries
            .iter()
            .find(|entry| entry.id == "repo-tmp")
            .expect("repo tmp entry");
        assert_eq!(
            repo_tmp.details,
            vec!["Repository-local scratch root for temporary investigations.".to_owned()]
        );
    });
}

#[test]
fn clean_hygiene_safe_removes_only_safe_roots() {
    let repo_root = tempdir().expect("repo tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        prepare_artifact_layout(repo_root.path(), CommandArtifactLayout::ManagedWorkspace)
            .expect("prepare workspace layout");
        prepare_artifact_layout(repo_root.path(), CommandArtifactLayout::ManagedCoverage)
            .expect("prepare coverage layout");
        let mutation_reports =
            prepare_mutation_report_root(repo_root.path()).expect("prepare mutation reports");
        fs::write(mutation_reports.join("evidence"), "retained").expect("mutation evidence");
        fs::create_dir_all(repo_root.path().join("target").join("debug"))
            .expect("create legacy target");
        fs::create_dir_all(repo_root.path().join("fuzz").join("target").join("debug"))
            .expect("create legacy fuzz target");
        fs::create_dir_all(repo_root.path().join("tmp").join("probe"))
            .expect("create repo tmp root");
        fs::create_dir_all(semver_scratch_dir(repo_root.path())).expect("create semver target");
        fs::create_dir_all(semver_build_dir(repo_root.path())).expect("create semver build");
        fs::create_dir_all(semver_baseline_target_dir(repo_root.path()))
            .expect("create semver baseline target");

        let result = clean_hygiene(repo_root.path(), HygieneCleanMode::Safe)
            .expect("safe clean should work");

        assert!(!repo_root.path().join("target").exists());
        assert!(!repo_root.path().join("fuzz").join("target").exists());
        assert!(!repo_root.path().join("tmp").exists());
        assert!(!semver_scratch_dir(repo_root.path()).exists());
        assert!(!semver_build_dir(repo_root.path()).exists());
        assert!(!semver_baseline_target_dir(repo_root.path()).exists());
        assert!(cargo_target_root(repo_root.path()).exists());
        assert!(cargo_build_root(repo_root.path()).exists());
        assert!(mutation_report_root(repo_root.path()).exists());
        assert!(result.reclaimed_bytes > 0);
    });
}

#[test]
fn clean_hygiene_rebuildable_also_removes_managed_roots() {
    let repo_root = tempdir().expect("repo tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        prepare_artifact_layout(repo_root.path(), CommandArtifactLayout::ManagedWorkspace)
            .expect("prepare workspace layout");
        prepare_mutation_report_root(repo_root.path()).expect("prepare mutation reports");

        let result = clean_hygiene(repo_root.path(), HygieneCleanMode::Rebuildable)
            .expect("rebuildable clean should work");

        assert!(!cargo_target_root(repo_root.path()).exists());
        assert!(!cargo_build_root(repo_root.path()).exists());
        assert!(!mutation_report_root(repo_root.path()).exists());
        assert_eq!(result.removed_paths.len(), 3);
    });
}

#[test]
fn render_hygiene_report_uses_none_when_no_violations() {
    let repo_root = tempdir().expect("repo tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        prepare_artifact_layout(repo_root.path(), CommandArtifactLayout::ManagedWorkspace)
            .expect("prepare workspace layout");
        prepare_artifact_layout(repo_root.path(), CommandArtifactLayout::ManagedCoverage)
            .expect("prepare coverage layout");
        prepare_mutation_report_root(repo_root.path()).expect("prepare mutation reports");

        let report = hygiene_report(repo_root.path()).expect("hygiene report");
        let rendered = render_hygiene_report(&report);

        assert!(report.violations.is_empty());
        assert!(rendered.contains("violations: none"));
        ensure_hygiene(repo_root.path()).expect("clean report should verify");
    });
}

#[test]
fn clean_hygiene_reports_remove_failures_for_non_directories() {
    let repo_root = tempdir().expect("repo tempdir");
    fs::create_dir_all(repo_root.path()).expect("create repo root");
    fs::write(repo_root.path().join("target"), "not a directory").expect("write target file");

    let error = clean_hygiene(repo_root.path(), HygieneCleanMode::Safe)
        .expect_err("non-directory target root should fail removal");

    assert!(
        error
            .to_string()
            .contains("failed to remove hygiene artifact root")
    );
}

#[test]
fn prepare_artifact_layout_reports_create_dir_failures() {
    let repo_root = tempdir().expect("repo tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        let target_root = cargo_target_root(repo_root.path());
        fs::create_dir_all(target_root.parent().expect("target parent"))
            .expect("create target parent");
        fs::write(&target_root, "blocking file").expect("write blocking file");

        let error =
            prepare_artifact_layout(repo_root.path(), CommandArtifactLayout::ManagedWorkspace)
                .expect_err("file at target root should fail create_dir_all");

        assert!(
            error
                .to_string()
                .contains("failed to create managed hygiene artifact root")
        );
    });
}

#[test]
fn prepare_coverage_artifact_layout_reports_nested_root_creation_failures() {
    let repo_root = tempdir().expect("repo tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        let nested_target = coverage_cargo_target_dir(repo_root.path());
        fs::create_dir_all(nested_target.parent().expect("nested target parent"))
            .expect("create nested target parent");
        fs::write(&nested_target, "blocking file").expect("write blocking file");

        let error =
            prepare_artifact_layout(repo_root.path(), CommandArtifactLayout::ManagedCoverage)
                .expect_err("file at nested coverage root should fail create_dir_all");

        assert!(
            error
                .to_string()
                .contains("failed to create managed hygiene artifact root")
        );
    });
}

#[cfg(unix)]
#[test]
fn prepare_artifact_layout_reports_cache_marker_failures() {
    let repo_root = tempdir().expect("repo tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        let target_root = cargo_target_root(repo_root.path());
        let build_root = cargo_build_root(repo_root.path());
        fs::create_dir_all(&target_root).expect("create target root");
        fs::create_dir_all(&build_root).expect("create build root");

        let mut permissions = fs::metadata(&target_root).expect("metadata").permissions();
        permissions.set_mode(0o500);
        fs::set_permissions(&target_root, permissions.clone()).expect("chmod target root");

        let error =
            prepare_artifact_layout(repo_root.path(), CommandArtifactLayout::ManagedWorkspace)
                .expect_err("read-only root should fail cachedir tag write");

        permissions.set_mode(0o700);
        fs::set_permissions(&target_root, permissions).expect("restore target root permissions");

        assert!(
            error
                .to_string()
                .contains("failed to write managed hygiene cache marker")
        );
    });
}

#[test]
fn prepare_artifact_layout_reports_manifest_write_failures() {
    let repo_root = tempdir().expect("repo tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        let target_root = cargo_target_root(repo_root.path());
        let build_root = cargo_build_root(repo_root.path());
        fs::create_dir_all(&target_root).expect("create target root");
        fs::create_dir_all(&build_root).expect("create build root");
        fs::create_dir_all(target_root.join(".ffhn-artifact.toml"))
            .expect("create manifest blocking dir");

        let error =
            prepare_artifact_layout(repo_root.path(), CommandArtifactLayout::ManagedWorkspace)
                .expect_err("blocking manifest directory should fail manifest write");

        assert!(
            error
                .to_string()
                .contains("failed to write managed hygiene manifest")
        );
    });
}

#[cfg(unix)]
#[test]
fn helper_functions_surface_filesystem_errors() {
    let repo_root = tempdir().expect("repo tempdir");
    let blocked_dir = repo_root.path().join("blocked");
    fs::create_dir_all(&blocked_dir).expect("create blocked dir");
    fs::write(blocked_dir.join("payload.bin"), vec![0u8; 16]).expect("write payload");

    let mut permissions = fs::metadata(&blocked_dir).expect("metadata").permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&blocked_dir, permissions.clone()).expect("chmod blocked dir");

    let aggregate_error =
        aggregate_entry_for_tests(repo_root.path(), std::slice::from_ref(&blocked_dir))
            .expect_err("aggregate helper should surface filesystem failures");
    let entry_error = entry_from_path_for_tests(&blocked_dir)
        .expect_err("entry helper should surface filesystem failures");

    permissions.set_mode(0o700);
    fs::set_permissions(&blocked_dir, permissions).expect("restore blocked dir permissions");

    assert!(
        aggregate_error
            .to_string()
            .contains("failed to inspect hygiene aggregate member")
    );
    assert!(
        entry_error
            .to_string()
            .contains("failed to inspect hygiene artifact root")
    );
}

#[cfg(unix)]
#[test]
fn hygiene_report_surfaces_managed_root_inspection_failures() {
    let repo_root = tempdir().expect("repo tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        let artifact_parent = repo_root.path().join(".managed-artifacts");
        fs::create_dir_all(&artifact_parent).expect("create artifact parent");
        let mut permissions = fs::metadata(&artifact_parent)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&artifact_parent, permissions.clone()).expect("chmod artifact parent");

        let error = hygiene_report(repo_root.path())
            .expect_err("inaccessible managed artifact roots must be reported");

        permissions.set_mode(0o700);
        fs::set_permissions(&artifact_parent, permissions)
            .expect("restore artifact parent permissions");

        assert!(
            error
                .to_string()
                .contains("failed to inspect hygiene artifact root")
        );
    });
}

#[cfg(unix)]
#[test]
fn hygiene_report_surfaces_legacy_root_inspection_failures() {
    let repo_root = tempdir().expect("repo tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        let legacy_target = repo_root.path().join("target");
        fs::create_dir_all(&legacy_target).expect("create legacy target");
        fs::write(legacy_target.join("payload.bin"), "payload").expect("write payload");
        let mut permissions = fs::metadata(&legacy_target)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&legacy_target, permissions.clone()).expect("chmod legacy target");

        let error = hygiene_report(repo_root.path())
            .expect_err("inaccessible legacy artifact roots must be reported");

        permissions.set_mode(0o700);
        fs::set_permissions(&legacy_target, permissions)
            .expect("restore legacy target permissions");

        assert!(
            error
                .to_string()
                .contains("failed to inspect hygiene artifact root")
        );
    });
}

mod policy;

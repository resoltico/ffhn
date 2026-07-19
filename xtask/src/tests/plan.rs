use super::*;

fn normalized_path_text(value: &str) -> String {
    value.replace('\\', "/")
}

#[test]
fn shell_script_paths_returns_sorted_shell_scripts_only() {
    let repo_root = tempdir().expect("tempdir");
    let scripts_dir = repo_root.path().join("scripts");
    let root_check = repo_root.path().join("check.sh");
    fs::create_dir_all(&scripts_dir).expect("create scripts dir");
    fs::write(&root_check, "#!/usr/bin/env bash\n").expect("write check.sh");
    fs::write(scripts_dir.join("b.sh"), "#!/usr/bin/env bash\n").expect("write b.sh");
    fs::write(scripts_dir.join("a.sh"), "#!/usr/bin/env bash\n").expect("write a.sh");
    fs::write(scripts_dir.join("note.txt"), "ignore").expect("write note.txt");

    let scripts = shell_script_paths(repo_root.path()).expect("script paths");

    assert_eq!(
        scripts,
        vec![
            root_check,
            scripts_dir.join("a.sh"),
            scripts_dir.join("b.sh")
        ]
    );
}

#[test]
fn shell_script_paths_returns_empty_when_scripts_dir_is_missing() {
    let repo_root = tempdir().expect("tempdir");

    let scripts = shell_script_paths(repo_root.path()).expect("script paths");

    assert!(scripts.is_empty());
}

#[test]
fn collect_shell_script_paths_keeps_errors_visible() {
    let error = collect_shell_script_paths(vec![Err(std::io::Error::other("boom"))])
        .expect_err("iteration error");
    assert!(error.to_string().contains("boom"));
}

#[test]
fn check_plan_includes_all_strict_gates() {
    let repo_root = tempdir().expect("tempdir");
    let tooling = sample_tooling();
    write_repo_scaffold(repo_root.path());
    let scripts_dir = repo_root.path().join("scripts");
    let root_check = repo_root.path().join("check.sh");
    fs::create_dir_all(&scripts_dir).expect("create scripts dir");
    fs::write(&root_check, "#!/usr/bin/env bash\n").expect("write check.sh");
    fs::write(scripts_dir.join("z.sh"), "#!/usr/bin/env bash\n").expect("write z.sh");
    fs::write(scripts_dir.join("a.sh"), "#!/usr/bin/env bash\n").expect("write a.sh");

    let plan = with_test_artifact_roots(repo_root.path(), || {
        check_plan(repo_root.path(), &tooling).expect("check plan")
    });

    assert_eq!(
        plan[0],
        CommandSpec::new(
            "bash",
            [
                "-n".to_owned(),
                root_check.to_string_lossy().into_owned(),
                scripts_dir.join("a.sh").to_string_lossy().into_owned(),
                scripts_dir.join("z.sh").to_string_lossy().into_owned(),
            ],
            false,
        )
        .with_step_id("shell-syntax")
    );
    assert_eq!(
        plan[1],
        CommandSpec::new(
            "shellcheck",
            [
                root_check.to_string_lossy().into_owned(),
                scripts_dir.join("a.sh").to_string_lossy().into_owned(),
                scripts_dir.join("z.sh").to_string_lossy().into_owned(),
            ],
            false,
        )
        .with_step_id("shellcheck")
    );
    assert!(plan.iter().any(|spec| spec.args == ["xtask", "audit"]));
    assert!(plan.iter().all(|spec| spec.step_id != "unnamed-command"));
    assert!(plan.iter().any(|spec| {
        spec.args == ["xtask", "miri"]
            && spec.artifact_layout == CommandArtifactLayout::ManagedWorkspace
    }));
    assert!(plan.iter().any(|spec| {
        spec.args == ["fmt", "--check"]
            && spec.artifact_layout == CommandArtifactLayout::ManagedWorkspace
    }));
    assert!(plan.iter().any(|spec| {
        spec.args
            == [
                "xtask".to_owned(),
                "audit".to_owned(),
                "--file".to_owned(),
                repo_root
                    .path()
                    .join("fuzz")
                    .join("Cargo.lock")
                    .to_string_lossy()
                    .into_owned(),
            ]
    }));
    assert!(plan.iter().any(|spec| {
        spec.args
            == [
                "clippy".to_owned(),
                "--manifest-path".to_owned(),
                repo_root
                    .path()
                    .join("fuzz")
                    .join("Cargo.toml")
                    .to_string_lossy()
                    .into_owned(),
                "--bins".to_owned(),
                "--locked".to_owned(),
                "--".to_owned(),
                "-D".to_owned(),
                "warnings".to_owned(),
            ]
    }));
    assert!(plan.iter().any(|spec| {
        spec.args
            == [
                "+nightly-2026-05-11".to_owned(),
                "fuzz".to_owned(),
                "check".to_owned(),
                "--fuzz-dir".to_owned(),
                "fuzz".to_owned(),
            ]
    }));
    assert!(plan.iter().any(|spec| {
        spec.args
            .windows(2)
            .any(|window| window == ["--release-type", "major"])
    }));
    assert!(plan.iter().any(|spec| {
        spec.args
            == [
                "nextest",
                "run",
                "--no-fail-fast",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--locked",
            ]
    }));
    assert!(plan.iter().any(|spec| {
        spec.args
            == [
                "doc".to_owned(),
                "--workspace".to_owned(),
                "--all-features".to_owned(),
                "--no-deps".to_owned(),
                "--locked".to_owned(),
            ]
            && spec.env.get("RUSTDOCFLAGS") == Some(&"-D warnings".to_owned())
    }));
    let semver_spec = plan
        .iter()
        .find(|spec| is_semver_check_spec(spec))
        .expect("semver gate");
    assert_eq!(
        semver_spec
            .env
            .get("CARGO_TARGET_DIR")
            .map(|value| normalized_path_text(value)),
        Some(normalized_path_text(
            &semver_scratch_dir_for_tests(
                repo_root.path(),
                Some(Path::new(".managed-artifacts/target"))
            )
            .to_string_lossy(),
        ))
    );
    assert_eq!(
        semver_spec
            .env
            .get("CARGO_BUILD_BUILD_DIR")
            .map(|value| normalized_path_text(value)),
        Some(normalized_path_text(
            &repo_root
                .path()
                .join(".managed-artifacts")
                .join("build")
                .join("semver-checks")
                .to_string_lossy(),
        ))
    );
    assert_eq!(
        plan.last().expect("release smoke"),
        &CommandSpec::new(
            release_binary_path_for_tests(
                repo_root.path(),
                Some(Path::new(".managed-artifacts/target"))
            ),
            ["--version"],
            true,
        )
        .with_step_id("dist-smoke")
    );
}

#[test]
fn check_plan_skips_shell_gates_when_no_scripts_exist() {
    let repo_root = tempdir().expect("tempdir");
    let tooling = sample_tooling();
    write_repo_scaffold(repo_root.path());

    let plan = with_test_artifact_roots(repo_root.path(), || {
        check_plan(repo_root.path(), &tooling).expect("check plan")
    });

    assert_eq!(
        plan[0],
        CommandSpec::new("cargo", ["fmt", "--check"], false)
            .with_step_id("format")
            .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace)
    );
}

#[test]
fn semver_scratch_dir_lives_under_the_managed_target_root() {
    let repo_root = tempdir().expect("tempdir");
    with_test_artifact_roots(repo_root.path(), || {
        assert_eq!(
            semver_scratch_dir(repo_root.path()),
            repo_root
                .path()
                .join(".managed-artifacts")
                .join("target")
                .join("semver-checks")
        );
    });
}

#[test]
fn cargo_path_helpers_follow_the_configured_roots() {
    let repo_root = tempdir().expect("tempdir");
    with_test_artifact_roots(repo_root.path(), || {
        assert_eq!(
            cargo_target_root(repo_root.path()),
            repo_root.path().join(".managed-artifacts").join("target")
        );
        assert_eq!(
            cargo_build_root(repo_root.path()),
            repo_root.path().join(".managed-artifacts").join("build")
        );
    });

    assert_eq!(
        cargo_target_root_for_tests(repo_root.path(), Some(Path::new("custom-target"))),
        repo_root.path().join("custom-target")
    );
    let absolute_target_root = tempdir().expect("absolute target root");
    assert_eq!(
        cargo_target_root_for_tests(repo_root.path(), Some(absolute_target_root.path())),
        absolute_target_root.path()
    );
    let absolute_build_root = tempdir().expect("absolute build root");
    assert_eq!(
        cargo_build_root_for_tests(repo_root.path(), Some(absolute_build_root.path())),
        absolute_build_root.path()
    );
    assert_eq!(
        coverage_target_root_for_tests(repo_root.path(), Some(Path::new("managed-target"))),
        repo_root.path().join("coverage-target")
    );
    assert_eq!(
        coverage_build_root_for_tests(repo_root.path(), Some(Path::new("managed-build"))),
        repo_root.path().join("coverage-build")
    );
    assert_eq!(
        coverage_cargo_target_dir_for_tests(repo_root.path(), Some(Path::new("managed-target"))),
        repo_root
            .path()
            .join("coverage-target")
            .join("llvm-cov-target")
    );
    assert_eq!(
        coverage_cargo_build_dir_for_tests(repo_root.path(), Some(Path::new("managed-build"))),
        repo_root
            .path()
            .join("coverage-build")
            .join("llvm-cov-target")
    );
    assert_eq!(
        semver_scratch_dir_for_tests(repo_root.path(), Some(Path::new("custom-target"))),
        repo_root.path().join("custom-target").join("semver-checks")
    );
    assert_eq!(
        release_binary_path_for_tests(repo_root.path(), Some(Path::new("custom-target"))),
        repo_root
            .path()
            .join("custom-target")
            .join("dist")
            .join(binary_name())
    );
    assert_eq!(
        sibling_artifact_dir_for_tests(Path::new("target"), "coverage-target"),
        PathBuf::from("coverage-target")
    );
}

#[test]
fn cargo_path_helpers_fall_back_when_cargo_config_is_invalid() {
    let repo_root = tempdir().expect("tempdir");
    let cargo_dir = repo_root.path().join(".cargo");
    fs::create_dir_all(&cargo_dir).expect("create cargo dir");
    fs::write(cargo_dir.join("config.toml"), "[build\nbroken = true\n")
        .expect("write invalid cargo config");

    assert_eq!(
        cargo_target_root(repo_root.path()),
        repo_root.path().join("target")
    );
    assert_eq!(
        cargo_build_root(repo_root.path()),
        repo_root.path().join("target")
    );
}

#[test]
fn semver_baseline_target_dir_points_inside_the_unpacked_baseline() {
    let repo_root = tempdir().expect("tempdir");

    assert_eq!(
        semver_baseline_target_dir(repo_root.path()),
        repo_root
            .path()
            .join("semver-baseline")
            .join("ffhn-core")
            .join("target")
    );
}

#[test]
fn is_semver_check_spec_matches_only_the_semver_gate() {
    assert!(is_semver_check_spec(&CommandSpec::new(
        "cargo",
        ["semver-checks", "--all-features"],
        false,
    )));
    assert!(!is_semver_check_spec(&CommandSpec::new(
        "cargo",
        ["nextest", "run"],
        false,
    )));
    assert!(!is_semver_check_spec(&CommandSpec::new(
        "bash",
        ["scripts/qa-gate.sh"],
        false,
    )));
}

#[cfg(unix)]
#[test]
fn tracked_files_canonicalize_the_expected_maintained_sources() {
    let repo_root = tempdir().expect("tempdir");
    seed_tracked_files(repo_root.path());

    let tracked = tracked_files(repo_root.path()).expect("tracked files");

    assert_eq!(tracked.len(), SEEDED_TRACKED_FILES.len());
    for relative_path in SEEDED_TRACKED_FILES {
        let absolute_path =
            normalize_path(repo_root.path(), &repo_root.path().join(relative_path)).expect("path");
        assert_eq!(
            tracked.get(&absolute_path),
            Some(&relative_path.to_string())
        );
    }
}

#[test]
fn normalize_path_supports_relative_and_absolute_inputs() {
    let repo_root = tempdir().expect("tempdir");
    let file_path = repo_root.path().join("scripts").join("lint.sh");
    fs::create_dir_all(file_path.parent().expect("parent")).expect("create dir");
    fs::write(&file_path, "#!/usr/bin/env bash\n").expect("write script");

    let from_relative =
        normalize_path(repo_root.path(), Path::new("scripts/lint.sh")).expect("relative");
    let from_absolute = normalize_path(repo_root.path(), &file_path).expect("absolute");

    assert_eq!(from_relative, from_absolute);
}

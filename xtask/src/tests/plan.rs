use super::*;

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
    write_repo_scaffold(repo_root.path());
    let scripts_dir = repo_root.path().join("scripts");
    let root_check = repo_root.path().join("check.sh");
    fs::create_dir_all(&scripts_dir).expect("create scripts dir");
    fs::write(&root_check, "#!/usr/bin/env bash\n").expect("write check.sh");
    fs::write(scripts_dir.join("z.sh"), "#!/usr/bin/env bash\n").expect("write z.sh");
    fs::write(scripts_dir.join("a.sh"), "#!/usr/bin/env bash\n").expect("write a.sh");

    let plan = check_plan(repo_root.path()).expect("check plan");

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
            false,
        )
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
            false,
        )
    );
    assert!(plan.iter().any(|spec| spec.args
        == [
            "outdated",
            "--workspace",
            "--root-deps-only",
            "--exit-code",
            "1"
        ]));
    assert!(plan.iter().any(|spec| {
        spec.args
            == [
                "outdated".to_owned(),
                "--manifest-path".to_owned(),
                repo_root
                    .path()
                    .join("fuzz")
                    .join("Cargo.toml")
                    .to_string_lossy()
                    .into_owned(),
                "--root-deps-only".to_owned(),
                "--exit-code".to_owned(),
                "1".to_owned(),
            ]
    }));
    assert!(
        plan.iter()
            .any(|spec| spec.args == ["audit", "-D", "warnings"])
    );
    assert!(plan.iter().any(|spec| {
        spec.args
            == [
                "audit".to_owned(),
                "--file".to_owned(),
                repo_root
                    .path()
                    .join("fuzz")
                    .join("Cargo.lock")
                    .to_string_lossy()
                    .into_owned(),
                "-D".to_owned(),
                "warnings".to_owned(),
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
    let semver_spec = plan
        .iter()
        .find(|spec| is_semver_check_spec(spec))
        .expect("semver gate");
    assert_eq!(
        semver_spec.env.get("CARGO_TARGET_DIR"),
        Some(
            &semver_scratch_dir(repo_root.path())
                .to_string_lossy()
                .into_owned()
        )
    );
    assert_eq!(
        plan.last().expect("release smoke"),
        &CommandSpec::new(
            release_binary_path(repo_root.path()),
            ["--version"],
            true,
            false
        )
    );
}

#[test]
fn check_plan_skips_shell_gates_when_no_scripts_exist() {
    let repo_root = tempdir().expect("tempdir");
    write_repo_scaffold(repo_root.path());

    let plan = check_plan(repo_root.path()).expect("check plan");

    assert_eq!(
        plan[0],
        CommandSpec::new("cargo", ["fmt", "--check"], false, false)
    );
}

#[test]
fn semver_scratch_dir_uses_target_tree() {
    let repo_root = tempdir().expect("tempdir");

    assert_eq!(
        semver_scratch_dir(repo_root.path()),
        repo_root.path().join("target").join("semver-checks")
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
        true
    )));
    assert!(!is_semver_check_spec(&CommandSpec::new(
        "cargo",
        ["nextest", "run"],
        false,
        true
    )));
    assert!(!is_semver_check_spec(&CommandSpec::new(
        "bash",
        ["scripts/qa-gate.sh"],
        false,
        false
    )));
}

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

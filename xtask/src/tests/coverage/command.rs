use super::*;

#[test]
fn coverage_command_targets_repo_coverage_file() {
    let repo_root = tempdir().expect("tempdir");
    let tooling = sample_tooling();

    let command = with_test_artifact_roots(repo_root.path(), || {
        coverage_command(repo_root.path(), &tooling)
    });
    let clean = with_test_artifact_roots(repo_root.path(), || coverage_clean_command(&tooling));

    assert_eq!(command.program, PathBuf::from("cargo"));
    assert_eq!(command.env.get("CC"), Some(&"clang".to_owned()));
    assert_eq!(
        command.artifact_layout,
        CommandArtifactLayout::ManagedCoverage
    );
    assert_eq!(
        command.args,
        vec![
            "+nightly-2026-08-25".to_owned(),
            "llvm-cov".to_owned(),
            "--branch".to_owned(),
            "--workspace".to_owned(),
            "--all-targets".to_owned(),
            "--all-features".to_owned(),
            "--locked".to_owned(),
            "--json".to_owned(),
            "--output-path".to_owned(),
            repo_root
                .path()
                .join(".managed-artifacts")
                .join("coverage-target")
                .join("coverage.json")
                .to_string_lossy()
                .into_owned(),
            "--".to_owned(),
            "--test-threads=1".to_owned(),
        ]
    );
    assert_eq!(clean.program, PathBuf::from("cargo"));
    assert_eq!(
        clean.args,
        vec![
            "+nightly-2026-08-25".to_owned(),
            "llvm-cov".to_owned(),
            "clean".to_owned(),
            "--workspace".to_owned(),
        ]
    );
    assert_eq!(
        clean.artifact_layout,
        CommandArtifactLayout::ManagedCoverage
    );
}

#[test]
fn coverage_command_targets_the_configured_coverage_root() {
    let repo_root = tempdir().expect("tempdir");
    let tooling = sample_tooling();

    let relative_command = with_cargo_artifact_root_overrides(
        repo_root.path().join("custom-target"),
        repo_root.path().join("custom-build"),
        || coverage_command(repo_root.path(), &tooling),
    );
    assert_eq!(
        relative_command.args[9],
        repo_root
            .path()
            .join("coverage-target")
            .join("coverage.json")
            .to_string_lossy()
            .into_owned()
    );

    let absolute_target_root = tempdir().expect("absolute target root");
    let absolute_build_root = tempdir().expect("absolute build root");
    let absolute_command = with_cargo_artifact_root_overrides(
        absolute_target_root.path().to_path_buf(),
        absolute_build_root.path().to_path_buf(),
        || coverage_command(repo_root.path(), &tooling),
    );
    assert_eq!(
        absolute_command.args[9],
        absolute_target_root
            .path()
            .parent()
            .expect("absolute target root parent")
            .join("coverage-target")
            .join("coverage.json")
            .to_string_lossy()
            .into_owned()
    );
}

#[test]
fn coverage_output_path_tracks_the_configured_roots() {
    let repo_root = tempdir().expect("tempdir");

    with_test_artifact_roots(repo_root.path(), || {
        assert_eq!(
            coverage_output_path(repo_root.path()),
            repo_root
                .path()
                .join(".managed-artifacts")
                .join("coverage-target")
                .join("coverage.json")
        );
    });
}

#[test]
fn read_coverage_report_loads_json_from_disk() {
    let repo_root = tempdir().expect("tempdir");
    let coverage_path = repo_root.path().join("coverage.json");
    fs::write(
        &coverage_path,
        r#"{"data":[{"files":[{"filename":"tracked.rs","segments":[[7,0,1,false,true,false]],"summary":{"branches":{"count":1,"covered":1,"notcovered":0}}}]}]}"#,
    )
    .expect("write coverage report");

    let report = read_coverage_report(&coverage_path).expect("read coverage report");

    assert_eq!(report.data.len(), 1);
    assert_eq!(report.data[0].files.len(), 1);
    assert_eq!(
        report.data[0].files[0].filename,
        PathBuf::from("tracked.rs")
    );
}

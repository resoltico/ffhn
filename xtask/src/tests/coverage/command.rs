use super::*;

#[test]
fn coverage_command_targets_repo_coverage_file() {
    let repo_root = tempdir().expect("tempdir");

    let command = with_cargo_target_dir(None, || coverage_command(repo_root.path()));
    let clean = coverage_clean_command();

    assert_eq!(command.program, PathBuf::from("cargo"));
    assert!(command.force_clang);
    assert_eq!(
        command.args,
        vec![
            "+nightly".to_owned(),
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
                .join("target")
                .join("coverage.json")
                .to_string_lossy()
                .into_owned(),
            "--".to_owned(),
            "--test-threads=1".to_owned(),
        ]
    );
    assert_eq!(clean.program, PathBuf::from("cargo"));
    assert!(!clean.force_clang);
    assert_eq!(
        clean.args,
        vec![
            "+nightly".to_owned(),
            "llvm-cov".to_owned(),
            "clean".to_owned(),
            "--workspace".to_owned(),
        ]
    );
}

#[test]
fn coverage_command_honors_the_active_cargo_target_root() {
    let repo_root = tempdir().expect("tempdir");
    let absolute_target_root = tempdir().expect("absolute target root");

    let relative_command = with_cargo_target_dir(Some(Path::new("custom-target")), || {
        coverage_command(repo_root.path())
    });
    assert_eq!(
        relative_command.args[9],
        repo_root
            .path()
            .join("custom-target")
            .join("coverage.json")
            .to_string_lossy()
            .into_owned()
    );

    let absolute_command = with_cargo_target_dir(Some(absolute_target_root.path()), || {
        coverage_command(repo_root.path())
    });
    assert_eq!(
        absolute_command.args[9],
        absolute_target_root
            .path()
            .join("coverage.json")
            .to_string_lossy()
            .into_owned()
    );
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

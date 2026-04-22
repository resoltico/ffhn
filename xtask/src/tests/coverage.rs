use super::*;

#[test]
fn coverage_command_targets_repo_coverage_file() {
    let repo_root = tempdir().expect("tempdir");

    let command = coverage_command(repo_root.path());
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

#[test]
fn evaluate_coverage_report_merges_duplicate_segments_and_ignores_untracked_files() {
    let repo_root = tempdir().expect("tempdir");
    let tracked = tracked_subset(
        repo_root.path(),
        &[
            "crates/ffhn-core/src/canonical.rs",
            "crates/ffhn-core/src/fetch.rs",
            "crates/ffhn-core/src/model/report.rs",
            "xtask/src/model.rs",
        ],
    );
    let extra_file = repo_root.path().join("notes.txt");
    fs::write(&extra_file, "ignore").expect("write extra file");

    let report = CoverageReport {
        data: vec![
            CoverageDataSet {
                files: vec![
                    CoverageFile {
                        filename: repo_root.path().join("crates/ffhn-core/src/canonical.rs"),
                        segments: vec![
                            (10, 0, 0, false, true, false),
                            (11, 0, 0, false, false, false),
                        ],
                        branches: Vec::new(),
                        summary: CoverageFileSummary {
                            branches: CoverageCounter {
                                count: 1,
                                covered: 1,
                                not_covered: 0,
                            },
                        },
                    },
                    CoverageFile {
                        filename: extra_file,
                        segments: vec![(99, 0, 1, false, true, false)],
                        branches: Vec::new(),
                        summary: CoverageFileSummary::default(),
                    },
                ],
            },
            CoverageDataSet {
                files: vec![
                    CoverageFile {
                        filename: repo_root.path().join("crates/ffhn-core/src/canonical.rs"),
                        segments: vec![(10, 0, 2, false, true, false)],
                        branches: Vec::new(),
                        summary: CoverageFileSummary {
                            branches: CoverageCounter {
                                count: 1,
                                covered: 1,
                                not_covered: 0,
                            },
                        },
                    },
                    CoverageFile {
                        filename: repo_root.path().join("crates/ffhn-core/src/fetch.rs"),
                        segments: vec![(20, 0, 1, false, true, false)],
                        branches: Vec::new(),
                        summary: CoverageFileSummary {
                            branches: CoverageCounter {
                                count: 2,
                                covered: 2,
                                not_covered: 0,
                            },
                        },
                    },
                    CoverageFile {
                        filename: repo_root
                            .path()
                            .join("crates/ffhn-core/src/model/report.rs"),
                        segments: vec![(30, 0, 1, false, true, false)],
                        branches: Vec::new(),
                        summary: CoverageFileSummary::default(),
                    },
                    CoverageFile {
                        filename: repo_root.path().join("xtask/src/model.rs"),
                        segments: vec![(40, 0, 1, false, true, false)],
                        branches: Vec::new(),
                        summary: CoverageFileSummary {
                            branches: CoverageCounter {
                                count: 3,
                                covered: 3,
                                not_covered: 0,
                            },
                        },
                    },
                ],
            },
        ],
    };

    let summary =
        evaluate_coverage_report(repo_root.path(), &tracked, report).expect("coverage summary");

    assert_eq!(summary.tracked_line_count, 4);
    assert_eq!(summary.tracked_branch_count, 6);
    assert!(summary.failures.is_empty());
}

#[test]
fn evaluate_coverage_report_deduplicates_duplicate_branch_spans() {
    let repo_root = tempdir().expect("tempdir");
    let tracked = tracked_subset(
        repo_root.path(),
        &[
            "crates/ffhn-core/src/runtime/run/execute.rs",
            "crates/ffhn-cli/src/execute.rs",
            "xtask/src/plan.rs",
            "xtask/src/coverage.rs",
        ],
    );

    let report = CoverageReport {
        data: vec![CoverageDataSet {
            files: vec![
                CoverageFile {
                    filename: repo_root
                        .path()
                        .join("crates/ffhn-core/src/runtime/run/execute.rs"),
                    segments: vec![(7, 0, 1, false, true, false)],
                    branches: Vec::new(),
                    summary: CoverageFileSummary::default(),
                },
                CoverageFile {
                    filename: repo_root.path().join("crates/ffhn-cli/src/execute.rs"),
                    segments: vec![(9, 0, 1, false, true, false)],
                    branches: vec![
                        (12, 0, 12, 24, 0, 0, 0, 0, 4),
                        (12, 0, 12, 24, 3, 2, 0, 0, 4),
                    ],
                    summary: CoverageFileSummary {
                        branches: CoverageCounter {
                            count: 2,
                            covered: 0,
                            not_covered: 2,
                        },
                    },
                },
                CoverageFile {
                    filename: repo_root.path().join("xtask/src/plan.rs"),
                    segments: vec![(11, 0, 1, false, true, false)],
                    branches: Vec::new(),
                    summary: CoverageFileSummary::default(),
                },
                CoverageFile {
                    filename: repo_root.path().join("xtask/src/coverage.rs"),
                    segments: vec![(13, 0, 1, false, true, false)],
                    branches: Vec::new(),
                    summary: CoverageFileSummary::default(),
                },
            ],
        }],
    };

    let summary =
        evaluate_coverage_report(repo_root.path(), &tracked, report).expect("coverage summary");

    assert_eq!(summary.tracked_line_count, 4);
    assert_eq!(summary.tracked_branch_count, 2);
    assert!(summary.failures.is_empty());
}

#[test]
fn evaluate_coverage_report_reports_uncovered_and_missing_files() {
    let repo_root = tempdir().expect("tempdir");
    let tracked = seed_tracked_files(repo_root.path());

    let report = CoverageReport {
        data: vec![CoverageDataSet {
            files: vec![CoverageFile {
                filename: repo_root
                    .path()
                    .join("crates/ffhn-core/src/model/report.rs"),
                segments: vec![(7, 0, 0, false, true, false)],
                branches: Vec::new(),
                summary: CoverageFileSummary {
                    branches: CoverageCounter {
                        count: 2,
                        covered: 1,
                        not_covered: 1,
                    },
                },
            }],
        }],
    };

    let summary =
        evaluate_coverage_report(repo_root.path(), &tracked, report).expect("coverage summary");

    assert_eq!(summary.tracked_line_count, 1);
    assert_eq!(summary.tracked_branch_count, 2);
    let core_failure = summary
        .failures
        .iter()
        .find(|failure| failure.file == "crates/ffhn-core/src/model/report.rs")
        .expect("core failure");
    assert_eq!(core_failure.uncovered_lines, vec!["7".to_owned()]);
    assert_eq!(core_failure.uncovered_branch_count, 1);
    assert!(
        summary.failures.iter().any(
            |failure| failure.uncovered_lines == vec!["<no executable lines found>".to_owned()]
        )
    );
}

#[test]
fn evaluate_coverage_report_reports_branch_only_failures() {
    let repo_root = tempdir().expect("tempdir");
    let tracked = seed_tracked_files(repo_root.path());

    let report = CoverageReport {
        data: vec![CoverageDataSet {
            files: vec![
                CoverageFile {
                    filename: repo_root
                        .path()
                        .join("crates/ffhn-core/src/model/report.rs"),
                    segments: vec![(7, 0, 1, false, true, false)],
                    branches: Vec::new(),
                    summary: CoverageFileSummary {
                        branches: CoverageCounter {
                            count: 2,
                            covered: 2,
                            not_covered: 0,
                        },
                    },
                },
                CoverageFile {
                    filename: repo_root.path().join("crates/ffhn-cli/src/execute.rs"),
                    segments: vec![(9, 0, 1, false, true, false)],
                    branches: Vec::new(),
                    summary: CoverageFileSummary {
                        branches: CoverageCounter {
                            count: 3,
                            covered: 2,
                            not_covered: 1,
                        },
                    },
                },
                CoverageFile {
                    filename: repo_root.path().join("xtask/src/plan.rs"),
                    segments: vec![(11, 0, 1, false, true, false)],
                    branches: Vec::new(),
                    summary: CoverageFileSummary {
                        branches: CoverageCounter {
                            count: 1,
                            covered: 1,
                            not_covered: 0,
                        },
                    },
                },
            ],
        }],
    };

    let summary =
        evaluate_coverage_report(repo_root.path(), &tracked, report).expect("coverage summary");

    let cli_failure = summary
        .failures
        .iter()
        .find(|failure| failure.file == "crates/ffhn-cli/src/execute.rs")
        .expect("cli branch-only failure");
    assert!(cli_failure.uncovered_lines.is_empty());
    assert_eq!(cli_failure.uncovered_branch_count, 1);
}

#[cfg(windows)]
#[test]
fn binary_name_matches_the_current_platform() {
    assert_eq!(binary_name(), "ffhn.exe");
}

#[cfg(not(windows))]
#[test]
fn binary_name_matches_the_current_platform() {
    assert_eq!(binary_name(), "ffhn");
}

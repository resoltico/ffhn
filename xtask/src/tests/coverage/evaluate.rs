use super::*;

#[test]
fn evaluate_coverage_report_merges_duplicate_segments_and_ignores_untracked_files() {
    let repo_root = tempdir().expect("tempdir");
    let tracked = tracked_subset(
        repo_root.path(),
        &[
            "crates/ffhn-core/src/canonical.rs",
            "crates/ffhn-core/src/fetch.rs",
            "crates/ffhn-core/src/model/report/run.rs",
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
                        segments: vec![
                            (10, 0, 2, false, true, false),
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
                        filename: repo_root.path().join("crates/ffhn-core/src/fetch.rs"),
                        segments: vec![
                            (20, 0, 1, false, true, false),
                            (21, 0, 0, false, false, false),
                        ],
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
                            .join("crates/ffhn-core/src/model/report/run.rs"),
                        segments: vec![
                            (30, 0, 1, false, true, false),
                            (31, 0, 0, false, false, false),
                        ],
                        branches: Vec::new(),
                        summary: CoverageFileSummary::default(),
                    },
                    CoverageFile {
                        filename: repo_root.path().join("xtask/src/model.rs"),
                        segments: vec![
                            (40, 0, 1, false, true, false),
                            (41, 0, 0, false, false, false),
                        ],
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
            "xtask/src/plan/check.rs",
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
                    segments: vec![
                        (7, 0, 1, false, true, false),
                        (8, 0, 0, false, false, false),
                    ],
                    branches: Vec::new(),
                    summary: CoverageFileSummary::default(),
                },
                CoverageFile {
                    filename: repo_root.path().join("crates/ffhn-cli/src/execute.rs"),
                    segments: vec![
                        (9, 0, 1, false, true, false),
                        (10, 0, 0, false, false, false),
                    ],
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
                    filename: repo_root.path().join("xtask/src/plan/check.rs"),
                    segments: vec![
                        (11, 0, 1, false, true, false),
                        (12, 0, 0, false, false, false),
                    ],
                    branches: Vec::new(),
                    summary: CoverageFileSummary::default(),
                },
                CoverageFile {
                    filename: repo_root.path().join("xtask/src/coverage.rs"),
                    segments: vec![
                        (13, 0, 1, false, true, false),
                        (14, 0, 0, false, false, false),
                    ],
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
                    .join("crates/ffhn-core/src/model/report/run.rs"),
                segments: vec![
                    (7, 0, 0, false, true, false),
                    (8, 0, 0, false, false, false),
                ],
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
        .find(|failure| failure.file == "crates/ffhn-core/src/model/report/run.rs")
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
                        .join("crates/ffhn-core/src/model/report/run.rs"),
                    segments: vec![
                        (7, 0, 1, false, true, false),
                        (8, 0, 0, false, false, false),
                    ],
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
                    segments: vec![
                        (9, 0, 1, false, true, false),
                        (10, 0, 0, false, false, false),
                    ],
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
                    filename: repo_root.path().join("xtask/src/plan/check.rs"),
                    segments: vec![
                        (11, 0, 1, false, true, false),
                        (12, 0, 0, false, false, false),
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

#[test]
fn evaluate_coverage_report_treats_multiline_regions_as_covered_lines() {
    let repo_root = tempdir().expect("tempdir");
    let tracked = tracked_subset(repo_root.path(), &["xtask/src/app/semver.rs"]);

    let report = CoverageReport {
        data: vec![CoverageDataSet {
            files: vec![CoverageFile {
                filename: repo_root.path().join("xtask/src/app/semver.rs"),
                segments: vec![
                    (1, 1, 1, false, true, false),
                    (5, 1, 0, false, false, false),
                ],
                branches: Vec::new(),
                summary: CoverageFileSummary::default(),
            }],
        }],
    };

    let summary =
        evaluate_coverage_report(repo_root.path(), &tracked, report).expect("coverage summary");

    assert_eq!(summary.tracked_line_count, 4);
    assert!(summary.failures.is_empty());
}

#[test]
fn evaluate_coverage_report_skips_module_barrels_with_no_executable_regions() {
    let repo_root = tempdir().expect("tempdir");
    let barrel_path = repo_root.path().join("xtask/src/lib.rs");
    fs::create_dir_all(barrel_path.parent().expect("parent")).expect("create dir");
    fs::write(&barrel_path, "mod app;\npub use app::run;\n").expect("write barrel");

    let tracked = tracked_files(repo_root.path()).expect("tracked files");
    let summary = evaluate_coverage_report(
        repo_root.path(),
        &tracked,
        CoverageReport { data: Vec::new() },
    )
    .expect("coverage summary");

    assert_eq!(summary.tracked_line_count, 0);
    assert_eq!(summary.tracked_branch_count, 0);
    assert!(summary.failures.is_empty());
}

#[test]
fn evaluate_coverage_report_rejects_tracked_sources_when_segments_never_cover_executable_code() {
    let repo_root = tempdir().expect("tempdir");
    let file_path = repo_root.path().join("xtask/src/app/check.rs");
    fs::create_dir_all(file_path.parent().expect("parent")).expect("create dir");
    fs::write(&file_path, "fn tracked(\n) {\n}\n").expect("write tracked file");
    let tracked = tracked_files(repo_root.path()).expect("tracked files");

    let report = CoverageReport {
        data: vec![CoverageDataSet {
            files: vec![CoverageFile {
                filename: repo_root.path().join("xtask/src/app/check.rs"),
                segments: vec![
                    (2, 1, 0, false, true, false),
                    (2, 2, 0, false, false, false),
                ],
                branches: Vec::new(),
                summary: CoverageFileSummary::default(),
            }],
        }],
    };

    let summary =
        evaluate_coverage_report(repo_root.path(), &tracked, report).expect("coverage summary");

    assert!(summary.failures.iter().any(|failure| {
        failure.file == "xtask/src/app/check.rs"
            && failure.uncovered_lines == vec!["<no executable lines found>".to_owned()]
    }));
}

#[test]
fn evaluate_coverage_report_ignores_non_executable_files_even_when_the_report_mentions_only_punctuation()
 {
    let repo_root = tempdir().expect("tempdir");
    let file_path = repo_root.path().join("xtask/src/lib.rs");
    fs::create_dir_all(file_path.parent().expect("parent")).expect("create dir");
    fs::write(&file_path, "mod app;\npub use app::run;\n").expect("write barrel");
    let tracked = tracked_files(repo_root.path()).expect("tracked files");

    let report = CoverageReport {
        data: vec![CoverageDataSet {
            files: vec![CoverageFile {
                filename: repo_root.path().join("xtask/src/lib.rs"),
                segments: vec![
                    (1, 1, 0, false, true, false),
                    (1, 2, 0, false, false, false),
                ],
                branches: Vec::new(),
                summary: CoverageFileSummary::default(),
            }],
        }],
    };

    let summary =
        evaluate_coverage_report(repo_root.path(), &tracked, report).expect("coverage summary");

    assert!(summary.failures.is_empty());
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

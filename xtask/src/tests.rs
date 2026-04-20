use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

use crate::app::refresh_semver_baseline;
use crate::coverage::{
    coverage_clean_command, coverage_command, evaluate_coverage_report, read_coverage_report,
    tracked_files,
};
use crate::model::{
    CommandSpec, CoverageCounter, CoverageDataSet, CoverageFile, CoverageFileSummary,
    CoverageReport, TRACKED_RELATIVE_PATHS,
};
use crate::plan::{
    binary_name, check_plan, collect_shell_script_paths, is_semver_check_spec, normalize_path,
    release_binary_path, release_tag_exists, semver_release_type, semver_release_type_from_git_tag,
    semver_scratch_dir, shell_script_paths, with_workspace_stub, workspace_version,
    workspace_version_from_manifest,
};

fn write_repo_scaffold(repo_root: &Path) {
    fs::write(
        repo_root.join("Cargo.toml"),
        "[workspace.package]\nversion = \"2.0.0\"\n",
    )
    .expect("write Cargo.toml");
    fs::write(repo_root.join("changelog.md"), "## [Unreleased]\n").expect("write changelog.md");
}

fn write_semver_fixture(repo_root: &Path, workspace_version: &str, lib_body: &str) {
    let crate_dir = repo_root.join("crates").join("ffhn-core");
    let src_dir = crate_dir.join("src");
    fs::create_dir_all(&src_dir).expect("create ffhn-core src");
    fs::write(
        repo_root.join("Cargo.toml"),
        format!(
            "[workspace]\nmembers = [\"crates/ffhn-core\"]\nresolver = \"2\"\n\n[workspace.package]\nversion = \"{workspace_version}\"\nedition = \"2024\"\nlicense = \"MIT\"\ndescription = \"FFHN semver fixture\"\n"
        ),
    )
    .expect("write workspace Cargo.toml");
    fs::write(
        crate_dir.join("Cargo.toml"),
        "[package]\nname = \"ffhn-core\"\nversion.workspace = true\nedition.workspace = true\nlicense.workspace = true\ndescription.workspace = true\n",
    )
    .expect("write ffhn-core Cargo.toml");
    fs::write(src_dir.join("lib.rs"), lib_body).expect("write lib.rs");
}

fn run_git(repo_root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {:?} failed with {status}", args);
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
    assert!(
        plan.iter()
            .any(|spec| spec.args == ["audit", "-D", "warnings"])
    );
    assert!(plan.iter().any(|spec| {
        spec.args
            .windows(2)
            .any(|window| window == ["--release-type", "major"])
    }));
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
fn semver_scratch_dir_uses_target_tree() {
    let repo_root = tempdir().expect("tempdir");

    assert_eq!(
        semver_scratch_dir(repo_root.path()),
        repo_root.path().join("target").join("semver-checks")
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

    assert_eq!(tracked.len(), TRACKED_RELATIVE_PATHS.len());
    for relative_path in TRACKED_RELATIVE_PATHS {
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

#[test]
fn workspace_version_from_manifest_extracts_workspace_package_version() {
    let version = workspace_version_from_manifest(
        "[workspace.package]\nversion = \"2.0.0\"\nedition = \"2024\"\n",
    )
    .expect("workspace version");

    assert_eq!(version, "2.0.0");
}

#[test]
fn workspace_version_from_manifest_requires_a_version_line() {
    let error = workspace_version_from_manifest("[workspace.package]\nedition = \"2024\"\n")
        .expect_err("missing version should fail");

    assert_eq!(
        error.to_string(),
        "workspace version not found in Cargo.toml"
    );
}

#[test]
fn workspace_version_from_manifest_ignores_dependency_version_tables() {
    let manifest = r#"
[workspace.dependencies.htmlcut-core]
version = "2.0.0"

[workspace.package]
version = "2.0.0"
"#;

    let version = workspace_version_from_manifest(manifest).expect("workspace version");

    assert_eq!(version, "2.0.0");
}

#[test]
fn refresh_semver_baseline_uses_the_requested_git_ref_instead_of_the_worktree() {
    let repo_root = tempdir().expect("tempdir");
    write_semver_fixture(
        repo_root.path(),
        "2.0.0",
        "pub const RELEASE_LINE: &str = \"tagged\";\n",
    );
    run_git(repo_root.path(), &["init", "-q"]);
    run_git(repo_root.path(), &["config", "user.name", "FFHN Tests"]);
    run_git(
        repo_root.path(),
        &["config", "user.email", "ffhn@example.invalid"],
    );
    run_git(repo_root.path(), &["add", "Cargo.toml", "crates/ffhn-core"]);
    run_git(
        repo_root.path(),
        &["commit", "-qm", "seed published snapshot"],
    );
    run_git(repo_root.path(), &["tag", "v2.0.0"]);

    write_semver_fixture(
        repo_root.path(),
        "9.9.9",
        "pub const RELEASE_LINE: &str = \"worktree\";\n",
    );

    refresh_semver_baseline(repo_root.path(), "v2.0.0").expect("refresh baseline");

    let baseline_manifest = fs::read_to_string(
        repo_root
            .path()
            .join("semver-baseline")
            .join("ffhn-core")
            .join("Cargo.toml"),
    )
    .expect("read baseline manifest");
    let baseline_lib = fs::read_to_string(
        repo_root
            .path()
            .join("semver-baseline")
            .join("ffhn-core")
            .join("src")
            .join("lib.rs"),
    )
    .expect("read baseline lib");

    assert!(baseline_manifest.contains("version = \"2.0.0\""));
    assert!(!baseline_manifest.contains("version = \"9.9.9\""));
    assert_eq!(baseline_lib, "pub const RELEASE_LINE: &str = \"tagged\";\n");
}

#[test]
fn refresh_semver_baseline_replaces_existing_baseline_artifacts() {
    let repo_root = tempdir().expect("tempdir");
    write_semver_fixture(
        repo_root.path(),
        "2.0.0",
        "pub const RELEASE_LINE: &str = \"published\";\n",
    );
    run_git(repo_root.path(), &["init", "-q"]);
    run_git(repo_root.path(), &["config", "user.name", "FFHN Tests"]);
    run_git(
        repo_root.path(),
        &["config", "user.email", "ffhn@example.invalid"],
    );
    run_git(repo_root.path(), &["add", "Cargo.toml", "crates/ffhn-core"]);
    run_git(
        repo_root.path(),
        &["commit", "-qm", "seed published snapshot"],
    );
    run_git(repo_root.path(), &["tag", "v2.0.0"]);

    let baseline_parent = repo_root.path().join("semver-baseline");
    let baseline_dir = baseline_parent.join("ffhn-core");
    fs::create_dir_all(baseline_dir.join("src")).expect("create stale baseline dir");
    fs::write(baseline_dir.join("src").join("lib.rs"), "stale\n").expect("write stale baseline");
    fs::create_dir_all(&baseline_parent).expect("create baseline parent");
    fs::write(baseline_parent.join("ffhn-core.tar.gz"), "stale archive")
        .expect("write stale archive");

    refresh_semver_baseline(repo_root.path(), "v2.0.0").expect("refresh baseline");

    let baseline_lib = fs::read_to_string(baseline_dir.join("src").join("lib.rs"))
        .expect("read refreshed baseline");
    assert_eq!(
        baseline_lib,
        "pub const RELEASE_LINE: &str = \"published\";\n"
    );
    assert!(!baseline_parent.join("ffhn-core.tar.gz").exists());
}

#[test]
fn refresh_semver_baseline_reports_missing_git_refs() {
    let repo_root = tempdir().expect("tempdir");
    write_semver_fixture(
        repo_root.path(),
        "2.0.0",
        "pub const RELEASE_LINE: &str = \"published\";\n",
    );
    run_git(repo_root.path(), &["init", "-q"]);
    run_git(repo_root.path(), &["config", "user.name", "FFHN Tests"]);
    run_git(
        repo_root.path(),
        &["config", "user.email", "ffhn@example.invalid"],
    );
    run_git(repo_root.path(), &["add", "Cargo.toml", "crates/ffhn-core"]);
    run_git(
        repo_root.path(),
        &["commit", "-qm", "seed published snapshot"],
    );

    let error = refresh_semver_baseline(repo_root.path(), "v9.9.9").expect_err("missing ref");
    assert!(
        error
            .to_string()
            .contains("failed to read Cargo.toml from git ref v9.9.9")
    );
}

#[test]
fn workspace_version_reads_from_repo_manifest() {
    let repo_root = tempdir().expect("tempdir");
    fs::write(
        repo_root.path().join("Cargo.toml"),
        "[workspace.package]\nversion = \"9.9.9\"\n",
    )
    .expect("write Cargo.toml");

    let version = workspace_version(repo_root.path()).expect("workspace version");

    assert_eq!(version, "9.9.9");
}

#[test]
fn semver_release_type_uses_major_until_the_current_version_has_a_release_tag() {
    let repo_root = tempdir().expect("tempdir");
    fs::write(
        repo_root.path().join("Cargo.toml"),
        "[workspace.package]\nversion = \"2.0.0\"\n",
    )
    .expect("write Cargo.toml");

    assert_eq!(semver_release_type_from_git_tag(false), "major");
    assert_eq!(semver_release_type_from_git_tag(true), "minor");
    assert!(!release_tag_exists(repo_root.path(), "2.0.0").expect("missing git repo"));
    assert_eq!(
        semver_release_type(repo_root.path()).expect("release type without tag"),
        "major"
    );

    let status = Command::new("git")
        .arg("-C")
        .arg(repo_root.path())
        .arg("init")
        .arg("-q")
        .status()
        .expect("git init");
    assert!(status.success());

    fs::write(repo_root.path().join("README.md"), "# ffhn\n").expect("write README");

    let status = Command::new("git")
        .arg("-C")
        .arg(repo_root.path())
        .args([
            "-c",
            "user.name=FFHN",
            "-c",
            "user.email=ffhn@example.com",
            "add",
            "README.md",
        ])
        .status()
        .expect("git add");
    assert!(status.success());

    let status = Command::new("git")
        .arg("-C")
        .arg(repo_root.path())
        .args([
            "-c",
            "user.name=FFHN",
            "-c",
            "user.email=ffhn@example.com",
            "commit",
            "-q",
            "-m",
            "init",
        ])
        .status()
        .expect("git commit");
    assert!(status.success());

    let status = Command::new("git")
        .arg("-C")
        .arg(repo_root.path())
        .arg("tag")
        .arg("-f")
        .arg("v2.0.0")
        .status()
        .expect("git tag");
    assert!(status.success());

    assert!(release_tag_exists(repo_root.path(), "2.0.0").expect("release tag"));
    assert!(!release_tag_exists(repo_root.path(), "2.0.1").expect("other tag"));
    assert_eq!(
        semver_release_type(repo_root.path()).expect("release type with tag"),
        "minor"
    );
}

#[test]
fn with_workspace_stub_appends_once() {
    let workspace_manifest = r#"
[workspace]
members = ["crates/ffhn-core"]

[workspace.package]
version = "2.0.0"
edition = "2024"

[workspace.dependencies]
serde = "1.0.228"

[workspace.lints.rust]
unsafe_code = "warn"
"#;
    let updated = with_workspace_stub("[package]\nname = \"ffhn-core\"\n", workspace_manifest)
        .expect("workspace stub");
    let unchanged = with_workspace_stub(
        "[package]\nname = \"ffhn-core\"\n\n[workspace]\n",
        workspace_manifest,
    )
    .expect("unchanged workspace stub");

    assert!(updated.contains("[workspace.package]"));
    assert!(updated.contains("version = \"2.0.0\""));
    assert!(updated.contains("edition = \"2024\""));
    assert!(updated.contains("[workspace.dependencies]"));
    assert!(updated.contains("serde = \"1.0.228\""));
    assert!(updated.contains("[workspace.lints.rust]"));
    assert!(updated.contains("unsafe_code = \"warn\""));
    assert_eq!(
        unchanged,
        "[package]\nname = \"ffhn-core\"\n\n[workspace]\n"
    );
}

#[test]
fn with_workspace_stub_skips_missing_workspace_inheritance_sections() {
    let workspace_manifest = r#"
[workspace]
members = ["crates/ffhn-core"]

[workspace.package]
version = "2.0.0"
edition = "2024"
"#;

    let updated = with_workspace_stub("[package]\nname = \"ffhn-core\"\n", workspace_manifest)
        .expect("workspace stub");

    assert!(updated.contains("[workspace.package]"));
    assert!(!updated.contains("[workspace.dependencies]"));
    assert!(!updated.contains("[workspace.lints"));
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
            "crates/ffhn-core/src/runtime/run.rs",
            "crates/ffhn-cli/src/execute.rs",
            "xtask/src/plan.rs",
            "xtask/src/coverage.rs",
        ],
    );

    let report = CoverageReport {
        data: vec![CoverageDataSet {
            files: vec![
                CoverageFile {
                    filename: repo_root.path().join("crates/ffhn-core/src/runtime/run.rs"),
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

fn seed_tracked_files(repo_root: &Path) -> BTreeMap<PathBuf, String> {
    for relative_path in TRACKED_RELATIVE_PATHS {
        let file_path = repo_root.join(relative_path);
        fs::create_dir_all(file_path.parent().expect("parent")).expect("create dir");
        fs::write(&file_path, "// tracked\n").expect("write tracked file");
    }

    tracked_files(repo_root).expect("tracked files")
}

fn tracked_subset(repo_root: &Path, relative_paths: &[&str]) -> BTreeMap<PathBuf, String> {
    for relative_path in relative_paths {
        let file_path = repo_root.join(relative_path);
        fs::create_dir_all(file_path.parent().expect("parent")).expect("create dir");
        fs::write(&file_path, "// tracked\n").expect("write tracked file");
    }

    relative_paths
        .iter()
        .map(|relative_path| {
            (
                normalize_path(repo_root, &repo_root.join(relative_path)).expect("path"),
                (*relative_path).to_owned(),
            )
        })
        .collect()
}

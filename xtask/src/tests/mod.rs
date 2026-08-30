use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use tempfile::tempdir;

use crate::app::refresh_semver_baseline;
use crate::coverage::{
    coverage_clean_command, coverage_command, coverage_output_path, evaluate_coverage_report,
    read_coverage_report, tracked_files,
};
use crate::hygiene::{
    HygieneCleanMode, HygieneEntry, aggregate_entry_for_tests, clean_hygiene,
    dir_size_bytes_excluding_roots_for_tests, dir_size_bytes_for_tests,
    dir_size_bytes_result_for_tests, ensure_hygiene, format_bytes_for_tests, hygiene_report,
    looks_like_cargo_target_dir_for_tests, missing_managed_markers_for_entry_for_tests,
    missing_managed_markers_for_tests, prepare_artifact_layout, prepare_mutation_report_root,
    render_hygiene_report, report_violations_for_tests,
};
#[cfg(unix)]
use crate::hygiene::{entry_from_path_for_tests, repo_tmp_cargo_roots_for_tests};
use crate::model::{
    CommandArtifactLayout, CommandSpec, CoverageCounter, CoverageDataSet, CoverageFile,
    CoverageFileSummary, CoverageReport,
};
use crate::plan::{
    binary_name, binary_name_for_windows_for_tests, cargo_build_root, cargo_build_root_for_tests,
    cargo_target_root, cargo_target_root_for_tests, check_plan, collect_shell_script_paths,
    core_manifest_path, coverage_build_root, coverage_build_root_for_tests,
    coverage_cargo_build_dir, coverage_cargo_build_dir_for_tests, coverage_cargo_target_dir,
    coverage_cargo_target_dir_for_tests, coverage_target_root, coverage_target_root_for_tests,
    fuzz_lockfile_path, fuzz_manifest_path, is_semver_check_spec, mutation_report_root,
    mutation_report_root_for_tests, normalize_path, release_binary_path_for_tests,
    release_tag_exists, semver_baseline_path, semver_baseline_target_dir, semver_build_dir,
    semver_release_type, semver_release_type_from_git_tag, semver_scratch_dir,
    semver_scratch_dir_for_tests, shell_script_paths, sibling_artifact_dir_for_tests,
    with_cargo_artifact_root_overrides, with_workspace_stub, workspace_version,
    workspace_version_from_manifest,
};
use crate::tooling::{RustTooling, parse_rust_tooling};

pub(crate) static PROCESS_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn with_test_artifact_roots<T>(repo_root: &Path, operation: impl FnOnce() -> T) -> T {
    with_cargo_artifact_root_overrides(
        repo_root.join(".managed-artifacts").join("target"),
        repo_root.join(".managed-artifacts").join("build"),
        operation,
    )
}

fn write_repo_scaffold(repo_root: &Path) {
    fs::create_dir_all(repo_root.join("tooling")).expect("create tooling dir");
    fs::write(
        repo_root.join("Cargo.toml"),
        "[workspace.package]\nversion = \"2.0.0\"\n",
    )
    .expect("write Cargo.toml");
    fs::write(repo_root.join("Cargo.lock"), "version = 4\n").expect("write Cargo.lock");
    fs::create_dir_all(repo_root.join("fuzz")).expect("create fuzz dir");
    fs::write(
        repo_root.join("fuzz/Cargo.toml"),
        "[package]\nname = \"fixture-fuzz\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n",
    )
    .expect("write fuzz Cargo.toml");
    fs::write(repo_root.join("fuzz/Cargo.lock"), "version = 4\n").expect("write fuzz Cargo.lock");
    fs::write(repo_root.join("changelog.md"), "## [Unreleased]\n").expect("write changelog.md");
    fs::write(
        repo_root.join("tooling/rust-tooling.env"),
        "RUST_WORKSPACE_EDITION=2024\n\
RUST_WORKSPACE_RUST_VERSION=1.98\n\
RUST_STABLE_TOOLCHAIN=1.98.0\n\
RUST_QA_NIGHTLY_TOOLCHAIN=nightly-2026-08-25\n\
\n\
CARGO_AUDIT_VERSION=0.22.2\n\
CARGO_DENY_VERSION=0.20.2\n\
CARGO_FUZZ_VERSION=0.13.2\n\
CARGO_LLVM_COV_VERSION=0.9.0\n\
CARGO_MUTANTS_VERSION=27.1.0\n\
CARGO_NEXTEST_VERSION=0.9.143\n\
CARGO_OUTDATED_VERSION=0.19.0\n\
CARGO_SEMVER_CHECKS_VERSION=0.50.0\n",
    )
    .expect("write rust tooling env");
    fs::write(
        repo_root.join("tooling/rust-source-shape-policy.toml"),
        "version = 1\n\
\n\
[[rules]]\n\
path = \"crates/ffhn-core/src/\"\n\
match = \"prefix\"\n\
role = \"fixture\"\n\
owner = \"fixture\"\n\
rationale = \"fixture\"\n\
split_trigger = \"fixture\"\n\
max_physical_lines = 10\n\
max_items = 10\n\
max_public_items = 10\n\
max_imports = 10\n\
max_functions = 10\n\
max_decision_points = 10\n\
max_match_arms = 10\n\
\n\
[[rules]]\n\
path = \"crates/ffhn-cli/src/\"\n\
match = \"prefix\"\n\
role = \"fixture\"\n\
owner = \"fixture\"\n\
rationale = \"fixture\"\n\
split_trigger = \"fixture\"\n\
max_physical_lines = 10\n\
max_items = 10\n\
max_public_items = 10\n\
max_imports = 10\n\
max_functions = 10\n\
max_decision_points = 10\n\
max_match_arms = 10\n\
\n\
[[rules]]\n\
path = \"xtask/src/\"\n\
match = \"prefix\"\n\
role = \"fixture\"\n\
owner = \"fixture\"\n\
rationale = \"fixture\"\n\
split_trigger = \"fixture\"\n\
max_physical_lines = 10\n\
max_items = 10\n\
max_public_items = 10\n\
max_imports = 10\n\
max_functions = 10\n\
max_decision_points = 10\n\
max_match_arms = 10\n",
    )
    .expect("write Rust source-shape policy");
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

fn workspace_package_field(repo_root: &Path, field: &str) -> Option<String> {
    let manifest = fs::read_to_string(repo_root.join("Cargo.toml")).ok()?;
    let mut in_workspace_package = false;

    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_workspace_package = trimmed == "[workspace.package]";
            continue;
        }

        if !in_workspace_package {
            continue;
        }

        if let Some(value) = trimmed.strip_prefix(&format!("{field} = \""))
            && let Some(value) = value.strip_suffix('"')
        {
            return Some(value.to_owned());
        }
    }

    None
}

fn sample_tooling() -> RustTooling {
    parse_rust_tooling(
        "RUST_WORKSPACE_EDITION=2024\n\
RUST_WORKSPACE_RUST_VERSION=1.98\n\
RUST_STABLE_TOOLCHAIN=1.98.0\n\
RUST_QA_NIGHTLY_TOOLCHAIN=nightly-2026-08-25\n\
\n\
CARGO_AUDIT_VERSION=0.22.2\n\
CARGO_DENY_VERSION=0.20.2\n\
CARGO_FUZZ_VERSION=0.13.2\n\
CARGO_LLVM_COV_VERSION=0.9.0\n\
CARGO_MUTANTS_VERSION=27.1.0\n\
CARGO_NEXTEST_VERSION=0.9.143\n\
CARGO_OUTDATED_VERSION=0.19.0\n\
CARGO_SEMVER_CHECKS_VERSION=0.50.0\n",
    )
    .expect("parse sample tooling")
}

#[test]
fn parse_rust_tooling_rejects_invalid_and_empty_assignments() {
    assert_eq!(
        parse_rust_tooling("RUST_WORKSPACE_EDITION").expect_err("missing assignment"),
        "invalid tooling line: RUST_WORKSPACE_EDITION"
    );
    assert_eq!(
        parse_rust_tooling("RUST_WORKSPACE_EDITION=").expect_err("empty value"),
        "invalid tooling line: RUST_WORKSPACE_EDITION="
    );
    assert_eq!(
        parse_rust_tooling(" = 2024").expect_err("empty key"),
        "invalid tooling line: = 2024"
    );
    assert_eq!(
        parse_rust_tooling("RUST_WORKSPACE_EDITION=2024\n").expect_err("missing required keys"),
        "missing RUST_WORKSPACE_RUST_VERSION"
    );

    let tooling = parse_rust_tooling(
        "# pinned toolchain metadata\n\
RUST_WORKSPACE_EDITION=2024\n\
RUST_WORKSPACE_RUST_VERSION=1.98\n\
RUST_STABLE_TOOLCHAIN=1.98.0\n\
RUST_QA_NIGHTLY_TOOLCHAIN=nightly-2026-08-25\n\
CARGO_AUDIT_VERSION=0.22.2\n\
CARGO_DENY_VERSION=0.20.2\n\
CARGO_FUZZ_VERSION=0.13.2\n\
CARGO_LLVM_COV_VERSION=0.9.0\n\
CARGO_MUTANTS_VERSION=27.1.0\n\
CARGO_NEXTEST_VERSION=0.9.143\n\
CARGO_OUTDATED_VERSION=0.19.0\n\
CARGO_SEMVER_CHECKS_VERSION=0.50.0\n",
    )
    .expect("commented tooling manifest");
    assert_eq!(tooling.workspace_edition, "2024");
}

#[test]
fn parse_rust_tooling_requires_the_semver_check_version() {
    let error = parse_rust_tooling(
        "RUST_WORKSPACE_EDITION=2024\n\
RUST_WORKSPACE_RUST_VERSION=1.98\n\
RUST_STABLE_TOOLCHAIN=1.98.0\n\
RUST_QA_NIGHTLY_TOOLCHAIN=nightly-2026-08-25\n\
CARGO_AUDIT_VERSION=0.22.2\n\
CARGO_DENY_VERSION=0.20.2\n\
CARGO_FUZZ_VERSION=0.13.2\n\
CARGO_LLVM_COV_VERSION=0.9.0\n\
CARGO_MUTANTS_VERSION=27.1.0\n\
CARGO_NEXTEST_VERSION=0.9.143\n\
CARGO_OUTDATED_VERSION=0.19.0\n",
    )
    .expect_err("missing semver-checks version must be rejected");

    assert_eq!(error, "missing CARGO_SEMVER_CHECKS_VERSION");
}

fn run_git(repo_root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {:?} failed with {status}", args);
}

mod app;
mod audit;
mod automation;
mod command;
mod coverage;
mod devcontainer;
mod hygiene;
mod mutants;
mod plan;
mod release;
mod semver;

#[cfg(unix)]
const SEEDED_TRACKED_FILES: &[&str] = &[
    "crates/ffhn-core/src/model/report/run.rs",
    "crates/ffhn-cli/src/execute.rs",
    "xtask/src/coverage.rs",
    "xtask/src/plan/check.rs",
];

fn tracked_source() -> String {
    let mut source = String::from("fn tracked() {\n    let _ = 1;\n}\n");
    for _ in 0..80 {
        source.push('\n');
    }
    source
}

#[cfg(unix)]
fn seed_tracked_files(repo_root: &Path) -> BTreeMap<PathBuf, String> {
    for relative_path in SEEDED_TRACKED_FILES {
        let file_path = repo_root.join(relative_path);
        fs::create_dir_all(file_path.parent().expect("parent")).expect("create dir");
        fs::write(&file_path, tracked_source()).expect("write tracked file");
    }

    tracked_files(repo_root).expect("tracked files")
}

fn tracked_subset(repo_root: &Path, relative_paths: &[&str]) -> BTreeMap<PathBuf, String> {
    for relative_path in relative_paths {
        let file_path = repo_root.join(relative_path);
        fs::create_dir_all(file_path.parent().expect("parent")).expect("create dir");
        fs::write(&file_path, tracked_source()).expect("write tracked file");
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

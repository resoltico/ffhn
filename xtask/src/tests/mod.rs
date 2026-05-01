use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use tempfile::tempdir;

use crate::app::refresh_semver_baseline;
use crate::coverage::{
    coverage_clean_command, coverage_command, evaluate_coverage_report, read_coverage_report,
    tracked_files,
};
use crate::model::{
    CommandSpec, CoverageCounter, CoverageDataSet, CoverageFile, CoverageFileSummary,
    CoverageReport,
};
use crate::plan::{
    binary_name, cargo_target_root, check_plan, collect_shell_script_paths, is_semver_check_spec,
    normalize_path, release_binary_path, release_tag_exists, semver_baseline_target_dir,
    semver_release_type, semver_release_type_from_git_tag, semver_scratch_dir, shell_script_paths,
    with_workspace_stub, workspace_version, workspace_version_from_manifest,
};

static CARGO_TARGET_DIR_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[allow(unsafe_code)]
fn with_cargo_target_dir<T>(value: Option<&Path>, operation: impl FnOnce() -> T) -> T {
    let _guard = CARGO_TARGET_DIR_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("cargo target dir lock");
    let original = env::var_os("CARGO_TARGET_DIR");

    match value {
        Some(path) => {
            // SAFETY: environment mutation is process-global and not thread-safe. This helper
            // serializes every CARGO_TARGET_DIR mutation behind CARGO_TARGET_DIR_LOCK.
            unsafe { env::set_var("CARGO_TARGET_DIR", path) };
        }
        None => {
            // SAFETY: environment mutation is process-global and not thread-safe. This helper
            // serializes every CARGO_TARGET_DIR mutation behind CARGO_TARGET_DIR_LOCK.
            unsafe { env::remove_var("CARGO_TARGET_DIR") };
        }
    }

    let result = operation();

    match original {
        Some(target_dir) => {
            // SAFETY: same invariant as above; the helper still holds CARGO_TARGET_DIR_LOCK for
            // the full mutation window.
            unsafe { env::set_var("CARGO_TARGET_DIR", target_dir) };
        }
        None => {
            // SAFETY: same invariant as above; the helper still holds CARGO_TARGET_DIR_LOCK for
            // the full mutation window.
            unsafe { env::remove_var("CARGO_TARGET_DIR") };
        }
    }

    result
}

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

fn run_git(repo_root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {:?} failed with {status}", args);
}

mod app;
mod coverage;
mod devcontainer;
mod plan;
mod release;
mod semver;

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

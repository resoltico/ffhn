use std::env;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use super::*;
use crate::app::{
    TEST_REPO_ROOT_ENV, remove_dir_if_exists, remove_file_if_exists, repo_root, run_check,
    run_coverage, run_spec,
};
use crate::plan::{release_binary_path, semver_baseline_target_dir, semver_scratch_dir};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

static PATH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[test]
fn run_from_routes_cli_requests_through_the_library_entrypoint() {
    let error = crate::run_from([
        "xtask",
        "refresh-semver-baseline",
        "--git-ref",
        "definitely-missing-ref",
    ])
    .expect_err("missing git ref should fail");

    assert!(
        error
            .to_string()
            .contains("failed to read Cargo.toml from git ref definitely-missing-ref")
    );
}

#[cfg(unix)]
#[test]
fn run_spec_reports_failing_process_statuses() {
    let repo_root = tempdir().expect("tempdir");

    let error = run_spec(
        repo_root.path(),
        &CommandSpec::new("sh", ["-c", "exit 7"], false, false),
    )
    .expect_err("failing command should surface an error");

    assert!(error.to_string().contains("exit status: 7"));
}

#[cfg(unix)]
#[test]
fn run_spec_can_quiet_stdout_while_forcing_clang() {
    let repo_root = tempdir().expect("tempdir");

    run_spec(
        repo_root.path(),
        &CommandSpec::new(
            "sh",
            [
                "-c",
                "test \"$CC\" = clang && printf 'quiet-output' && exit 0",
            ],
            true,
            true,
        ),
    )
    .expect("quiet stdout clang run should succeed");
}

#[test]
fn remove_helpers_delete_existing_paths_and_ignore_missing_ones() {
    let repo_root = tempdir().expect("tempdir");
    let removable_dir = repo_root.path().join("target").join("scratch");
    let removable_file = repo_root.path().join("target").join("scratch.txt");
    fs::create_dir_all(&removable_dir).expect("create removable dir");
    fs::write(&removable_file, "scratch").expect("write removable file");

    remove_dir_if_exists(&removable_dir).expect("remove dir");
    remove_dir_if_exists(&removable_dir).expect("ignore missing dir");
    remove_file_if_exists(&removable_file).expect("remove file");
    remove_file_if_exists(&removable_file).expect("ignore missing file");

    assert!(!removable_dir.exists());
    assert!(!removable_file.exists());
}

#[test]
#[allow(unsafe_code)]
fn repo_root_falls_back_to_the_workspace_parent_without_a_test_override() {
    let _guard = PATH_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("path lock");
    let original_repo_root = env::var_os(TEST_REPO_ROOT_ENV);
    unsafe { env::remove_var(TEST_REPO_ROOT_ENV) };

    let resolved = repo_root().expect("repo root");
    assert_eq!(
        resolved,
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace parent")
    );

    match original_repo_root {
        Some(repo_root) => unsafe { env::set_var(TEST_REPO_ROOT_ENV, repo_root) },
        None => unsafe { env::remove_var(TEST_REPO_ROOT_ENV) },
    }
}

#[cfg(unix)]
#[test]
fn run_check_executes_the_plan_and_cleans_semver_artifacts() {
    let repo_root = tempdir().expect("tempdir");
    let bin_dir = repo_root.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_repo_scaffold(repo_root.path());
    write_tracked_barrels(repo_root.path());
    fs::create_dir_all(semver_scratch_dir(repo_root.path())).expect("create semver scratch");
    fs::create_dir_all(semver_baseline_target_dir(repo_root.path()))
        .expect("create semver baseline target");
    write_release_binary(repo_root.path());
    write_coverage_cargo_stub(&bin_dir, "{\"data\":[]}");

    with_test_environment(&bin_dir, Some(repo_root.path()), || {
        run_check(repo_root.path()).expect("run check");
    });

    assert!(!semver_scratch_dir(repo_root.path()).exists());
    assert!(!semver_baseline_target_dir(repo_root.path()).exists());
}

#[cfg(unix)]
#[test]
fn run_coverage_fails_when_tracked_lines_or_branches_are_missing() {
    let repo_root = tempdir().expect("tempdir");
    let bin_dir = repo_root.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_repo_scaffold(repo_root.path());
    write_tracked_source(repo_root.path(), "xtask/src/app/check.rs", tracked_source());
    write_coverage_cargo_stub(
        &bin_dir,
        "{\"data\":[{\"files\":[{\"filename\":\"REPO_ROOT/xtask/src/app/check.rs\",\"segments\":[[1,1,0,false,true,false],[2,1,0,false,false,false]],\"branches\":[],\"summary\":{\"branches\":{\"count\":1,\"covered\":0,\"notcovered\":1}}}]}]}",
    );

    let error = with_test_environment(&bin_dir, None, || {
        run_coverage(repo_root.path()).expect_err("coverage failure should surface")
    });

    assert_eq!(error.to_string(), "coverage gate failed");
}

#[cfg(unix)]
#[test]
fn run_coverage_reports_branch_only_failures() {
    let repo_root = tempdir().expect("tempdir");
    let bin_dir = repo_root.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_repo_scaffold(repo_root.path());
    write_tracked_source(repo_root.path(), "xtask/src/app/check.rs", tracked_source());
    write_coverage_cargo_stub(
        &bin_dir,
        "{\"data\":[{\"files\":[{\"filename\":\"REPO_ROOT/xtask/src/app/check.rs\",\"segments\":[[1,1,1,false,true,false],[2,1,0,false,false,false]],\"branches\":[],\"summary\":{\"branches\":{\"count\":1,\"covered\":0,\"notcovered\":1}}}]}]}",
    );

    let error = with_test_environment(&bin_dir, None, || {
        run_coverage(repo_root.path()).expect_err("branch-only failure should surface")
    });

    assert_eq!(error.to_string(), "coverage gate failed");
}

#[cfg(unix)]
#[test]
fn run_coverage_reports_line_only_failures() {
    let repo_root = tempdir().expect("tempdir");
    let bin_dir = repo_root.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_repo_scaffold(repo_root.path());
    write_tracked_source(repo_root.path(), "xtask/src/app/check.rs", tracked_source());
    write_coverage_cargo_stub(
        &bin_dir,
        "{\"data\":[{\"files\":[{\"filename\":\"REPO_ROOT/xtask/src/app/check.rs\",\"segments\":[[1,1,0,false,true,false],[2,1,0,false,false,false]],\"branches\":[],\"summary\":{\"branches\":{\"count\":0,\"covered\":0,\"notcovered\":0}}}]}]}",
    );

    let error = with_test_environment(&bin_dir, None, || {
        run_coverage(repo_root.path()).expect_err("line-only failure should surface")
    });

    assert_eq!(error.to_string(), "coverage gate failed");
}

#[cfg(unix)]
#[test]
fn run_from_routes_check_and_coverage_subcommands_through_the_repo_root_override() {
    let repo_root = tempdir().expect("tempdir");
    let bin_dir = repo_root.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_repo_scaffold(repo_root.path());
    write_tracked_barrels(repo_root.path());
    write_release_binary(repo_root.path());
    write_coverage_cargo_stub(&bin_dir, "{\"data\":[]}");

    with_test_environment(&bin_dir, Some(repo_root.path()), || {
        crate::run_from(["xtask", "check"]).expect("run check through cli");
        crate::run_from(["xtask", "coverage"]).expect("run coverage through cli");
    });
}

fn write_tracked_barrels(repo_root: &Path) {
    write_tracked_source(repo_root, "crates/ffhn-core/src/lib.rs", "mod report;\n");
    write_tracked_source(repo_root, "crates/ffhn-cli/src/lib.rs", "mod help;\n");
    write_tracked_source(repo_root, "xtask/src/lib.rs", "mod app;\n");
}

fn write_tracked_source(repo_root: &Path, relative_path: &str, source: impl AsRef<str>) {
    let file_path = repo_root.join(relative_path);
    fs::create_dir_all(file_path.parent().expect("parent")).expect("create source dir");
    fs::write(file_path, source.as_ref()).expect("write source file");
}

#[cfg(unix)]
fn write_executable(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("create executable parent");
    fs::write(path, contents).expect("write executable");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

#[cfg(unix)]
fn write_release_binary(repo_root: &Path) {
    write_executable(&release_binary_path(repo_root), "#!/bin/sh\nexit 0\n");
}

#[cfg(unix)]
fn write_coverage_cargo_stub(bin_dir: &Path, coverage_json: &str) {
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"+nightly\" ] && [ \"$2\" = \"llvm-cov\" ] && [ \"$3\" = \"--branch\" ]; then\n  while [ \"$#\" -gt 0 ]; do\n    if [ \"$1\" = \"--output-path\" ]; then\n      shift\n      output_path=\"$1\"\n      break\n    fi\n    shift\n  done\n  mkdir -p \"$(dirname \"$output_path\")\"\n  printf '%s' '{}' | sed \"s|REPO_ROOT|$PWD|g\" > \"$output_path\"\nfi\nexit 0\n",
        coverage_json.replace('\'', "'\"'\"'")
    );
    write_executable(&bin_dir.join("cargo"), &script);
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn with_test_environment<T>(
    bin_dir: &Path,
    repo_root_override: Option<&Path>,
    operation: impl FnOnce() -> T,
) -> T {
    let _guard = PATH_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("path lock");
    let original_path = env::var_os("PATH").unwrap_or_default();
    let original_repo_root = env::var_os(TEST_REPO_ROOT_ENV);
    let mut updated_path = std::ffi::OsString::from(bin_dir);
    updated_path.push(":");
    updated_path.push(&original_path);
    // SAFETY: tests serialize PATH mutation through PATH_LOCK.
    unsafe { env::set_var("PATH", &updated_path) };
    if let Some(repo_root_override) = repo_root_override {
        // SAFETY: tests serialize environment mutation through PATH_LOCK.
        unsafe { env::set_var(TEST_REPO_ROOT_ENV, repo_root_override) };
    }
    let result = operation();
    // SAFETY: tests serialize PATH mutation through PATH_LOCK.
    unsafe { env::set_var("PATH", original_path) };
    match original_repo_root {
        Some(repo_root) => {
            // SAFETY: tests serialize environment mutation through PATH_LOCK.
            unsafe { env::set_var(TEST_REPO_ROOT_ENV, repo_root) };
        }
        None => {
            // SAFETY: tests serialize environment mutation through PATH_LOCK.
            unsafe { env::remove_var(TEST_REPO_ROOT_ENV) };
        }
    }
    result
}

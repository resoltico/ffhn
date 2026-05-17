use super::*;
use crate::app::{TEST_REPO_ROOT_ENV, remove_dir_if_exists, remove_file_if_exists, repo_root};
use crate::tests::PROCESS_ENV_LOCK;
use std::env;
use std::fs;
use std::path::Path;

#[cfg(unix)]
use crate::app::{run_audit, run_check, run_coverage, run_semver_check, run_spec};
#[cfg(unix)]
use crate::plan::{release_binary_path, semver_baseline_target_dir, semver_scratch_dir};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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
        &CommandSpec::new("sh", ["-c", "exit 7"], false),
    )
    .expect_err("failing command should surface an error");

    assert!(error.to_string().contains("exit status: 7"));
}

#[cfg(unix)]
#[test]
fn run_spec_can_quiet_stdout_while_passing_explicit_env_overrides() {
    let repo_root = tempdir().expect("tempdir");

    run_spec(
        repo_root.path(),
        &CommandSpec::new(
            "sh",
            [
                "-c",
                "test \"$FFHN_TEST_ENV\" = ready && printf 'quiet-output' && exit 0",
            ],
            true,
        )
        .with_envs([("FFHN_TEST_ENV", "ready")]),
    )
    .expect("quiet stdout env run should succeed");
}

#[cfg(unix)]
#[test]
#[allow(unsafe_code)]
fn run_spec_scrubs_ambient_native_toolchain_overrides_unless_explicitly_requested() {
    let _guard = PROCESS_ENV_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let repo_root = tempdir().expect("tempdir");
    let original_cc = env::var_os("CC");
    let original_cxx = env::var_os("CXX");
    let original_clang_bin = env::var_os("CLANG_BIN");
    let original_cppflags = env::var_os("CPPFLAGS");
    let original_ldflags = env::var_os("LDFLAGS");

    // SAFETY: PROCESS_ENV_LOCK serializes all process-environment mutation in this test module.
    unsafe {
        env::set_var("CC", "/broken/clang");
        env::set_var("CXX", "/broken/clang++");
        env::set_var("CLANG_BIN", "/broken/clang");
        env::set_var("CPPFLAGS", "-I/broken/include");
        env::set_var("LDFLAGS", "-L/broken/lib");
    }

    let result = run_spec(
        repo_root.path(),
        &CommandSpec::new(
            "sh",
            [
                "-c",
                "test -z \"$CC\" && test -z \"$CXX\" && test -z \"$CLANG_BIN\" && test -z \"$CPPFLAGS\" && test -z \"$LDFLAGS\"",
            ],
            false,
        ),
    );

    match original_cc {
        Some(value) => unsafe { env::set_var("CC", value) },
        None => unsafe { env::remove_var("CC") },
    }
    match original_cxx {
        Some(value) => unsafe { env::set_var("CXX", value) },
        None => unsafe { env::remove_var("CXX") },
    }
    match original_clang_bin {
        Some(value) => unsafe { env::set_var("CLANG_BIN", value) },
        None => unsafe { env::remove_var("CLANG_BIN") },
    }
    match original_cppflags {
        Some(value) => unsafe { env::set_var("CPPFLAGS", value) },
        None => unsafe { env::remove_var("CPPFLAGS") },
    }
    match original_ldflags {
        Some(value) => unsafe { env::set_var("LDFLAGS", value) },
        None => unsafe { env::remove_var("LDFLAGS") },
    }

    result.expect("ambient compiler overrides should be scrubbed");
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
    let _guard = PROCESS_ENV_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let original_repo_root = env::var_os(TEST_REPO_ROOT_ENV);
    // SAFETY: environment mutation is process-global and not thread-safe. This test holds
    // PROCESS_ENV_LOCK for the entire mutation window, and every test in this process that reads or
    // writes TEST_REPO_ROOT_ENV or PATH must also acquire PROCESS_ENV_LOCK first. Adding a PATH- or
    // TEST_REPO_ROOT_ENV-dependent test without the lock breaks that invariant and risks a data
    // race through the shared process environment.
    unsafe { env::remove_var(TEST_REPO_ROOT_ENV) };

    let resolved = repo_root().expect("repo root");
    assert_eq!(
        resolved,
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace parent")
    );

    match original_repo_root {
        Some(repo_root) => {
            // SAFETY: same invariant as above; PROCESS_ENV_LOCK still serializes all environment access
            // for tests that touch TEST_REPO_ROOT_ENV or PATH.
            unsafe { env::set_var(TEST_REPO_ROOT_ENV, repo_root) }
        }
        None => {
            // SAFETY: same invariant as above; PROCESS_ENV_LOCK still serializes all environment access
            // for tests that touch TEST_REPO_ROOT_ENV or PATH.
            unsafe { env::remove_var(TEST_REPO_ROOT_ENV) }
        }
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
fn run_from_routes_check_semver_coverage_and_miri_subcommands_through_the_repo_root_override() {
    let repo_root = tempdir().expect("tempdir");
    let bin_dir = repo_root.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_repo_scaffold(repo_root.path());
    write_tracked_barrels(repo_root.path());
    write_release_binary(repo_root.path());
    write_coverage_cargo_stub(&bin_dir, "{\"data\":[]}");

    with_test_environment(&bin_dir, Some(repo_root.path()), || {
        crate::run_from(["xtask", "check"]).expect("run check through cli");
        crate::run_from(["xtask", "audit"]).expect("run audit through cli");
        crate::run_from(["xtask", "audit", "--file", "fuzz/Cargo.lock"])
            .expect("run audit --file through cli");
        crate::run_from(["xtask", "semver-check"]).expect("run semver-check through cli");
        crate::run_from(["xtask", "coverage"]).expect("run coverage through cli");
        crate::run_from(["xtask", "miri"]).expect("run miri through cli");
    });
}

#[cfg(unix)]
#[test]
fn run_audit_retries_transient_advisory_fetch_failures() {
    let repo_root = tempdir().expect("tempdir");
    let bin_dir = repo_root.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_repo_scaffold(repo_root.path());
    let attempts_file = repo_root.path().join("audit-attempts.txt");
    let tooling = sample_tooling();
    let script = format!(
        "#!/bin/sh\nattempts_file=\"{attempts_file}\"\nif [ \"$2\" = \"--version\" ] && [ \"$1\" = \"audit\" ]; then\n  printf 'cargo-audit {version}\\n'\n  exit 0\nfi\nif [ \"$1\" = \"audit\" ]; then\n  if [ ! -f \"$attempts_file\" ]; then\n    printf '1' > \"$attempts_file\"\n    printf \"error: couldn't fetch advisory database\\n\" >&2\n    printf \"Caused by:\\n  -> An IO error occurred when talking to the server\\n\" >&2\n    exit 1\n  fi\n  printf '2' > \"$attempts_file\"\n  exit 0\nfi\nexit 0\n",
        attempts_file = attempts_file.display(),
        version = tooling.cargo_audit_version,
    );
    write_executable(&bin_dir.join("cargo"), &script);

    with_test_environment(&bin_dir, Some(repo_root.path()), || {
        run_audit(repo_root.path(), None).expect("transient audit failure should retry");
    });

    assert_eq!(
        fs::read_to_string(attempts_file).expect("attempts file"),
        "2"
    );
}

#[cfg(unix)]
#[test]
fn run_audit_does_not_retry_non_transient_failures() {
    let repo_root = tempdir().expect("tempdir");
    let bin_dir = repo_root.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_repo_scaffold(repo_root.path());
    let attempts_file = repo_root.path().join("audit-attempts.txt");
    let tooling = sample_tooling();
    let script = format!(
        "#!/bin/sh\nattempts_file=\"{attempts_file}\"\nif [ \"$2\" = \"--version\" ] && [ \"$1\" = \"audit\" ]; then\n  printf 'cargo-audit {version}\\n'\n  exit 0\nfi\nif [ \"$1\" = \"audit\" ]; then\n  count=0\n  if [ -f \"$attempts_file\" ]; then\n    count=$(cat \"$attempts_file\")\n  fi\n  count=$((count + 1))\n  printf '%s' \"$count\" > \"$attempts_file\"\n  printf 'found vulnerable crate\\n' >&2\n  exit 1\nfi\nexit 0\n",
        attempts_file = attempts_file.display(),
        version = tooling.cargo_audit_version,
    );
    write_executable(&bin_dir.join("cargo"), &script);

    let error = with_test_environment(&bin_dir, Some(repo_root.path()), || {
        run_audit(repo_root.path(), None).expect_err("deterministic audit failure should surface")
    });

    assert!(error.to_string().contains("command failed with status"));
    assert_eq!(
        fs::read_to_string(attempts_file).expect("attempts file"),
        "1"
    );
}

#[cfg(unix)]
#[test]
fn run_audit_stops_after_the_final_transient_fetch_failure() {
    let repo_root = tempdir().expect("tempdir");
    let bin_dir = repo_root.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_repo_scaffold(repo_root.path());
    let attempts_file = repo_root.path().join("audit-attempts.txt");
    let tooling = sample_tooling();
    let script = format!(
        "#!/bin/sh\nattempts_file=\"{attempts_file}\"\nif [ \"$2\" = \"--version\" ] && [ \"$1\" = \"audit\" ]; then\n  printf 'cargo-audit {version}\\n'\n  exit 0\nfi\nif [ \"$1\" = \"audit\" ]; then\n  count=0\n  if [ -f \"$attempts_file\" ]; then\n    count=$(cat \"$attempts_file\")\n  fi\n  count=$((count + 1))\n  printf '%s' \"$count\" > \"$attempts_file\"\n  printf \"error: couldn't fetch advisory database\\n\" >&2\n  printf \"Caused by:\\n  -> An IO error occurred when talking to the server\\n\" >&2\n  exit 1\nfi\nexit 0\n",
        attempts_file = attempts_file.display(),
        version = tooling.cargo_audit_version,
    );
    write_executable(&bin_dir.join("cargo"), &script);

    let error = with_test_environment(&bin_dir, Some(repo_root.path()), || {
        run_audit(repo_root.path(), None)
            .expect_err("final transient audit failure should surface after the retry budget")
    });

    assert!(error.to_string().contains("command failed with status"));
    assert_eq!(
        fs::read_to_string(attempts_file).expect("attempts file"),
        "3"
    );
}

#[cfg(unix)]
#[test]
fn run_from_routes_hygiene_subcommands_through_the_repo_root_override() {
    let repo_root = tempdir().expect("tempdir");
    let bin_dir = repo_root.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    fs::create_dir_all(repo_root.path().join("tmp").join("probe")).expect("create repo tmp root");

    with_test_artifact_roots(repo_root.path(), || {
        with_test_environment(&bin_dir, Some(repo_root.path()), || {
            crate::run_from(["xtask", "hygiene", "report"]).expect("run hygiene report");
            crate::run_from(["xtask", "hygiene", "report", "--format", "json"])
                .expect("run hygiene json report");
            crate::run_from(["xtask", "hygiene", "clean", "--mode", "safe"])
                .expect("run hygiene clean");
            crate::run_from(["xtask", "hygiene", "verify"]).expect("run hygiene verify");
        });
    });
}

#[cfg(unix)]
#[test]
fn run_semver_check_executes_only_the_semver_lane_and_cleans_artifacts() {
    let repo_root = tempdir().expect("tempdir");
    let bin_dir = repo_root.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_repo_scaffold(repo_root.path());
    fs::create_dir_all(semver_scratch_dir(repo_root.path())).expect("create semver scratch");
    fs::create_dir_all(semver_baseline_target_dir(repo_root.path()))
        .expect("create semver baseline target");
    write_coverage_cargo_stub(&bin_dir, "{\"data\":[]}");

    with_test_environment(&bin_dir, Some(repo_root.path()), || {
        run_semver_check(repo_root.path()).expect("run semver-only lane");
    });

    assert!(!semver_scratch_dir(repo_root.path()).exists());
    assert!(!semver_baseline_target_dir(repo_root.path()).exists());
}

#[cfg(unix)]
#[test]
fn run_semver_check_prepares_the_isolated_target_tree_before_launch() {
    let repo_root = tempdir().expect("tempdir");
    let bin_dir = repo_root.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_repo_scaffold(repo_root.path());
    write_executable(
        &bin_dir.join("cargo"),
        "#!/bin/sh\nif [ \"$2\" = \"--version\" ]; then\n  case \"$1\" in\n    semver-checks) printf 'cargo-semver-checks 0.47.0\\n' ; exit 0 ;;\n    audit) printf 'cargo-audit 0.22.1\\n' ; exit 0 ;;\n    deny) printf 'cargo-deny 0.19.4\\n' ; exit 0 ;;\n    nextest) printf 'cargo-nextest 0.9.133\\n' ; exit 0 ;;\n  esac\nfi\nif [ \"$1\" = \"semver-checks\" ]; then\n  test -d \"$CARGO_TARGET_DIR\"\n  test -d \"$CARGO_TARGET_DIR/debug\"\n  test -d \"$CARGO_TARGET_DIR/debug/deps\"\nfi\nexit 0\n",
    );

    with_test_environment(&bin_dir, Some(repo_root.path()), || {
        run_semver_check(repo_root.path()).expect("run semver-only lane");
    });

    assert!(!semver_scratch_dir(repo_root.path()).exists());
}

#[cfg(unix)]
#[test]
fn run_spec_preserves_explicit_artifact_env_overrides() {
    let repo_root = tempdir().expect("tempdir");

    run_spec(
        repo_root.path(),
        &CommandSpec::new(
            "sh",
            [
                "-c",
                "test \"$CARGO_TARGET_DIR\" = explicit-target && test \"$CARGO_BUILD_BUILD_DIR\" = explicit-build",
            ],
            false,
        )
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace)
        .with_envs([
            ("CARGO_TARGET_DIR", "explicit-target"),
            ("CARGO_BUILD_BUILD_DIR", "explicit-build"),
        ]),
    )
    .expect("explicit artifact layout envs should win");
}

#[cfg(unix)]
fn write_tracked_barrels(repo_root: &Path) {
    write_tracked_source(repo_root, "crates/ffhn-core/src/lib.rs", "mod report;\n");
    write_tracked_source(repo_root, "crates/ffhn-cli/src/lib.rs", "mod help;\n");
    write_tracked_source(repo_root, "xtask/src/lib.rs", "mod app;\n");
}

#[cfg(unix)]
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
    let tooling = sample_tooling();
    let script = format!(
        "#!/bin/sh\nif [ \"$2\" = \"--version\" ]; then\n  case \"$1\" in\n    audit) printf 'cargo-audit %s\\n' \"{cargo_audit_version}\" ; exit 0 ;;\n    deny) printf 'cargo-deny %s\\n' \"{cargo_deny_version}\" ; exit 0 ;;\n    fuzz) printf 'cargo-fuzz %s\\n' \"{cargo_fuzz_version}\" ; exit 0 ;;\n    llvm-cov) printf 'cargo-llvm-cov %s\\n' \"{cargo_llvm_cov_version}\" ; exit 0 ;;\n    nextest) printf 'cargo-nextest %s\\n' \"{cargo_nextest_version}\" ; exit 0 ;;\n    outdated) printf 'cargo-outdated %s\\n' \"{cargo_outdated_version}\" ; exit 0 ;;\n    semver-checks) printf 'cargo-semver-checks %s\\n' \"{cargo_semver_checks_version}\" ; exit 0 ;;\n    miri) printf 'cargo-miri test-stub\\n' ; exit 0 ;;\n  esac\nfi\nif [ \"$1\" = \"build\" ]; then\n  target_root=\"${{CARGO_TARGET_DIR:-$PWD/target}}\"\n  mkdir -p \"$target_root/dist\"\n  printf '#!/bin/sh\\nexit 0\\n' > \"$target_root/dist/ffhn\"\n  chmod +x \"$target_root/dist/ffhn\"\nfi\nif [ \"$1\" = \"{qa_nightly_toolchain}\" ] && [ \"$2\" = \"llvm-cov\" ] && [ \"$3\" = \"--branch\" ]; then\n  while [ \"$#\" -gt 0 ]; do\n    if [ \"$1\" = \"--output-path\" ]; then\n      shift\n      output_path=\"$1\"\n      break\n    fi\n    shift\n  done\n  mkdir -p \"$(dirname \"$output_path\")\"\n  printf '%s' '{}' | sed \"s|REPO_ROOT|$PWD|g\" > \"$output_path\"\nfi\nif [ \"$1\" = \"{qa_nightly_toolchain}\" ] && [ \"$2\" = \"miri\" ] && [ \"$3\" = \"test\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"{qa_nightly_toolchain}\" ] && [ \"$2\" = \"miri\" ] && [ \"$3\" = \"--version\" ]; then\n  printf 'miri 0.1.0 (test stub)\\n'\n  exit 0\nfi\nexit 0\n",
        coverage_json.replace('\'', "'\"'\"'"),
        qa_nightly_toolchain = tooling.qa_nightly_toolchain_arg(),
        cargo_audit_version = tooling.cargo_audit_version,
        cargo_deny_version = tooling.cargo_deny_version,
        cargo_fuzz_version = tooling.cargo_fuzz_version,
        cargo_llvm_cov_version = tooling.cargo_llvm_cov_version,
        cargo_nextest_version = tooling.cargo_nextest_version,
        cargo_outdated_version = tooling.cargo_outdated_version,
        cargo_semver_checks_version = tooling.cargo_semver_checks_version,
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
    let _guard = PROCESS_ENV_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let original_path = env::var_os("PATH").unwrap_or_default();
    let original_repo_root = env::var_os(TEST_REPO_ROOT_ENV);
    let mut updated_path = std::ffi::OsString::from(bin_dir);
    updated_path.push(":");
    updated_path.push(&original_path);
    write_executable(
        &bin_dir.join("rustc"),
        "#!/bin/sh\ncase \"$1\" in\n  +1.95.0|+nightly-2026-05-11) printf 'rustc 1.95.0 (test stub)\\n' ;;\n  --version) printf 'rustc 1.95.0 (test stub)\\n' ;;\n  *) printf 'rustc 1.95.0 (test stub)\\n' ;;\nesac\n",
    );
    write_executable(
        &bin_dir.join("shellcheck"),
        "#!/bin/sh\nprintf 'ShellCheck - test stub\\n'\n",
    );
    write_executable(
        &bin_dir.join("rustup"),
        "#!/bin/sh\nif [ \"$1\" = \"component\" ] && [ \"$2\" = \"list\" ] && [ \"$3\" = \"--installed\" ]; then\n  printf 'miri (installed)\\n'\n  printf 'rust-src (installed)\\n'\n  exit 0\nfi\nif [ \"$1\" = \"--version\" ]; then\n  printf 'rustup 1.0.0 (test stub)\\n'\n  exit 0\nfi\nprintf 'rustup 1.0.0 (test stub)\\n'\n",
    );
    // SAFETY: environment mutation is process-global and not thread-safe. This is safe only
    // because PROCESS_ENV_LOCK is held for the entire mutation window, and every test that reads or
    // writes PATH or TEST_REPO_ROOT_ENV (including through spawned commands) must acquire the
    // same lock first. Adding a PATH-dependent test without PROCESS_ENV_LOCK breaks that invariant and
    // risks a data race through the shared process environment.
    unsafe { env::set_var("PATH", &updated_path) };
    if let Some(repo_root_override) = repo_root_override {
        // SAFETY: same invariant as above; PROCESS_ENV_LOCK still serializes all PATH and
        // TEST_REPO_ROOT_ENV access for the current test process.
        unsafe { env::set_var(TEST_REPO_ROOT_ENV, repo_root_override) };
    }
    let result = operation();
    // SAFETY: same invariant as above; PROCESS_ENV_LOCK still serializes all PATH and
    // TEST_REPO_ROOT_ENV access for the current test process.
    unsafe { env::set_var("PATH", original_path) };
    match original_repo_root {
        Some(repo_root) => {
            // SAFETY: same invariant as above; PROCESS_ENV_LOCK still serializes all PATH and
            // TEST_REPO_ROOT_ENV access for the current test process.
            unsafe { env::set_var(TEST_REPO_ROOT_ENV, repo_root) };
        }
        None => {
            // SAFETY: same invariant as above; PROCESS_ENV_LOCK still serializes all PATH and
            // TEST_REPO_ROOT_ENV access for the current test process.
            unsafe { env::remove_var(TEST_REPO_ROOT_ENV) };
        }
    }
    result
}

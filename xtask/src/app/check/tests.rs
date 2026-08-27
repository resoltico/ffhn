//! Focused maintenance-gate orchestration scenarios.

use super::*;

#[cfg(unix)]
use crate::tests::PROCESS_ENV_LOCK;
#[cfg(unix)]
use std::env;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

#[cfg(unix)]
fn write_executable(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    fs::write(&path, body).expect("write executable");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("set permissions");
}

#[cfg(windows)]
fn unsuccessful_probe_command() -> (&'static str, &'static [&'static str]) {
    ("cmd", &["/C", "exit 7"])
}

#[cfg(not(windows))]
fn unsuccessful_probe_command() -> (&'static str, &'static [&'static str]) {
    ("/bin/sh", &["-c", "exit 7"])
}

#[cfg(unix)]
fn sample_tooling() -> RustTooling {
    crate::tooling::parse_rust_tooling(
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
    .expect("parse tooling")
}

#[test]
fn reported_version_extracts_digit_prefixed_segments_and_rejects_plain_text() {
    assert_eq!(reported_version("cargo-audit 0.22.2"), Some("0.22.2"));
    assert_eq!(reported_version("cargo-audit version unknown"), None);
    assert_eq!(
        bootstrap_hint(),
        "Run `./scripts/bootstrap-rust-tools.sh install-all` to install FFHN's pinned Rust toolchains and QA tools."
    );
}

#[test]
fn coverage_failure_messages_preserve_independent_line_and_branch_diagnostics() {
    let line_only = crate::model::CoverageFailure {
        file: "line.rs".to_owned(),
        uncovered_lines: vec!["7".to_owned()],
        uncovered_branch_count: 0,
    };
    assert_eq!(
        coverage_failure_messages(&line_only),
        ["- line.rs lines: 7".to_owned()]
    );
    let branch_only = crate::model::CoverageFailure {
        file: "branch.rs".to_owned(),
        uncovered_lines: Vec::new(),
        uncovered_branch_count: 1,
    };
    assert_eq!(
        coverage_failure_messages(&branch_only),
        ["- branch.rs branches: 1 uncovered".to_owned()]
    );
    let both = crate::model::CoverageFailure {
        file: "both.rs".to_owned(),
        uncovered_lines: vec!["9".to_owned()],
        uncovered_branch_count: 2,
    };
    assert_eq!(coverage_failure_messages(&both).len(), 2);
}

#[test]
fn ensure_toolchain_available_reports_missing_toolchains() {
    let error = ensure_toolchain_available_with("rustc", "definitely-missing-ffhn-toolchain")
        .expect_err("missing toolchain should fail");
    assert!(error.to_string().contains("required Rust toolchain"));
    assert!(error.to_string().contains("not available"));
}

#[cfg(unix)]
#[test]
fn ensure_cargo_subcommand_reports_missing_command() {
    let temp = tempdir().expect("tempdir");
    let cargo_program = temp.path().join("missing-cargo");
    let error = ensure_cargo_subcommand_with(
        cargo_program.to_str().expect("cargo path"),
        CargoQaToolSpec {
            package_name: "cargo-audit",
            subcommand_name: "audit",
            expected_version: "0.22.2",
        },
        "hint",
    )
    .expect_err("missing cargo should fail");

    assert!(
        error
            .to_string()
            .contains("required Cargo QA tool `cargo-audit` is unavailable")
    );
}

#[cfg(unix)]
#[test]
fn ensure_cargo_subcommand_reports_non_success_invalid_utf8_parse_failure_and_version_mismatch() {
    let temp = tempdir().expect("tempdir");
    let cargo_program = temp.path().join("cargo");
    let cargo_program = cargo_program.to_str().expect("cargo path");
    write_executable(temp.path(), "cargo", "#!/bin/sh\nexit 7\n");
    let non_success = ensure_cargo_subcommand_with(
        cargo_program,
        CargoQaToolSpec {
            package_name: "cargo-audit",
            subcommand_name: "audit",
            expected_version: "0.22.2",
        },
        "hint",
    )
    .expect_err("non-success status should fail");
    assert!(
        non_success
            .to_string()
            .contains("did not report a version successfully")
    );

    write_executable(temp.path(), "cargo", "#!/bin/sh\nprintf '\\377'\n");
    let invalid_utf8 = ensure_cargo_subcommand_with(
        cargo_program,
        CargoQaToolSpec {
            package_name: "cargo-audit",
            subcommand_name: "audit",
            expected_version: "0.22.2",
        },
        "hint",
    )
    .expect_err("non-utf8 output should fail");
    assert!(
        invalid_utf8
            .to_string()
            .contains("returned non-UTF-8 output")
    );

    write_executable(
        temp.path(),
        "cargo",
        "#!/bin/sh\nprintf 'cargo-audit version unknown\\n'\n",
    );
    let unparseable = ensure_cargo_subcommand_with(
        cargo_program,
        CargoQaToolSpec {
            package_name: "cargo-audit",
            subcommand_name: "audit",
            expected_version: "0.22.2",
        },
        "hint",
    )
    .expect_err("unparseable output should fail");
    assert!(
        unparseable
            .to_string()
            .contains("could not parse the installed version")
    );

    write_executable(
        temp.path(),
        "cargo",
        "#!/bin/sh\nprintf 'cargo-audit 0.1.0\\n'\n",
    );
    let mismatch = ensure_cargo_subcommand_with(
        cargo_program,
        CargoQaToolSpec {
            package_name: "cargo-audit",
            subcommand_name: "audit",
            expected_version: "0.22.2",
        },
        "hint",
    )
    .expect_err("version mismatch should fail");
    assert!(
        mismatch
            .to_string()
            .contains("is at version 0.1.0, expected 0.22.2")
    );
}

#[test]
fn ensure_command_exists_reports_missing_and_unsuccessful_commands() {
    let temp = tempdir().expect("tempdir");
    let missing_program = temp.path().join(format!(
        "missing-ffhn-command{}",
        std::env::consts::EXE_SUFFIX
    ));
    let missing_program_text = missing_program.to_string_lossy().into_owned();
    let missing = ensure_command_exists(&missing_program_text, &["--version"], "hint")
        .expect_err("missing command should fail");
    assert!(missing.to_string().contains(&format!(
        "required command `{}` is unavailable",
        missing_program.display()
    )));

    let (program, args) = unsuccessful_probe_command();
    let non_success =
        ensure_command_exists(program, args, "hint").expect_err("non-success command should fail");
    assert!(non_success.to_string().contains(&format!(
        "required command `{program}` exited unsuccessfully while probing availability"
    )));
}

#[cfg(unix)]
#[test]
#[allow(unsafe_code)]
fn ensure_miri_prerequisites_reports_missing_components() {
    let _guard = PROCESS_ENV_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let repo_root = tempdir().expect("repo_root");
    let bin_dir = tempdir().expect("bin_dir");
    let original_path = env::var_os("PATH").unwrap_or_default();
    let mut updated_path = std::ffi::OsString::from(bin_dir.path());
    updated_path.push(":");
    updated_path.push(&original_path);

    write_executable(
        bin_dir.path(),
        "rustup",
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'rustup 1.0.0 (test stub)\\n'\n  exit 0\nfi\nif [ \"$1\" = \"component\" ] && [ \"$2\" = \"list\" ] && [ \"$3\" = \"--installed\" ]; then\n  printf 'rust-src (installed)\\n'\n  exit 0\nfi\nexit 0\n",
    );
    write_executable(
        bin_dir.path(),
        "cargo",
        "#!/bin/sh\nif [ \"$1\" = \"+nightly-2026-08-25\" ] && [ \"$2\" = \"miri\" ] && [ \"$3\" = \"--version\" ]; then\n  printf 'miri 0.1.0 (test stub)\\n'\n  exit 0\nfi\nexit 0\n",
    );

    // SAFETY: PROCESS_ENV_LOCK serializes process-environment mutation in this module.
    unsafe { env::set_var("PATH", &updated_path) };
    let error = ensure_miri_prerequisites(repo_root.path(), &sample_tooling())
        .expect_err("missing components should fail");
    // SAFETY: PROCESS_ENV_LOCK still serializes process-environment mutation in this module.
    unsafe { env::set_var("PATH", original_path) };

    assert!(
        error
            .to_string()
            .contains("rustup component add miri --toolchain nightly-2026-08-25")
    );
}

#[cfg(unix)]
#[test]
#[allow(unsafe_code)]
fn ensure_miri_prerequisites_reports_broken_miri_binaries() {
    let _guard = PROCESS_ENV_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let repo_root = tempdir().expect("repo_root");
    let bin_dir = tempdir().expect("bin_dir");
    let original_path = env::var_os("PATH").unwrap_or_default();
    let mut updated_path = std::ffi::OsString::from(bin_dir.path());
    updated_path.push(":");
    updated_path.push(&original_path);

    write_executable(
        bin_dir.path(),
        "rustup",
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'rustup 1.0.0 (test stub)\\n'\n  exit 0\nfi\nif [ \"$1\" = \"component\" ] && [ \"$2\" = \"list\" ] && [ \"$3\" = \"--installed\" ]; then\n  printf 'miri (installed)\\n'\n  printf 'rust-src (installed)\\n'\n  exit 0\nfi\nexit 0\n",
    );
    write_executable(
        bin_dir.path(),
        "cargo",
        "#!/bin/sh\nif [ \"$1\" = \"+nightly-2026-08-25\" ] && [ \"$2\" = \"miri\" ] && [ \"$3\" = \"--version\" ]; then\n  exit 7\nfi\nexit 0\n",
    );

    // SAFETY: PROCESS_ENV_LOCK serializes process-environment mutation in this module.
    unsafe { env::set_var("PATH", &updated_path) };
    let error = ensure_miri_prerequisites(repo_root.path(), &sample_tooling())
        .expect_err("broken miri probe should fail");
    // SAFETY: PROCESS_ENV_LOCK still serializes process-environment mutation in this module.
    unsafe { env::set_var("PATH", original_path) };

    assert!(
        error
            .to_string()
            .contains("cargo +nightly-2026-08-25 miri --version")
    );
    assert!(
        error
            .to_string()
            .contains("rustup toolchain uninstall nightly-2026-08-25")
    );
}

#[cfg(unix)]
#[test]
#[allow(unsafe_code)]
fn ensure_miri_prerequisites_reports_component_query_failure() {
    let _guard = PROCESS_ENV_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let repo_root = tempdir().expect("repo_root");
    let bin_dir = tempdir().expect("bin_dir");
    let original_path = env::var_os("PATH").unwrap_or_default();
    let mut updated_path = std::ffi::OsString::from(bin_dir.path());
    updated_path.push(":");
    updated_path.push(&original_path);

    write_executable(
        bin_dir.path(),
        "rustup",
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'rustup 1.0.0 (test stub)\\n'\n  exit 0\nfi\nif [ \"$1\" = \"component\" ] && [ \"$2\" = \"list\" ] && [ \"$3\" = \"--installed\" ]; then\n  exit 9\nfi\nexit 0\n",
    );
    write_executable(bin_dir.path(), "cargo", "#!/bin/sh\nexit 0\n");

    // SAFETY: PROCESS_ENV_LOCK serializes process-environment mutation in this module.
    unsafe { env::set_var("PATH", &updated_path) };
    let error = ensure_miri_prerequisites(repo_root.path(), &sample_tooling())
        .expect_err("unsuccessful rustup component query should fail");
    // SAFETY: PROCESS_ENV_LOCK still serializes process-environment mutation in this module.
    unsafe { env::set_var("PATH", original_path) };

    assert!(
        error
            .to_string()
            .contains("failed to query rustup nightly components for `nightly-2026-08-25`")
    );
}

#[cfg(unix)]
#[test]
#[allow(unsafe_code)]
fn ensure_miri_prerequisites_reports_non_utf8_component_output() {
    let _guard = PROCESS_ENV_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let repo_root = tempdir().expect("repo_root");
    let bin_dir = tempdir().expect("bin_dir");
    let original_path = env::var_os("PATH").unwrap_or_default();
    let mut updated_path = std::ffi::OsString::from(bin_dir.path());
    updated_path.push(":");
    updated_path.push(&original_path);

    write_executable(
        bin_dir.path(),
        "rustup",
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'rustup 1.0.0 (test stub)\\n'\n  exit 0\nfi\nif [ \"$1\" = \"component\" ] && [ \"$2\" = \"list\" ] && [ \"$3\" = \"--installed\" ]; then\n  printf '\\377'\n  exit 0\nfi\nexit 0\n",
    );
    write_executable(bin_dir.path(), "cargo", "#!/bin/sh\nexit 0\n");

    // SAFETY: PROCESS_ENV_LOCK serializes process-environment mutation in this module.
    unsafe { env::set_var("PATH", &updated_path) };
    let error = ensure_miri_prerequisites(repo_root.path(), &sample_tooling())
        .expect_err("non-UTF-8 rustup component output should fail");
    // SAFETY: PROCESS_ENV_LOCK still serializes process-environment mutation in this module.
    unsafe { env::set_var("PATH", original_path) };

    assert!(
        error
            .to_string()
            .contains("rustup component list returned non-UTF-8 output for `nightly-2026-08-25`")
    );
}

use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::process::Stdio;
use std::{fs, path::PathBuf};
use std::{thread, time::Duration};

use crate::coverage::{
    coverage_clean_command, coverage_command, coverage_output_path, evaluate_coverage_report,
    read_coverage_report, tracked_files,
};
use crate::hygiene::{HygieneCleanMode, clean_hygiene, ensure_hygiene, prepare_artifact_layout};
use crate::miri::{
    miri_command, miri_preflight_failures, miri_preflight_message, miri_probe_command,
};
use crate::model::{CommandArtifactLayout, CommandSpec, DynResult};
use crate::plan::{
    check_plan, is_semver_check_spec, semver_baseline_target_dir, semver_build_dir,
    semver_check_spec, semver_scratch_dir,
};
use crate::tooling::{CargoQaToolSpec, RustTooling, rust_tooling};

use super::command::prepare_command;
use super::command::{remove_dir_if_exists, run_spec};

const AUDIT_RETRY_ATTEMPTS: usize = 3;
#[cfg(test)]
const AUDIT_RETRY_DELAY: Duration = Duration::from_millis(1);
#[cfg(not(test))]
const AUDIT_RETRY_DELAY: Duration = Duration::from_secs(5);
const TRANSIENT_AUDIT_FETCH_MARKERS: [&str; 4] = [
    "couldn't fetch advisory database",
    "failed to prepare fetch",
    "error sending request for url",
    "An IO error occurred when talking to the server",
];

pub(crate) fn run_check(repo_root: &Path) -> DynResult<()> {
    println!("==> Rust gate");
    let tooling = preflight_full_check(repo_root)?;
    clean_hygiene(repo_root, HygieneCleanMode::Safe)?;
    prepare_artifact_layout(repo_root, CommandArtifactLayout::ManagedWorkspace)?;
    ensure_hygiene(repo_root)?;

    for spec in check_plan(repo_root, &tooling)? {
        if is_semver_check_spec(&spec) {
            prepare_semver_artifacts(repo_root)?;
            let result = run_spec(repo_root, &spec);
            let cleanup = remove_semver_artifacts(repo_root);
            result?;
            cleanup?;
            continue;
        }

        run_spec(repo_root, &spec)?;
    }

    run_coverage_with_tooling(repo_root, &tooling)?;
    clean_hygiene(repo_root, HygieneCleanMode::Safe)?;
    ensure_hygiene(repo_root)
}

pub(crate) fn run_semver_check(repo_root: &Path) -> DynResult<()> {
    let tooling = rust_tooling(repo_root)?;
    ensure_toolchain_available(&tooling.stable_toolchain)?;
    ensure_cargo_subcommand(
        CargoQaToolSpec {
            package_name: "cargo-semver-checks",
            subcommand_name: "semver-checks",
            expected_version: &tooling.cargo_semver_checks_version,
        },
        bootstrap_hint(),
    )?;
    clean_hygiene(repo_root, HygieneCleanMode::Safe)?;
    prepare_artifact_layout(repo_root, CommandArtifactLayout::ManagedWorkspace)?;
    ensure_hygiene(repo_root)?;
    let spec = semver_check_spec(repo_root)?;
    prepare_semver_artifacts(repo_root)?;
    let result = run_spec(repo_root, &spec);
    let cleanup = remove_semver_artifacts(repo_root);
    result?;
    cleanup?;
    clean_hygiene(repo_root, HygieneCleanMode::Safe)?;
    ensure_hygiene(repo_root)
}

pub(crate) fn run_coverage(repo_root: &Path) -> DynResult<()> {
    let tooling = rust_tooling(repo_root)?;
    ensure_toolchain_available(&tooling.qa_nightly_toolchain)?;
    ensure_cargo_subcommand(
        CargoQaToolSpec {
            package_name: "cargo-llvm-cov",
            subcommand_name: "llvm-cov",
            expected_version: &tooling.cargo_llvm_cov_version,
        },
        bootstrap_hint(),
    )?;
    run_coverage_with_tooling(repo_root, &tooling)
}

pub(crate) fn run_miri(repo_root: &Path) -> DynResult<()> {
    let tooling = rust_tooling(repo_root)?;
    ensure_toolchain_available(&tooling.qa_nightly_toolchain)?;
    ensure_miri_prerequisites(repo_root, &tooling)?;
    clean_hygiene(repo_root, HygieneCleanMode::Safe)?;
    prepare_artifact_layout(repo_root, CommandArtifactLayout::ManagedWorkspace)?;
    ensure_hygiene(repo_root)?;
    let result = run_spec(repo_root, &miri_command(&tooling));
    let cleanup = clean_hygiene(repo_root, HygieneCleanMode::Safe);
    result?;
    cleanup?;
    ensure_hygiene(repo_root)
}

pub(crate) fn run_audit(repo_root: &Path, lockfile: Option<&Path>) -> DynResult<()> {
    let tooling = rust_tooling(repo_root)?;
    ensure_cargo_subcommand(
        CargoQaToolSpec {
            package_name: "cargo-audit",
            subcommand_name: "audit",
            expected_version: &tooling.cargo_audit_version,
        },
        bootstrap_hint(),
    )?;

    let spec = audit_spec(lockfile);
    run_retrying_audit(repo_root, &spec)
}

fn run_coverage_with_tooling(repo_root: &Path, tooling: &RustTooling) -> DynResult<()> {
    clean_hygiene(repo_root, HygieneCleanMode::Safe)?;
    prepare_artifact_layout(repo_root, CommandArtifactLayout::ManagedCoverage)?;
    ensure_hygiene(repo_root)?;
    let coverage_clean_spec = coverage_clean_command(tooling);
    let coverage_spec = coverage_command(repo_root, tooling);
    run_spec(repo_root, &coverage_clean_spec)?;

    let result = (|| -> DynResult<()> {
        run_spec(repo_root, &coverage_spec)?;

        let tracked = tracked_files(repo_root)?;
        let report = read_coverage_report(&coverage_output_path(repo_root))?;
        let summary = evaluate_coverage_report(repo_root, &tracked, report)?;

        if !summary.failures.is_empty() {
            eprintln!("Rust coverage gate failed.");
            for failure in summary.failures {
                if !failure.uncovered_lines.is_empty() {
                    eprintln!(
                        "- {} lines: {}",
                        failure.file,
                        failure.uncovered_lines.join(", ")
                    );
                }
                if failure.uncovered_branch_count > 0 {
                    eprintln!(
                        "- {} branches: {} uncovered",
                        failure.file, failure.uncovered_branch_count
                    );
                }
            }
            return Err("coverage gate failed".into());
        }

        println!(
            "Rust coverage: lines 100.00% ({0}/{0}) | branches 100.00% ({1}/{1})",
            summary.tracked_line_count, summary.tracked_branch_count
        );
        Ok(())
    })();

    let cleanup = run_spec(repo_root, &coverage_clean_spec);
    result?;
    cleanup?;
    clean_hygiene(repo_root, HygieneCleanMode::Safe)?;
    ensure_hygiene(repo_root)
}

fn preflight_full_check(repo_root: &Path) -> DynResult<RustTooling> {
    let tooling = rust_tooling(repo_root)?;
    ensure_command_exists(
        "shellcheck",
        &["--version"],
        "Install shellcheck for your platform.",
    )?;
    ensure_toolchain_available(&tooling.stable_toolchain)?;
    ensure_toolchain_available(&tooling.qa_nightly_toolchain)?;
    ensure_miri_prerequisites(repo_root, &tooling)?;
    for tool in tooling.cargo_qa_tools() {
        ensure_cargo_subcommand(tool, bootstrap_hint())?;
    }
    Ok(tooling)
}

fn ensure_toolchain_available(toolchain: &str) -> DynResult<()> {
    ensure_toolchain_available_with("rustc", toolchain)
}

fn ensure_toolchain_available_with(rustc_program: &str, toolchain: &str) -> DynResult<()> {
    let status = Command::new(rustc_program)
        .args([format!("+{toolchain}"), "--version".to_owned()])
        .status()?;
    if status.success() {
        return Ok(());
    }

    Err(format!(
        "required Rust toolchain `{toolchain}` is not available. {}",
        bootstrap_hint()
    )
    .into())
}

fn ensure_cargo_subcommand(tool: CargoQaToolSpec<'_>, hint: &str) -> DynResult<()> {
    ensure_cargo_subcommand_with("cargo", tool, hint)
}

fn ensure_cargo_subcommand_with(
    cargo_program: &str,
    tool: CargoQaToolSpec<'_>,
    hint: &str,
) -> DynResult<()> {
    let output = Command::new(cargo_program)
        .args([tool.subcommand_name, "--version"])
        .output()
        .map_err(|error| {
            format!(
                "required Cargo QA tool `{}` is unavailable: {error}. {hint}",
                tool.package_name
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "required Cargo QA tool `{}` did not report a version successfully. {hint}",
            tool.package_name
        )
        .into());
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        format!(
            "{} --version returned non-UTF-8 output: {error}",
            tool.package_name
        )
    })?;
    let reported_version = reported_version(&stdout).ok_or_else(|| {
        format!(
            "could not parse the installed version for `{}` from `{}`",
            tool.package_name,
            stdout.trim()
        )
    })?;
    if reported_version == tool.expected_version {
        return Ok(());
    }

    Err(format!(
        "required Cargo QA tool `{}` is at version {reported_version}, expected {}. {hint}",
        tool.package_name, tool.expected_version
    )
    .into())
}

fn ensure_command_exists(program: &str, args: &[&str], hint: &str) -> DynResult<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|error| format!("required command `{program}` is unavailable: {error}. {hint}"))?;
    if status.success() {
        return Ok(());
    }

    Err(format!(
        "required command `{program}` exited unsuccessfully while probing availability. {hint}"
    )
    .into())
}

fn reported_version(output: &str) -> Option<&str> {
    output.split_whitespace().find(|segment| {
        segment
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
    })
}

fn bootstrap_hint() -> &'static str {
    "Run `./scripts/bootstrap-rust-tools.sh install-all` to install FFHN's pinned Rust toolchains and QA tools."
}

fn ensure_miri_prerequisites(repo_root: &Path, tooling: &RustTooling) -> DynResult<()> {
    ensure_command_exists(
        "rustup",
        &["--version"],
        "Install rustup before running FFHN's maintained Miri proof.",
    )?;
    let installed_components_output = Command::new("rustup")
        .args([
            "component",
            "list",
            "--installed",
            "--toolchain",
            &tooling.qa_nightly_toolchain,
        ])
        .output()
        .map_err(|error| format!("failed to query rustup nightly components: {error}"))?;
    if !installed_components_output.status.success() {
        return Err(format!(
            "failed to query rustup nightly components for `{}`. {}",
            tooling.qa_nightly_toolchain,
            bootstrap_hint()
        )
        .into());
    }
    let installed_components =
        String::from_utf8(installed_components_output.stdout).map_err(|error| {
            format!(
                "rustup component list returned non-UTF-8 output for `{}`: {error}",
                tooling.qa_nightly_toolchain
            )
        })?;

    let miri_binary_runs = run_spec(repo_root, &miri_probe_command(tooling)).is_ok();
    let failures = miri_preflight_failures(&installed_components, miri_binary_runs);
    if failures.is_empty() {
        return Ok(());
    }

    Err(miri_preflight_message(tooling, &failures).into())
}

fn audit_spec(lockfile: Option<&Path>) -> CommandSpec {
    let mut args = vec!["audit".to_owned()];
    if let Some(lockfile) = lockfile {
        args.push("--file".to_owned());
        args.push(lockfile.to_string_lossy().into_owned());
    }
    args.push("-D".to_owned());
    args.push("warnings".to_owned());
    CommandSpec::new("cargo", args, false)
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace)
}

fn run_retrying_audit(repo_root: &Path, spec: &CommandSpec) -> DynResult<()> {
    let mut last_error = None;
    for attempt in 1..=AUDIT_RETRY_ATTEMPTS {
        let mut command = Command::new(&spec.program);
        prepare_command(&mut command, repo_root, spec)?;
        command.stdin(Stdio::inherit());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let output = command.output()?;
        io::stdout().write_all(&output.stdout)?;
        io::stderr().write_all(&output.stderr)?;
        if output.status.success() {
            return Ok(());
        }

        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let retryable = is_transient_audit_fetch_failure(&combined);
        let status_error = format!("command failed with status {}", output.status);
        if retryable && attempt < AUDIT_RETRY_ATTEMPTS {
            eprintln!(
                "Transient RustSec advisory-database fetch failure on attempt {attempt}/{AUDIT_RETRY_ATTEMPTS}; retrying in {} seconds.",
                AUDIT_RETRY_DELAY.as_secs()
            );
            thread::sleep(AUDIT_RETRY_DELAY);
            continue;
        }

        last_error = Some(status_error);
        break;
    }

    Err(last_error
        .unwrap_or_else(|| "command failed without a reported process status".to_owned())
        .into())
}

fn is_transient_audit_fetch_failure(output: &str) -> bool {
    TRANSIENT_AUDIT_FETCH_MARKERS
        .iter()
        .any(|marker| output.contains(marker))
}

fn remove_semver_artifacts(repo_root: &Path) -> DynResult<()> {
    remove_dir_if_exists(&semver_scratch_dir(repo_root))?;
    remove_dir_if_exists(&semver_build_dir(repo_root))?;
    remove_dir_if_exists(&semver_baseline_target_dir(repo_root))
}

fn prepare_semver_artifacts(repo_root: &Path) -> DynResult<()> {
    remove_semver_artifacts(repo_root)?;
    for path in semver_required_directories(repo_root) {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

fn semver_required_directories(repo_root: &Path) -> [PathBuf; 6] {
    let scratch_dir = semver_scratch_dir(repo_root);
    let build_dir = semver_build_dir(repo_root);
    [
        scratch_dir.clone(),
        scratch_dir.join("debug"),
        scratch_dir.join("debug").join("deps"),
        build_dir.clone(),
        build_dir.join("debug"),
        build_dir.join("debug").join("deps"),
    ]
}

#[cfg(test)]
mod tests {
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
RUST_WORKSPACE_RUST_VERSION=1.95\n\
RUST_STABLE_TOOLCHAIN=1.95.0\n\
RUST_QA_NIGHTLY_TOOLCHAIN=nightly-2026-05-11\n\
\n\
CARGO_AUDIT_VERSION=0.22.1\n\
CARGO_DENY_VERSION=0.19.4\n\
CARGO_FUZZ_VERSION=0.13.1\n\
CARGO_LLVM_COV_VERSION=0.8.5\n\
CARGO_NEXTEST_VERSION=0.9.133\n\
CARGO_OUTDATED_VERSION=0.19.0\n\
CARGO_SEMVER_CHECKS_VERSION=0.47.0\n",
        )
        .expect("parse tooling")
    }

    #[test]
    fn reported_version_extracts_digit_prefixed_segments_and_rejects_plain_text() {
        assert_eq!(reported_version("cargo-audit 0.22.1"), Some("0.22.1"));
        assert_eq!(reported_version("cargo-audit version unknown"), None);
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
                expected_version: "0.22.1",
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
    fn ensure_cargo_subcommand_reports_non_success_invalid_utf8_parse_failure_and_version_mismatch()
    {
        let temp = tempdir().expect("tempdir");
        let cargo_program = temp.path().join("cargo");
        let cargo_program = cargo_program.to_str().expect("cargo path");
        write_executable(temp.path(), "cargo", "#!/bin/sh\nexit 7\n");
        let non_success = ensure_cargo_subcommand_with(
            cargo_program,
            CargoQaToolSpec {
                package_name: "cargo-audit",
                subcommand_name: "audit",
                expected_version: "0.22.1",
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
                expected_version: "0.22.1",
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
                expected_version: "0.22.1",
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
                expected_version: "0.22.1",
            },
            "hint",
        )
        .expect_err("version mismatch should fail");
        assert!(
            mismatch
                .to_string()
                .contains("is at version 0.1.0, expected 0.22.1")
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
        let non_success = ensure_command_exists(program, args, "hint")
            .expect_err("non-success command should fail");
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
            "#!/bin/sh\nif [ \"$1\" = \"+nightly-2026-05-11\" ] && [ \"$2\" = \"miri\" ] && [ \"$3\" = \"--version\" ]; then\n  printf 'miri 0.1.0 (test stub)\\n'\n  exit 0\nfi\nexit 0\n",
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
                .contains("rustup component add miri --toolchain nightly-2026-05-11")
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
            "#!/bin/sh\nif [ \"$1\" = \"+nightly-2026-05-11\" ] && [ \"$2\" = \"miri\" ] && [ \"$3\" = \"--version\" ]; then\n  exit 7\nfi\nexit 0\n",
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
                .contains("cargo +nightly-2026-05-11 miri --version")
        );
        assert!(
            error
                .to_string()
                .contains("rustup toolchain uninstall nightly-2026-05-11")
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
                .contains("failed to query rustup nightly components for `nightly-2026-05-11`")
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
            error.to_string().contains(
                "rustup component list returned non-UTF-8 output for `nightly-2026-05-11`"
            )
        );
    }
}

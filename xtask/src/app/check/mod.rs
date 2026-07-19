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

use super::GateOutputOptions;
use super::command::prepare_command;
use super::command::{remove_dir_if_exists, run_spec};
use super::gate::GateReporter;

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

pub(crate) fn run_check(repo_root: &Path, output: GateOutputOptions) -> DynResult<()> {
    let mut reporter = GateReporter::new(repo_root, "check", output)?;
    let result = run_check_with_reporter(repo_root, &mut reporter);
    let finish = reporter.finish(result.is_ok());
    result?;
    finish
}

fn run_check_with_reporter(repo_root: &Path, reporter: &mut GateReporter) -> DynResult<()> {
    let tooling = reporter.run_operation("preflight", || preflight_full_check(repo_root))?;
    reporter.run_operation("hygiene-clean-before", || {
        clean_hygiene(repo_root, HygieneCleanMode::Safe)
    })?;
    reporter.run_operation("artifact-layout", || {
        prepare_artifact_layout(repo_root, CommandArtifactLayout::ManagedWorkspace)
    })?;
    reporter.run_operation("hygiene-verify-before", || ensure_hygiene(repo_root))?;
    reporter.run_operation("source-structure", || crate::structure::check(repo_root))?;

    for spec in check_plan(repo_root, &tooling)? {
        if is_semver_check_spec(&spec) {
            reporter.run_operation("semver-artifacts-prepare", || {
                prepare_semver_artifacts(repo_root)
            })?;
            let result = reporter.run_spec(repo_root, &spec);
            let cleanup = reporter.run_operation("semver-artifacts-clean", || {
                remove_semver_artifacts(repo_root)
            });
            result?;
            cleanup?;
            continue;
        }

        reporter.run_spec(repo_root, &spec)?;
    }

    reporter.run_operation("coverage", || {
        run_coverage_with_tooling(repo_root, &tooling)
    })?;
    reporter.run_operation("hygiene-clean-after", || {
        clean_hygiene(repo_root, HygieneCleanMode::Safe)
    })?;
    reporter.run_operation("hygiene-verify-after", || ensure_hygiene(repo_root))
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
    let output = Command::new(rustc_program)
        .args([format!("+{toolchain}"), "--version".to_owned()])
        .output()?;
    if output.status.success() {
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
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("required command `{program}` is unavailable: {error}. {hint}"))?;
    if output.status.success() {
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
mod tests;

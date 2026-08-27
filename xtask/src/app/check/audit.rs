//! RustSec audit orchestration, transient retries, and feature-reachability exceptions.

use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::{fs, thread, time::Duration};

use crate::model::{CommandArtifactLayout, CommandSpec, DynResult};
use crate::tooling::{CargoQaToolSpec, rust_tooling};

use super::{bootstrap_hint, ensure_cargo_subcommand};
use crate::app::command::prepare_command;

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
const UNREACHABLE_RKYV_ADVISORY: &str = "RUSTSEC-2026-0235";
const UNREACHABLE_RKYV_VERSION: &str = "0.7.46";

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
    let ignored = unreachable_advisory_ignores(repo_root, lockfile)?;
    run_retrying_audit(repo_root, &audit_spec(lockfile, &ignored))
}

fn audit_spec(lockfile: Option<&Path>, ignored: &[&str]) -> CommandSpec {
    let mut args = vec!["audit".to_owned()];
    if let Some(lockfile) = lockfile {
        args.push("--file".to_owned());
        args.push(lockfile.to_string_lossy().into_owned());
    }
    for advisory in ignored {
        args.push("--ignore".to_owned());
        args.push((*advisory).to_owned());
    }
    args.extend(["-D".to_owned(), "warnings".to_owned()]);
    CommandSpec::new("cargo", args, false)
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace)
}

fn unreachable_advisory_ignores(
    repo_root: &Path,
    lockfile: Option<&Path>,
) -> DynResult<Vec<&'static str>> {
    let lockfile = lockfile
        .map(|path| repo_root.join(path))
        .unwrap_or_else(|| repo_root.join("Cargo.lock"));
    let lock: toml::Value = toml::from_str(&fs::read_to_string(&lockfile)?)?;
    let contains_rkyv = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .is_some_and(|packages| {
            packages.iter().any(|package| {
                package.get("name").and_then(toml::Value::as_str) == Some("rkyv")
                    && package.get("version").and_then(toml::Value::as_str)
                        == Some(UNREACHABLE_RKYV_VERSION)
            })
        });
    if !contains_rkyv {
        return Ok(Vec::new());
    }
    prove_rkyv_unreachable(repo_root, &lockfile)?;
    Ok(vec![UNREACHABLE_RKYV_ADVISORY])
}

fn prove_rkyv_unreachable(repo_root: &Path, lockfile: &Path) -> DynResult<()> {
    let manifest = lockfile
        .parent()
        .ok_or("audit lockfile has no parent directory")?
        .join("Cargo.toml");
    let manifest = manifest
        .strip_prefix(repo_root)
        .map_err(|_| "audit lockfile must be inside the repository")?;
    let mut args = vec!["tree".to_owned()];
    if manifest != Path::new("Cargo.toml") {
        args.extend([
            "--manifest-path".to_owned(),
            manifest.to_string_lossy().into_owned(),
        ]);
    }
    args.extend([
        "--all-targets".to_owned(),
        "--all-features".to_owned(),
        "--target".to_owned(),
        "all".to_owned(),
        "--invert".to_owned(),
        format!("rkyv@{UNREACHABLE_RKYV_VERSION}"),
        "--locked".to_owned(),
    ]);
    let spec = CommandSpec::new("cargo", args, false)
        .with_artifact_layout(CommandArtifactLayout::ManagedWorkspace);
    let mut command = Command::new(&spec.program);
    prepare_command(&mut command, repo_root, &spec)?;
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "could not prove {UNREACHABLE_RKYV_ADVISORY} unreachable for {}",
            lockfile.display()
        )
        .into());
    }
    if String::from_utf8_lossy(&output.stdout)
        .contains(&format!("rkyv v{UNREACHABLE_RKYV_VERSION}"))
    {
        return Err(format!(
            "{UNREACHABLE_RKYV_ADVISORY} is reachable in the maintained build graph for {}",
            lockfile.display()
        )
        .into());
    }
    Ok(())
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
        let status_error = format!("command failed with status {}", output.status);
        if is_transient_audit_fetch_failure(&combined) && attempt < AUDIT_RETRY_ATTEMPTS {
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

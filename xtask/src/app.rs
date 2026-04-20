use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, Subcommand};

use crate::coverage::{
    coverage_clean_command, coverage_command, coverage_output_path, evaluate_coverage_report,
    read_coverage_report, tracked_files,
};
use crate::model::{CommandSpec, DynResult};
use crate::plan::{
    check_plan, is_semver_check_spec, semver_scratch_dir, with_workspace_stub, workspace_version,
};

const XTASK_NAME: &str = env!("CARGO_PKG_NAME");
const XTASK_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

#[derive(Parser)]
#[command(name = XTASK_NAME, version, about = XTASK_DESCRIPTION)]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Subcommand)]
enum Task {
    Check,
    Coverage,
    RefreshSemverBaseline,
}

/// Parses the xtask CLI and dispatches the selected maintenance action.
///
/// # Errors
///
/// Returns an error when the workspace root cannot be resolved or when the selected maintenance
/// action fails any required repository check.
pub fn run() -> DynResult<()> {
    let cli = Cli::parse();
    let repo_root = repo_root()?;

    match cli.command {
        Task::Check => run_check(&repo_root),
        Task::Coverage => run_coverage(&repo_root),
        Task::RefreshSemverBaseline => refresh_semver_baseline(&repo_root),
    }
}

fn run_check(repo_root: &Path) -> DynResult<()> {
    println!("==> Rust gate");

    for spec in check_plan(repo_root)? {
        if is_semver_check_spec(&spec) {
            remove_dir_if_exists(&semver_scratch_dir(repo_root))?;
            let result = run_spec(repo_root, &spec);
            let cleanup = remove_dir_if_exists(&semver_scratch_dir(repo_root));
            result?;
            cleanup?;
            continue;
        }

        run_spec(repo_root, &spec)?;
    }

    run_coverage(repo_root)
}

fn run_coverage(repo_root: &Path) -> DynResult<()> {
    let coverage_clean_spec = coverage_clean_command();
    let coverage_spec = coverage_command(repo_root);
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
    cleanup
}

fn refresh_semver_baseline(repo_root: &Path) -> DynResult<()> {
    let version = workspace_version(repo_root)?;
    let workspace_manifest = fs::read_to_string(repo_root.join("Cargo.toml"))?;
    let baseline_parent = repo_root.join("semver-baseline");
    let baseline_dir = baseline_parent.join("ffhn-core");
    let extracted_dir = baseline_parent.join(format!("ffhn-core-{version}"));
    let archive = repo_root
        .join("target")
        .join("package")
        .join(format!("ffhn-core-{version}.crate"));
    let source_dir = repo_root.join("crates").join("ffhn-core");

    if baseline_dir.exists() {
        fs::remove_dir_all(&baseline_dir)?;
    }
    if extracted_dir.exists() {
        fs::remove_dir_all(&extracted_dir)?;
    }
    fs::create_dir_all(&baseline_parent)?;

    let package_result = run_spec(
        repo_root,
        &CommandSpec::new(
            "cargo",
            ["package", "--allow-dirty", "--no-verify", "-p", "ffhn-core"],
            false,
            true,
        ),
    );

    if package_result.is_err() {
        eprintln!(
            "ffhn-core could not be packaged for semver baseline refresh; falling back to a source snapshot"
        );
        copy_dir_all(&source_dir, &baseline_dir)?;
        let baseline_manifest = baseline_dir.join("Cargo.toml");
        let cargo_toml = fs::read_to_string(&baseline_manifest)?;
        fs::write(
            &baseline_manifest,
            with_workspace_stub(&cargo_toml, &workspace_manifest)?,
        )?;
        return Ok(());
    }

    run_spec(
        repo_root,
        &CommandSpec::new(
            "tar",
            [
                "-xzf",
                archive.to_string_lossy().as_ref(),
                "-C",
                baseline_parent.to_string_lossy().as_ref(),
            ],
            false,
            false,
        ),
    )?;

    let baseline_manifest = extracted_dir.join("Cargo.toml");
    let cargo_toml = fs::read_to_string(&baseline_manifest)?;
    fs::write(
        &baseline_manifest,
        with_workspace_stub(&cargo_toml, &workspace_manifest)?,
    )?;
    fs::rename(extracted_dir, baseline_dir)?;
    Ok(())
}

fn run_spec(repo_root: &Path, spec: &CommandSpec) -> DynResult<()> {
    let mut command = Command::new(&spec.program);
    command.current_dir(repo_root);
    command.args(&spec.args);
    command.stdin(Stdio::inherit());
    if spec.quiet_stdout {
        command.stdout(Stdio::null());
    } else {
        command.stdout(Stdio::inherit());
    }
    command.stderr(Stdio::inherit());
    if spec.force_clang {
        command.env("CC", "clang");
    }

    let status = command.status()?;
    if status.success() {
        return Ok(());
    }

    Err(format!("command failed with status {status}").into())
}

fn repo_root() -> DynResult<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask should live directly under the workspace root".into())
}

fn copy_dir_all(source: &Path, destination: &Path) -> DynResult<()> {
    fs::create_dir_all(destination)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let entry_type = entry.file_type()?;
        let from = entry.path();
        let to = destination.join(entry.file_name());

        if entry_type.is_dir() {
            copy_dir_all(&from, &to)?;
        } else if entry_type.is_file() {
            fs::copy(&from, &to)?;
        }
    }

    Ok(())
}

fn remove_dir_if_exists(path: &Path) -> DynResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }

    Ok(())
}

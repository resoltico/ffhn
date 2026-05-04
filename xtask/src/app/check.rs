use std::path::Path;
use std::{fs, path::PathBuf};

use crate::coverage::{
    coverage_clean_command, coverage_command, coverage_output_path, evaluate_coverage_report,
    read_coverage_report, tracked_files,
};
use crate::model::DynResult;
use crate::plan::{
    check_plan, is_semver_check_spec, semver_baseline_target_dir, semver_check_spec,
    semver_scratch_dir,
};

use super::command::{remove_dir_if_exists, run_spec};

pub(crate) fn run_check(repo_root: &Path) -> DynResult<()> {
    println!("==> Rust gate");

    for spec in check_plan(repo_root)? {
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

    run_coverage(repo_root)
}

pub(crate) fn run_semver_check(repo_root: &Path) -> DynResult<()> {
    let spec = semver_check_spec(repo_root)?;
    prepare_semver_artifacts(repo_root)?;
    let result = run_spec(repo_root, &spec);
    let cleanup = remove_semver_artifacts(repo_root);
    result?;
    cleanup?;
    Ok(())
}

pub(crate) fn run_coverage(repo_root: &Path) -> DynResult<()> {
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

fn remove_semver_artifacts(repo_root: &Path) -> DynResult<()> {
    remove_dir_if_exists(&semver_scratch_dir(repo_root))?;
    remove_dir_if_exists(&semver_baseline_target_dir(repo_root))
}

fn prepare_semver_artifacts(repo_root: &Path) -> DynResult<()> {
    remove_semver_artifacts(repo_root)?;
    for path in semver_required_directories(repo_root) {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

fn semver_required_directories(repo_root: &Path) -> [PathBuf; 3] {
    let scratch_dir = semver_scratch_dir(repo_root);
    [
        scratch_dir.clone(),
        scratch_dir.join("debug"),
        scratch_dir.join("debug").join("deps"),
    ]
}

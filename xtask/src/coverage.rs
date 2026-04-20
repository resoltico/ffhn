use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::model::{
    BranchCoverageByFile, COVERAGE_TOOLCHAIN, CommandSpec, CoverageCounter, CoverageFailure,
    CoverageReport, CoverageSummary, DynResult, TRACKED_RELATIVE_PATHS,
};
use crate::plan::normalize_path;

/// Builds the llvm-cov command used by the coverage gate.
pub(crate) fn coverage_command(repo_root: &Path) -> CommandSpec {
    CommandSpec::new(
        "cargo",
        [
            COVERAGE_TOOLCHAIN,
            "llvm-cov",
            "--branch",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
            "--json",
            "--output-path",
            coverage_output_path(repo_root).to_string_lossy().as_ref(),
        ],
        false,
        true,
    )
}

/// Builds the cleanup command that resets llvm-cov state.
pub(crate) fn coverage_clean_command() -> CommandSpec {
    CommandSpec::new(
        "cargo",
        [COVERAGE_TOOLCHAIN, "llvm-cov", "clean", "--workspace"],
        false,
        false,
    )
}

/// Returns the coverage JSON output path.
pub(crate) fn coverage_output_path(repo_root: &Path) -> PathBuf {
    repo_root.join("target").join("coverage.json")
}

/// Loads the curated tracked-file set for the coverage gate.
pub(crate) fn tracked_files(repo_root: &Path) -> DynResult<BTreeMap<PathBuf, String>> {
    let mut tracked = BTreeMap::new();

    for relative_path in TRACKED_RELATIVE_PATHS {
        let absolute = normalize_path(repo_root, &repo_root.join(relative_path))?;
        tracked.insert(absolute, (*relative_path).to_owned());
    }

    Ok(tracked)
}

/// Evaluates one llvm-cov report against the tracked-file policy.
pub(crate) fn evaluate_coverage_report(
    repo_root: &Path,
    tracked_files: &BTreeMap<PathBuf, String>,
    report: CoverageReport,
) -> DynResult<CoverageSummary> {
    let mut coverage_by_file: BTreeMap<PathBuf, BTreeMap<u64, u64>> = BTreeMap::new();
    let mut branch_records_by_file: BranchCoverageByFile = BTreeMap::new();
    let mut branch_summary_by_file: BTreeMap<PathBuf, CoverageCounter> = BTreeMap::new();

    for data_set in report.data {
        for file in data_set.files {
            let normalized_filename = normalize_path(repo_root, &file.filename)?;
            if !tracked_files.contains_key(&normalized_filename) {
                continue;
            }

            let line_counts = coverage_by_file
                .entry(normalized_filename.clone())
                .or_default();
            for (line, _, count, _, has_count, _) in file.segments {
                if !has_count {
                    continue;
                }

                let current = line_counts.entry(line).or_insert(0);
                *current = (*current).max(count);
            }

            if !file.branches.is_empty() {
                let branch_records = branch_records_by_file
                    .entry(normalized_filename.clone())
                    .or_default();
                for (
                    start_line,
                    start_column,
                    end_line,
                    end_column,
                    first_count,
                    second_count,
                    ..,
                ) in file.branches
                {
                    let entry = branch_records
                        .entry((start_line, start_column, end_line, end_column))
                        .or_insert((0, 0));
                    entry.0 = entry.0.max(first_count);
                    entry.1 = entry.1.max(second_count);
                }
            }

            let summary = branch_summary_by_file
                .entry(normalized_filename)
                .or_default();
            summary.count = summary.count.max(file.summary.branches.count);
            summary.covered = summary.covered.max(file.summary.branches.covered);
            summary.not_covered = summary.not_covered.max(file.summary.branches.not_covered);
        }
    }

    let mut failures = Vec::new();
    let mut tracked_line_count = 0usize;
    let mut tracked_branch_count = 0usize;

    for (tracked_file, display_path) in tracked_files {
        let Some(line_counts) = coverage_by_file.get(tracked_file) else {
            failures.push(CoverageFailure {
                file: display_path.clone(),
                uncovered_lines: vec!["<no executable lines found>".to_owned()],
                uncovered_branch_count: 0,
            });
            continue;
        };

        tracked_line_count += line_counts.len();
        let uncovered_lines = line_counts
            .iter()
            .filter_map(|(line, count)| (*count == 0).then_some(line.to_string()))
            .collect::<Vec<_>>();
        let (branch_count, uncovered_branch_count) =
            if let Some(branch_records) = branch_records_by_file.get(tracked_file) {
                let branch_count = branch_records.len() * 2;
                let uncovered_branch_count = branch_records
                    .values()
                    .map(|(first_count, second_count)| {
                        usize::from(*first_count == 0) + usize::from(*second_count == 0)
                    })
                    .sum();
                (branch_count, uncovered_branch_count)
            } else {
                let summary = branch_summary_by_file
                    .get(tracked_file)
                    .copied()
                    .unwrap_or_default();
                (summary.count as usize, summary.not_covered as usize)
            };
        tracked_branch_count += branch_count;

        if !uncovered_lines.is_empty() || uncovered_branch_count > 0 {
            failures.push(CoverageFailure {
                file: display_path.clone(),
                uncovered_lines,
                uncovered_branch_count,
            });
        }
    }

    Ok(CoverageSummary {
        tracked_line_count,
        tracked_branch_count,
        failures,
    })
}

/// Reads one coverage report from disk.
pub(crate) fn read_coverage_report(path: &Path) -> DynResult<CoverageReport> {
    Ok(serde_json::from_reader(File::open(path)?)?)
}

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::model::{
    BranchCoverageByFile, CoverageCounter, CoverageFailure, CoverageReport, CoverageSummary,
    DynResult,
};

use super::source::{LineCoverage, accumulate_line_coverage, source_metadata};

/// Evaluates one llvm-cov report against the tracked-file policy.
pub(crate) fn evaluate_coverage_report(
    repo_root: &Path,
    tracked_files: &BTreeMap<PathBuf, String>,
    report: CoverageReport,
) -> DynResult<CoverageSummary> {
    let mut line_coverage_by_file: BTreeMap<PathBuf, BTreeMap<u64, LineCoverage>> = BTreeMap::new();
    let mut branch_records_by_file: BranchCoverageByFile = BTreeMap::new();
    let mut branch_summary_by_file: BTreeMap<PathBuf, CoverageCounter> = BTreeMap::new();
    let mut source_metadata_cache = BTreeMap::new();

    for data_set in report.data {
        for file in data_set.files {
            let normalized_filename = crate::plan::normalize_path(repo_root, &file.filename)?;
            if !tracked_files.contains_key(&normalized_filename) {
                continue;
            }

            let metadata = source_metadata(&mut source_metadata_cache, &normalized_filename)?;
            let line_coverage = line_coverage_by_file
                .entry(normalized_filename.clone())
                .or_default();
            accumulate_line_coverage(line_coverage, &file.segments, &metadata);

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
        let metadata = source_metadata(&mut source_metadata_cache, tracked_file)?;
        if !metadata.requires_line_coverage {
            continue;
        }
        let Some(line_coverage) = line_coverage_by_file.get(tracked_file) else {
            failures.push(CoverageFailure {
                file: display_path.clone(),
                uncovered_lines: vec!["<no executable lines found>".to_owned()],
                uncovered_branch_count: 0,
            });
            continue;
        };

        let executable_line_count = line_coverage
            .values()
            .filter(|line| line.executable)
            .count();
        if executable_line_count == 0 {
            failures.push(CoverageFailure {
                file: display_path.clone(),
                uncovered_lines: vec!["<no executable lines found>".to_owned()],
                uncovered_branch_count: 0,
            });
            continue;
        }

        tracked_line_count += executable_line_count;
        let uncovered_lines = line_coverage
            .iter()
            .filter_map(|(line, coverage)| (!coverage.covered).then_some(line.to_string()))
            .collect::<Vec<_>>();
        let (branch_count, uncovered_branch_count) = if let Some(branch_records) =
            branch_records_by_file.get(tracked_file)
        {
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
            (
                usize::try_from(summary.count).expect("branch count fits in usize"),
                usize::try_from(summary.not_covered).expect("uncovered branch count fits in usize"),
            )
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

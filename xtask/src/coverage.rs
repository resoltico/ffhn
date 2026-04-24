use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use crate::model::{
    BranchCoverageByFile, COVERAGE_TOOLCHAIN, CommandSpec, CoverageCounter, CoverageFailure,
    CoverageReport, CoverageSummary, DynResult,
};
use crate::plan::normalize_path;
use crate::repo_files::maintained_rust_source_entries;

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

/// Loads the maintained non-test Rust source files for the coverage gate.
pub(crate) fn tracked_files(repo_root: &Path) -> DynResult<BTreeMap<PathBuf, String>> {
    let mut tracked = BTreeMap::new();

    for (source_path, relative_path) in maintained_rust_source_entries(repo_root)? {
        let absolute = normalize_path(repo_root, &source_path)?;
        tracked.insert(absolute, relative_path);
    }

    Ok(tracked)
}

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
            let normalized_filename = normalize_path(repo_root, &file.filename)?;
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

#[derive(Clone, Copy, Default)]
struct LineCoverage {
    executable: bool,
    covered: bool,
}

#[derive(Clone)]
struct SourceMetadata {
    line_count: usize,
    requires_line_coverage: bool,
    lines: Vec<String>,
}

fn accumulate_line_coverage(
    line_coverage: &mut BTreeMap<u64, LineCoverage>,
    segments: &[(u64, u64, u64, bool, bool, bool)],
    metadata: &SourceMetadata,
) {
    let line_count = metadata.line_count;
    if segments.is_empty() || line_count == 0 {
        return;
    }

    let eof_segment = (line_count as u64 + 1, 1, 0, false, false, false);
    for (index, current) in segments.iter().enumerate() {
        let next = segments.get(index + 1).unwrap_or(&eof_segment);
        if !current.4 || current.5 {
            continue;
        }
        if !same_line_segment_is_executable(metadata, *current, *next) {
            continue;
        }

        if current.0 > line_count as u64 {
            continue;
        }
        let start_line = current.0;
        let end_line = covered_line_interval_end(*current, *next, line_count as u64);

        for line in start_line..=end_line {
            let coverage = line_coverage.entry(line).or_default();
            coverage.executable = true;
            coverage.covered |= current.2 > 0;
        }
    }
}

fn covered_line_interval_end(
    current: (u64, u64, u64, bool, bool, bool),
    next: (u64, u64, u64, bool, bool, bool),
    max_line: u64,
) -> u64 {
    if current.0 == next.0 {
        return current.0;
    }

    let inclusive_end = if next.1 <= 1 {
        next.0.saturating_sub(1)
    } else {
        next.0
    };
    inclusive_end.min(max_line)
}

fn same_line_segment_is_executable(
    metadata: &SourceMetadata,
    current: (u64, u64, u64, bool, bool, bool),
    next: (u64, u64, u64, bool, bool, bool),
) -> bool {
    if current.0 != next.0 {
        return true;
    }
    if current.1 >= next.1 {
        return false;
    }

    let Some(line) = metadata.lines.get(current.0.saturating_sub(1) as usize) else {
        return false;
    };
    let start = current.1.saturating_sub(1) as usize;
    let end = next.1.saturating_sub(1) as usize;
    let span = &line[start.min(line.len())..end.min(line.len())];
    span.chars().any(is_substantive_rust_span_char)
}

fn is_substantive_rust_span_char(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '"' | '\'')
}

fn source_metadata(
    cache: &mut BTreeMap<PathBuf, SourceMetadata>,
    path: &Path,
) -> DynResult<SourceMetadata> {
    if let Some(metadata) = cache.get(path).cloned() {
        return Ok(metadata);
    }

    let source = fs::read_to_string(path)?;
    let lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let metadata = SourceMetadata {
        line_count: lines.len(),
        requires_line_coverage: rust_source_requires_line_coverage(path, &source)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?,
        lines,
    };
    cache.insert(path.to_path_buf(), metadata.clone());
    Ok(metadata)
}

fn rust_source_requires_line_coverage(_path: &Path, source: &str) -> Result<bool, syn::Error> {
    let file = syn::parse_file(source)?;
    Ok(items_require_line_coverage(&file.items))
}

fn items_require_line_coverage(items: &[syn::Item]) -> bool {
    items.iter().any(item_requires_line_coverage)
}

fn item_requires_line_coverage(item: &syn::Item) -> bool {
    match item {
        syn::Item::Fn(_) => true,
        syn::Item::Impl(item) => item.items.iter().any(impl_item_requires_line_coverage),
        syn::Item::Trait(item) => item.items.iter().any(trait_item_requires_line_coverage),
        syn::Item::Mod(item) => item
            .content
            .as_ref()
            .is_some_and(|(_, items)| items_require_line_coverage(items)),
        syn::Item::Verbatim(_) => true,
        _ => false,
    }
}

fn impl_item_requires_line_coverage(item: &syn::ImplItem) -> bool {
    matches!(item, syn::ImplItem::Fn(_) | syn::ImplItem::Verbatim(_))
}

fn trait_item_requires_line_coverage(item: &syn::TraitItem) -> bool {
    match item {
        syn::TraitItem::Fn(item) => item.default.is_some(),
        syn::TraitItem::Verbatim(_) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_source_requires_line_coverage_distinguishes_contract_bodies_from_barrels() {
        assert!(
            rust_source_requires_line_coverage(Path::new("demo.rs"), "fn run() {}\n")
                .expect("parse function")
        );
        assert!(
            rust_source_requires_line_coverage(
                Path::new("demo.rs"),
                "trait Demo { fn run(&self) {} }\n"
            )
            .expect("parse trait default")
        );
        assert!(
            !rust_source_requires_line_coverage(
                Path::new("demo.rs"),
                "mod app;\npub use app::run;\n"
            )
            .expect("parse barrel")
        );
    }

    #[test]
    fn coverage_shape_helpers_handle_impls_modules_verbatim_and_zero_width_segments() {
        let impl_item =
            syn::parse_str::<syn::Item>("impl Demo { fn run(&self) {} }").expect("parse impl");
        assert!(item_requires_line_coverage(&impl_item));

        let nested_module =
            syn::parse_str::<syn::Item>("mod demo { fn run() {} }").expect("parse module");
        assert!(item_requires_line_coverage(&nested_module));

        assert!(item_requires_line_coverage(&syn::Item::Verbatim(
            Default::default()
        )));
        assert!(impl_item_requires_line_coverage(&syn::ImplItem::Verbatim(
            Default::default(),
        )));
        assert!(trait_item_requires_line_coverage(
            &syn::TraitItem::Verbatim(Default::default(),)
        ));

        let metadata = SourceMetadata {
            line_count: 1,
            requires_line_coverage: true,
            lines: vec!["    )?;".to_owned()],
        };
        let mut line_coverage = BTreeMap::new();
        accumulate_line_coverage(&mut line_coverage, &[], &metadata);
        accumulate_line_coverage(
            &mut line_coverage,
            &[
                (1, 1, 1, false, true, true),
                (1, 5, 0, false, true, false),
                (1, 7, 0, false, false, false),
                (2, 1, 1, false, true, false),
            ],
            &metadata,
        );
        assert!(line_coverage.is_empty());
        assert_eq!(
            covered_line_interval_end(
                (1, 1, 1, false, true, false),
                (1, 2, 0, false, false, false),
                2
            ),
            1
        );
        assert_eq!(
            covered_line_interval_end(
                (1, 1, 1, false, true, false),
                (2, 3, 0, false, false, false),
                2
            ),
            2
        );

        let metadata = SourceMetadata {
            line_count: 1,
            requires_line_coverage: true,
            lines: vec!["fn tracked() {}".to_owned()],
        };
        let mut out_of_range_coverage = BTreeMap::new();
        accumulate_line_coverage(
            &mut out_of_range_coverage,
            &[
                (2, 1, 1, false, true, false),
                (3, 1, 0, false, false, false),
            ],
            &metadata,
        );
        assert!(out_of_range_coverage.is_empty());

        let zero_line_metadata = SourceMetadata {
            line_count: 0,
            requires_line_coverage: true,
            lines: Vec::new(),
        };
        accumulate_line_coverage(
            &mut out_of_range_coverage,
            &[(1, 1, 1, false, true, false)],
            &zero_line_metadata,
        );
        assert!(out_of_range_coverage.is_empty());

        assert!(!same_line_segment_is_executable(
            &metadata,
            (2, 1, 0, false, true, false),
            (2, 2, 0, false, false, false),
        ));
        assert!(!trait_item_requires_line_coverage(
            &syn::parse_str::<syn::TraitItem>("fn run(&self);").expect("parse trait signature"),
        ));
        assert!(!trait_item_requires_line_coverage(
            &syn::parse_str::<syn::TraitItem>("type Output;").expect("parse trait type"),
        ));
    }
}

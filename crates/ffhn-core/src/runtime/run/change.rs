use crate::stable_json::sha256_hex;
use crate::{ChangeKind, RunChangeRegion, RunChangeSection, RunOutcome};

pub(super) fn build_change_section(
    previous: Option<&str>,
    current: &str,
    run_outcome: RunOutcome,
) -> RunChangeSection {
    let previous_lines = previous.map(split_lines).unwrap_or_default();
    let current_lines = split_lines(current);
    let common_prefix_lines = common_prefix_len(&previous_lines, &current_lines);
    let common_suffix_lines =
        common_suffix_len(&previous_lines, &current_lines, common_prefix_lines);

    let changed_region = match run_outcome {
        RunOutcome::Changed | RunOutcome::Initialized => {
            let previous_region = &previous_lines
                [common_prefix_lines..previous_lines.len().saturating_sub(common_suffix_lines)];
            let current_region = &current_lines
                [common_prefix_lines..current_lines.len().saturating_sub(common_suffix_lines)];
            Some(RunChangeRegion {
                previous_start_line: common_prefix_lines + 1,
                previous_line_count: previous_region.len(),
                current_start_line: common_prefix_lines + 1,
                current_line_count: current_region.len(),
                previous_excerpt: excerpt_from_lines(previous_region),
                current_excerpt: excerpt_from_lines(current_region),
                previous_excerpt_sha256: excerpt_from_lines(previous_region)
                    .as_deref()
                    .map(|excerpt| sha256_hex(excerpt.as_bytes())),
                current_excerpt_sha256: excerpt_from_lines(current_region)
                    .as_deref()
                    .map(|excerpt| sha256_hex(excerpt.as_bytes())),
            })
        }
        RunOutcome::Unchanged
        | RunOutcome::FailedTransient
        | RunOutcome::FailedPermanent
        | RunOutcome::SkippedDisabled => None,
    };

    RunChangeSection {
        kind: match run_outcome {
            RunOutcome::Initialized => ChangeKind::Initialized,
            RunOutcome::Changed => ChangeKind::Changed,
            RunOutcome::Unchanged => ChangeKind::Unchanged,
            RunOutcome::FailedTransient
            | RunOutcome::FailedPermanent
            | RunOutcome::SkippedDisabled => ChangeKind::Unchanged,
        },
        previous_compare_bytes: previous.map(str::len),
        current_compare_bytes: current.len(),
        previous_compare_line_count: previous.map(line_count),
        current_compare_line_count: line_count(current),
        common_prefix_lines,
        common_suffix_lines,
        changed_region,
    }
}

pub(super) fn split_lines(value: &str) -> Vec<&str> {
    if value.is_empty() {
        Vec::new()
    } else {
        value.split('\n').collect()
    }
}

fn line_count(value: &str) -> usize {
    split_lines(value).len()
}

pub(super) fn common_prefix_len(previous: &[&str], current: &[&str]) -> usize {
    previous
        .iter()
        .zip(current.iter())
        .take_while(|(left, right)| left == right)
        .count()
}

pub(super) fn common_suffix_len(
    previous: &[&str],
    current: &[&str],
    common_prefix_len: usize,
) -> usize {
    let max_suffix = previous
        .len()
        .min(current.len())
        .saturating_sub(common_prefix_len);
    let mut suffix = 0;
    while suffix < max_suffix
        && previous[previous.len() - 1 - suffix] == current[current.len() - 1 - suffix]
    {
        suffix += 1;
    }
    suffix
}

pub(super) fn excerpt_from_lines(lines: &[&str]) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    let mut excerpt = lines.iter().take(4).copied().collect::<Vec<_>>().join("\n");
    if lines.len() > 4 {
        excerpt.push_str("\n...");
    }
    if excerpt.len() > 240 {
        excerpt.truncate(240);
        excerpt.push_str("...");
    }
    Some(excerpt)
}

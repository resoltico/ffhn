//! Executable repository artifact-hygiene policy.

mod cleanup;
mod filesystem;
mod layout;
mod report;
mod types;

pub(crate) type HygieneCleanMode = types::HygieneCleanMode;
pub(crate) type HygieneCleanResult = types::HygieneCleanResult;
#[cfg(test)]
pub(crate) type HygieneEntry = types::HygieneEntry;
pub(crate) type HygieneReport = types::HygieneReport;
pub(crate) type HygieneReportFormat = types::HygieneReportFormat;
#[cfg(test)]
pub(crate) type HygieneViolation = types::HygieneViolation;

/// Prepares the managed artifact roots for one command layout and returns the env paths to use.
pub fn prepare_artifact_layout(
    repo_root: &std::path::Path,
    layout: crate::model::CommandArtifactLayout,
) -> crate::model::DynResult<Option<(std::path::PathBuf, std::path::PathBuf)>> {
    layout::prepare_artifact_layout(repo_root, layout)
}

/// Builds a full repository artifact report.
pub fn hygiene_report(repo_root: &std::path::Path) -> crate::model::DynResult<HygieneReport> {
    report::hygiene_report(repo_root)
}

/// Renders the report as human-readable text.
pub fn render_hygiene_report(report: &HygieneReport) -> String {
    report::render_hygiene_report(report)
}

/// Fails when the repository violates the maintained hygiene policy.
pub fn ensure_hygiene(repo_root: &std::path::Path) -> crate::model::DynResult<()> {
    report::ensure_hygiene(repo_root)
}

/// Removes disposable artifact roots according to the requested cleanup mode.
pub fn clean_hygiene(
    repo_root: &std::path::Path,
    mode: HygieneCleanMode,
) -> crate::model::DynResult<HygieneCleanResult> {
    cleanup::clean_hygiene(repo_root, mode)
}

#[cfg(test)]
pub(crate) fn aggregate_entry_for_tests(
    path: &std::path::Path,
    roots: &[std::path::PathBuf],
) -> crate::model::DynResult<HygieneEntry> {
    report::aggregate_entry(
        "test-aggregate",
        "test-aggregate",
        path,
        roots,
        Some(0),
        false,
        true,
    )
}

#[cfg(test)]
pub(crate) fn dir_size_bytes_for_tests(path: &std::path::Path) -> u64 {
    filesystem::dir_size_bytes(path).expect("dir size bytes")
}

#[cfg(test)]
pub(crate) fn dir_size_bytes_result_for_tests(
    path: &std::path::Path,
) -> crate::model::DynResult<u64> {
    filesystem::dir_size_bytes(path)
}

#[cfg(all(test, unix))]
pub(crate) fn entry_from_path_for_tests(
    path: &std::path::Path,
) -> crate::model::DynResult<HygieneEntry> {
    report::entry_from_path(
        "test-entry",
        "test-entry",
        path,
        Some(0),
        true,
        true,
        Vec::new(),
    )
}

#[cfg(test)]
pub(crate) fn format_bytes_for_tests(bytes: u64) -> String {
    filesystem::format_bytes(bytes)
}

#[cfg(test)]
pub(crate) fn looks_like_cargo_target_dir_for_tests(path: &std::path::Path) -> bool {
    filesystem::looks_like_cargo_target_dir(path)
}

#[cfg(test)]
pub(crate) fn missing_managed_markers_for_tests(path: &std::path::Path) -> Vec<String> {
    filesystem::missing_managed_markers(path)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
pub(crate) fn report_violations_for_tests(entries: &[HygieneEntry]) -> Vec<HygieneViolation> {
    report::report_violations(entries)
}

#[cfg(all(test, unix))]
pub(crate) fn repo_tmp_cargo_roots_for_tests(
    repo_root: &std::path::Path,
) -> crate::model::DynResult<Vec<std::path::PathBuf>> {
    filesystem::repo_tmp_cargo_roots(repo_root)
}

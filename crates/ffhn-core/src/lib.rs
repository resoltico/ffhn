//! Core FFHN library implementing the FFHN monitoring engine.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod canonical;
mod contract;
mod error;
mod fetch;
mod model;
mod paths;
mod runtime;
mod stable_json;
mod time;

pub use canonical::apply_canonicalizers;
pub use contract::{
    CLI_ARGUMENT_ALL_ID, CLI_ARGUMENT_DRY_RUN_ID, CLI_ARGUMENT_FORMAT_ID, CLI_ARGUMENT_JOBS_ID,
    CLI_ARGUMENT_TARGET_ID, CLI_ARGUMENT_WATCH_ROOT_ID, CLI_INVOCATION_RUN_ALL_ID,
    CLI_INVOCATION_RUN_BATCH_ID, CLI_INVOCATION_RUN_SINGLE_ID, CLI_INVOCATION_STATUS_ID,
    CLI_LIMIT_DURABLE_TARGET_IDS_ID, CLI_LIMIT_IMMEDIATE_DISCOVERY_DEPTH_ID,
    CLI_LIMIT_POSITIVE_BATCH_CONCURRENCY_ID, CLI_LIMIT_RUN_TARGET_SELECTION_ID,
    CLI_LIMIT_UNIQUE_TARGET_IDS_ID, CLI_OPERATION_RUN_ID, CLI_OPERATION_STATUS_ID,
    CliArgumentContract, CliArgumentValueKind, CliContractCatalog, CliHardLimitContract,
    CliInvocationContract, CliOperationContract, ExecutionModeContract, UserFacingDocumentContract,
    batch_run_report_write_error, cli_contract, cli_document, cli_execution_mode, cli_hard_limit,
    cli_operation, document_write_error, duplicate_target_ids_usage_error,
    positive_batch_concurrency_limit, positive_batch_concurrency_usage_error, run_operation,
    run_report_document, run_report_write_error, run_target_selection_usage_error,
    status_operation, status_report_document, status_report_write_error, unique_target_ids_limit,
};
pub use error::CoreError;
#[cfg(test)]
pub(crate) use model::NotificationAdapter;
#[cfg(test)]
pub(crate) use model::NotificationEndpoint;
pub use model::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, BaselinePhase,
    BatchOutcomeCounts, BatchRunEntry, BatchRunEntryView, BatchRunReport, BatchRunReportInput,
    CanonicalizerKind, CanonicalizerSpec, ChangeKind, CompareBasis, CompareConfigView,
    CssSelectorSelectionView, DelimiterMode, DelimiterPairSelectionView,
    EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION, ExtractionRecord,
    FailedRunReportView, FailureClass, FetchConfigView, FetchEngine, FileFetchConfigView,
    FileTargetSourceView, HttpFetchConfigView, HttpMethod, HttpTargetSourceView,
    LAST_RUN_SNAPSHOT_SCHEMA_NAME, LAST_RUN_SNAPSHOT_SCHEMA_VERSION, LastRunRecord,
    LastRunSnapshot, NOTIFICATION_PAYLOAD_SCHEMA_NAME, NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
    NotificationDeliveryStatus, NotificationPayload, NotificationRouteView, OutputKind,
    PersistWriteStatus, ProcessErrorDetail, ProcessErrorKind, RUN_REPORT_SCHEMA_NAME,
    RUN_REPORT_SCHEMA_VERSION, RegexFlag, RelativeArtifactPath, ReportableRunBodyView, RunBodyView,
    RunChangeRegion, RunChangeSection, RunCompareView, RunExtractionSection, RunFailureCause,
    RunFetchView, RunMode, RunNotificationDeliveryView, RunOutcome, RunPersistView, RunReport,
    RunResult, STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION, STATUS_REPORT_SCHEMA_NAME,
    STATUS_REPORT_SCHEMA_VERSION, SelectionConfigView, SelectionEvidence, SelectionKind,
    SelectionMatch, SelectionModeView, SelectionRange, SnapshotDigestSummary, SnapshotReference,
    SnapshotSlot, StateDocument, StatusReport, StatusSummary, StoredBaseline,
    SuccessfulRunReportView, TARGET_SCHEMA_NAME, TARGET_SCHEMA_VERSION, TargetDocument, TargetId,
    TargetSourceView, WhitespaceMode,
};
#[cfg(any(test, doctest))]
pub(crate) use model::{CompareConfig, FetchConfig, TargetSource};
pub(crate) use model::{
    FileFetchConfig, NetworkFetchConfig, NotificationDeliveryOutcome, NotificationRoute,
    RunCompareSection, RunFetchSection, RunNotificationDelivery, RunPersistSection,
    SelectionConfig, SelectionModeConfig,
};
pub use paths::TargetPaths;

/// Validates one target document loaded from disk.
///
/// # Errors
///
/// Returns [`CoreError`] when FFHN cannot validate the watch root directory, cannot read the
/// target directory, the target document is not valid TOML, or the decoded document violates
/// FFHN's schema and identity invariants.
pub fn validate_target(paths: &TargetPaths) -> Result<TargetDocument, CoreError> {
    runtime::validate_target(paths)
}

/// Produces one machine-readable FFHN status report.
///
/// # Errors
///
/// Returns [`CoreError`] when FFHN cannot validate the watch root directory, cannot read the
/// target directory, or hits a genuine shared-lock filesystem error while preparing one stable
/// status view.
pub fn status(paths: &TargetPaths) -> Result<StatusReport, CoreError> {
    runtime::status(paths)
}

/// Executes one FFHN run and returns the structured run report.
///
/// # Errors
///
/// Returns [`CoreError`] only for process-level failures before FFHN can emit a structured run
/// report, such as watch-root validation failures or filesystem / serialization failures around
/// timestamps, locks, or persistence.
pub fn run_once(paths: &TargetPaths) -> Result<RunReport, CoreError> {
    runtime::run_once(paths)
}

/// Executes one FFHN run in dry-run mode without mutating persisted state artifacts or reports.
///
/// # Errors
///
/// Returns [`CoreError`] only for process-level failures before FFHN can emit a structured run
/// report, such as watch-root validation failures or genuine shared-lock filesystem errors.
pub fn run_once_dry_run(paths: &TargetPaths) -> Result<RunReport, CoreError> {
    runtime::run_once_with_options(paths, runtime::RunOptions::DRY_RUN)
}

/// Executes multiple FFHN targets and returns one aggregate batch report.
///
/// # Errors
///
/// Returns [`CoreError`] when batch orchestration itself cannot produce the aggregate report, when
/// `jobs` is zero, or when `targets` contains duplicate target ids.
pub fn run_batch(
    watch_root: &std::path::Path,
    targets: &[TargetId],
    run_mode: RunMode,
    jobs: usize,
) -> Result<BatchRunReport, CoreError> {
    runtime::run_batch(
        watch_root,
        targets,
        runtime::RunOptions { mode: run_mode },
        jobs,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn validate_target_uses_the_public_library_entrypoint() {
        let temp = tempdir().expect("tempdir");
        let watch_root = temp.path().join("watchlist");
        let target_dir = watch_root.join("demo");
        fs::create_dir_all(&target_dir).expect("create target dir");

        fs::write(
            target_dir.join("target.toml"),
            r#"
schema_name = "ffhn.target"
schema_version = 3
target_id = "demo"
display_name = "Demo"
enabled = true

[target]
kind = "http"
source_url = "https://example.com"

[fetch]
engine = "http"
method = "GET"
timeout_ms = 15000
max_bytes = 2000000
user_agent = "ffhn/test"
follow_redirects = true
accept = "text/html"

[selection]
kind = "css_selector"
selector = "main"
match = "single"
output = "outer_html"
whitespace = "normalize"
rewrite_urls = false

[compare]
basis = "canonical_text_sha256"

[[compare.canonicalization]]
kind = "trim"
"#,
        )
        .expect("write target");

        let paths = TargetPaths::try_new(&watch_root, "demo").expect("target paths");
        let target = validate_target(&paths).expect("validate target");
        assert_eq!(target.target_id(), "demo");
        assert_eq!(target.display_name(), "Demo");
    }
}

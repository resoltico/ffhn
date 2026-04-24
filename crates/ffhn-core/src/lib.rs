//! Core FFHN library implementing the FFHN monitoring engine.
#![deny(missing_docs)]

mod canonical;
mod contract;
mod error;
mod fetch;
mod model;
mod paths;
mod runtime;
mod stable_json;

pub use canonical::apply_canonicalizers;
pub use contract::{
    CLI_ARGUMENT_ALL_ID, CLI_ARGUMENT_DRY_RUN_ID, CLI_ARGUMENT_JOBS_ID, CLI_ARGUMENT_TARGET_ID,
    CLI_ARGUMENT_WATCH_ROOT_ID, CLI_INVOCATION_RUN_ALL_ID, CLI_INVOCATION_RUN_BATCH_ID,
    CLI_INVOCATION_RUN_SINGLE_ID, CLI_INVOCATION_STATUS_ID, CLI_LIMIT_DURABLE_TARGET_IDS_ID,
    CLI_LIMIT_IMMEDIATE_DISCOVERY_DEPTH_ID, CLI_LIMIT_POSITIVE_BATCH_CONCURRENCY_ID,
    CLI_LIMIT_UNIQUE_TARGET_IDS_ID, CLI_OPERATION_RUN_ID, CLI_OPERATION_STATUS_ID,
    CliArgumentContract, CliArgumentValueKind, CliContractCatalog, CliHardLimitContract,
    CliInvocationContract, CliOperationContract, ExecutionModeContract, UserFacingDocumentContract,
    batch_run_report_write_error, cli_contract, cli_document, cli_execution_mode, cli_hard_limit,
    cli_operation, document_write_error, duplicate_target_ids_usage_error,
    positive_batch_concurrency_limit, positive_batch_concurrency_usage_error, run_operation,
    run_report_document, run_report_write_error, status_operation, status_report_document,
    status_report_write_error, unique_target_ids_limit,
};
pub use error::CoreError;
pub use model::{
    ArtifactStatus, BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION,
    BatchOutcomeCounts, BatchRunEntry, BatchRunReport, BatchRunReportInput, CanonicalizerKind,
    CanonicalizerSpec, ChangeKind, CompareBasis, CompareConfig, DelimiterMode,
    EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION, ExtractionRecord,
    FailureClass, FetchConfig, FetchEngine, HTMLCUT_INTEROP_PROFILE, HttpMethod,
    NOTIFICATION_PAYLOAD_SCHEMA_NAME, NOTIFICATION_PAYLOAD_SCHEMA_VERSION, NotificationEvent,
    NotificationHook, NotificationPayload, OutputKind, ProcessErrorDetail, ProcessErrorKind,
    RUN_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_VERSION, ReasonCode, RegexFlag, RelativeArtifactPath,
    RunChangeRegion, RunChangeSection, RunCompareSection, RunExtractionSection, RunFetchSection,
    RunMode, RunNotificationDelivery, RunOutcome, RunPersistSection, RunReport, STATE_SCHEMA_NAME,
    STATE_SCHEMA_VERSION, STATUS_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_VERSION, SelectionConfig,
    SelectionKind, SelectionMatch, SnapshotDigestSummary, SnapshotReference, SnapshotSlot,
    StateDocument, StatePhase, StatusReport, StorageConfig, TARGET_SCHEMA_NAME,
    TARGET_SCHEMA_VERSION, TargetDocument, TargetId, TargetSource, TargetStatus, WhitespaceMode,
};
pub use paths::TargetPaths;

/// Validates one target document loaded from disk.
///
/// # Errors
///
/// Returns [`CoreError`] when the target directory cannot be read, the target document is not
/// valid TOML, or the decoded document violates FFHN's schema and identity invariants.
pub fn validate_target(paths: &TargetPaths) -> Result<TargetDocument, CoreError> {
    runtime::validate_target(paths)
}

/// Produces one machine-readable FFHN status report.
///
/// # Errors
///
/// Returns [`CoreError`] when FFHN cannot read the target directory or acquire the shared status
/// lock required to inspect a valid target.
pub fn status(paths: &TargetPaths) -> Result<StatusReport, CoreError> {
    runtime::status(paths)
}

/// Executes one FFHN run and returns the structured run report.
///
/// # Errors
///
/// Returns [`CoreError`] only for process-level failures before FFHN can emit a structured run
/// report, such as filesystem or serialization failures around timestamps, locks, or persistence.
pub fn run_once(paths: &TargetPaths) -> Result<RunReport, CoreError> {
    runtime::run_once(paths)
}

/// Executes one FFHN run in dry-run mode without mutating persisted state artifacts or reports.
///
/// # Errors
///
/// Returns [`CoreError`] only for process-level failures before FFHN can emit a structured run
/// report, including shared-lock filesystem failures on otherwise valid targets.
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

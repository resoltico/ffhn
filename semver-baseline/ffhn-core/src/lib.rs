//! Core FFHN library implementing the v2.0.0 Rust rewrite.
#![deny(missing_docs)]

mod canonical;
mod error;
mod fetch;
mod model;
mod paths;
mod runtime;
mod stable_json;

pub use canonical::apply_canonicalizers;
pub use error::CoreError;
pub use model::{
    ArtifactStatus, BatchOutcomeCounts, BatchRunEntry, BatchRunReport,
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, CanonicalizerKind,
    CanonicalizerSpec, ChangeKind, CompareBasis, CompareConfig, DelimiterMode,
    EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION, ExtractionRecord,
    FailureClass, FetchConfig, FetchEngine, HTMLCUT_INTEROP_PROFILE, HttpMethod,
    NotificationEvent, NotificationHook, OutputKind, RUN_REPORT_SCHEMA_NAME,
    RUN_REPORT_SCHEMA_VERSION, ReasonCode, RegexFlag, RunChangeRegion, RunChangeSection,
    RunCompareSection, RunExtractionSection, RunFetchSection, RunNotificationDelivery, RunMode,
    RunOutcome, RunPersistSection, RunReport, STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION,
    STATUS_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_VERSION, SelectionConfig, SelectionKind,
    SelectionMatch, SnapshotDigestSummary, SnapshotReference, SnapshotSlot, StateDocument,
    StatePhase, StatusReport, StorageConfig, TARGET_SCHEMA_NAME, TARGET_SCHEMA_VERSION,
    TargetDocument, TargetSource, TargetStatus, WhitespaceMode,
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
/// Returns [`CoreError`] when FFHN cannot read the target directory, acquire the shared status
/// lock, or serialize timestamped report data after validating the target and state inputs.
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

/// Executes one FFHN run in dry-run mode without mutating persistent state.
///
/// # Errors
///
/// Returns [`CoreError`] only for process-level failures before FFHN can emit a structured run
/// report.
pub fn run_once_dry_run(paths: &TargetPaths) -> Result<RunReport, CoreError> {
    runtime::run_once_with_options(paths, runtime::RunOptions::DRY_RUN)
}

/// Executes multiple FFHN targets and returns one aggregate batch report.
///
/// # Errors
///
/// Returns [`CoreError`] when batch orchestration itself cannot produce the aggregate report.
pub fn run_batch(
    watch_root: &std::path::Path,
    targets: &[String],
    run_mode: RunMode,
    jobs: usize,
) -> Result<BatchRunReport, CoreError> {
    runtime::run_batch(watch_root, targets, runtime::RunOptions { mode: run_mode }, jobs)
}

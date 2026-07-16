//! Core FFHN v2 measurement foundation.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod contract;
mod error;
mod model;
mod paths;
mod runtime;
mod stable_json;

#[cfg(test)]
mod test_support;

pub use contract::{
    CLI_ARGUMENT_ALL_ID, CLI_ARGUMENT_DRY_RUN_ID, CLI_ARGUMENT_FORMAT_ID, CLI_ARGUMENT_JOBS_ID,
    CLI_ARGUMENT_TARGET_ID, CLI_ARGUMENT_WATCH_ROOT_ID, CLI_OPERATION_RESET_ID,
    CLI_OPERATION_RUN_ID, CLI_OPERATION_STATUS_ID, CliArgumentContract, CliArgumentValueKind,
    CliContractCatalog, CliHardLimitContract, CliInvocationContract, CliOperationContract,
    ExecutionModeContract, UserFacingDocumentContract, cli_contract, cli_operation,
    duplicate_target_ids_usage_error, positive_batch_concurrency_usage_error, reset_operation,
    run_operation, run_target_selection_usage_error, status_operation,
};
pub use error::CoreError;
pub use model::{
    AcquisitionKind, BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, BatchRunReport,
    Condition, ConditionContext, ConditionEvaluation, ConditionId, ConditionIssue,
    ConditionOutcome, ConditionPredicate, ConditionReference, DeclaredType, DeliveryAdapter,
    DeliveryOutcome, DeliveryRoute, DeliveryStatus, FetchConfig, FetchEngine, HtmlSelection,
    HtmlcutDiagnostic, HtmlcutFailureDetails, HttpMethod, NumericLocale, Observation,
    OnRunEventCause, OutboxOverflow, OutboxPolicy, PARSER_GRAMMAR_VERSION, PARSER_ID,
    PermanentErrorCode, PolicyRunInput, ProcessErrorDetail, ProcessErrorKind, Projection,
    RESET_REPORT_SCHEMA_NAME, RESET_REPORT_SCHEMA_VERSION, RUN_REPORT_SCHEMA_NAME,
    RUN_REPORT_SCHEMA_VERSION, ResetReport, RouteFamily, RouteId, RunMode, RunOutcome, RunReport,
    STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION, STATUS_REPORT_SCHEMA_NAME,
    STATUS_REPORT_SCHEMA_VERSION, SourceSuspectReason, StagedEventEligibility, StagedPolicyRun,
    StateDocument, StatusKind, StatusReport, TARGET_SCHEMA_NAME, TARGET_SCHEMA_VERSION,
    TargetDocument, TargetId, TargetSource, ThresholdDirection, TypeParams,
};
pub use paths::TargetPaths;

/// Validates a v2 target definition loaded from its watch-root directory.
pub fn validate_target(paths: &TargetPaths) -> Result<TargetDocument, CoreError> {
    runtime::validate_target(paths)
}
/// Executes one live v2 measurement run.
pub fn run_once(paths: &TargetPaths) -> Result<RunReport, CoreError> {
    runtime::run_once(paths)
}
/// Executes one v2 measurement run without writing state.
pub fn run_once_dry_run(paths: &TargetPaths) -> Result<RunReport, CoreError> {
    runtime::run_once_with_mode(paths, RunMode::DryRun)
}
/// Executes multiple targets with bounded parallelism.
pub fn run_batch(
    watch_root: &std::path::Path,
    targets: &[TargetId],
    run_mode: RunMode,
    jobs: usize,
) -> Result<BatchRunReport, CoreError> {
    let paths = targets
        .iter()
        .map(|target| TargetPaths::try_new(watch_root, target.as_str()))
        .collect::<Result<Vec<_>, _>>()?;
    runtime::run_batch(paths, run_mode, jobs)
}
/// Returns v2 status without mutating v2 state.
pub fn status(paths: &TargetPaths) -> Result<StatusReport, CoreError> {
    runtime::status(paths)
}
/// Blindly deletes the target's isolated v2 storage root under the target lock.
pub fn reset(paths: &TargetPaths) -> Result<ResetReport, CoreError> {
    runtime::reset(paths)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn public_v2_entrypoints_preserve_the_typed_measurement_lifecycle() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let watch_root = temporary.path().join("watchlist");
        let paths = TargetPaths::try_new(&watch_root, "demo").expect("target paths");
        fs::create_dir_all(paths.target_dir()).expect("target directory");
        let source = paths.target_dir().join("source.json");
        fs::write(&source, r#"{"value":7}"#).expect("source");
        fs::write(
            paths.target_file(),
            format!(
                "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = \"{}\"\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
                source.display()
            ),
        )
        .expect("target");

        assert_eq!(
            validate_target(&paths).expect("validate").target_id(),
            "demo"
        );
        assert_eq!(
            run_once_dry_run(&paths).expect("dry run").outcome(),
            RunOutcome::Initialized
        );
        assert_eq!(
            run_once(&paths).expect("live run").outcome(),
            RunOutcome::Initialized
        );
        assert_eq!(
            status(&paths).expect("ready status").kind(),
            StatusKind::Ready
        );
        let target = TargetId::new("demo").expect("target id");
        assert_eq!(
            run_batch(&watch_root, &[target], RunMode::DryRun, 1)
                .expect("batch")
                .reports()
                .len(),
            1
        );
        assert_eq!(
            serde_json::to_value(reset(&paths).expect("reset")).expect("reset document")["storage_cleared"],
            true
        );
        assert!(
            CoreError::internal("invariant gap")
                .to_string()
                .contains("invariant gap")
        );
    }
}

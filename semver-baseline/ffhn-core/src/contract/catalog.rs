use crate::{
    BATCH_RUN_REPORT_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_NAME, RUN_REPORT_SCHEMA_NAME, RunMode,
    STATE_SCHEMA_NAME, STATUS_REPORT_SCHEMA_NAME, TARGET_SCHEMA_NAME,
};

use super::types::{
    CliArgumentContract, CliArgumentValueKind, CliContractCatalog, CliHardLimitContract,
    CliInvocationContract, CliOperationContract, ExecutionModeContract, UserFacingDocumentContract,
};

/// Canonical CLI operation id for `ffhn run`.
pub const CLI_OPERATION_RUN_ID: &str = "run";
/// Canonical CLI operation id for `ffhn status`.
pub const CLI_OPERATION_STATUS_ID: &str = "status";

/// Canonical CLI argument id for `--watch-root`.
pub const CLI_ARGUMENT_WATCH_ROOT_ID: &str = "watch_root";
/// Canonical CLI argument id for repeated `--target`.
pub const CLI_ARGUMENT_TARGET_ID: &str = "target";
/// Canonical CLI argument id for `--all`.
pub const CLI_ARGUMENT_ALL_ID: &str = "all";
/// Canonical CLI argument id for `--jobs`.
pub const CLI_ARGUMENT_JOBS_ID: &str = "jobs";
/// Canonical CLI argument id for `--dry-run`.
pub const CLI_ARGUMENT_DRY_RUN_ID: &str = "dry_run";

/// Canonical CLI hard-limit id for positive batch concurrency.
pub const CLI_LIMIT_POSITIVE_BATCH_CONCURRENCY_ID: &str = "positive_batch_concurrency";
/// Canonical CLI hard-limit id for unique repeated `--target` values.
pub const CLI_LIMIT_UNIQUE_TARGET_IDS_ID: &str = "unique_target_ids";
/// Canonical CLI hard-limit id for immediate watch-root discovery depth.
pub const CLI_LIMIT_IMMEDIATE_DISCOVERY_DEPTH_ID: &str = "immediate_discovery_depth";

/// Canonical invocation id for `ffhn run --target <id>`.
pub const CLI_INVOCATION_RUN_SINGLE_ID: &str = "run_single_target";
/// Canonical invocation id for `ffhn run --target <a> --target <b>`.
pub const CLI_INVOCATION_RUN_BATCH_ID: &str = "run_explicit_batch";
/// Canonical invocation id for `ffhn run --all`.
pub const CLI_INVOCATION_RUN_ALL_ID: &str = "run_discovered_batch";
/// Canonical invocation id for `ffhn status --target <id>`.
pub const CLI_INVOCATION_STATUS_ID: &str = "status_single_target";

const RUN_ARGUMENTS: &[CliArgumentContract] = &[
    CliArgumentContract {
        id: CLI_ARGUMENT_WATCH_ROOT_ID,
        long_name: "watch-root",
        display_label: "Watch Root",
        value_name: Some("PATH"),
        help_summary: "Watch-root directory containing per-target subdirectories.",
        value_kind: CliArgumentValueKind::Path,
        repeatable: false,
        required: false,
        required_unless_present: None,
        conflicts_with: &[],
        default_value: Some("watchlist"),
    },
    CliArgumentContract {
        id: CLI_ARGUMENT_TARGET_ID,
        long_name: "target",
        display_label: "Target",
        value_name: Some("ID"),
        help_summary: "One or more target ids under the watch root.",
        value_kind: CliArgumentValueKind::String,
        repeatable: true,
        required: false,
        required_unless_present: Some(CLI_ARGUMENT_ALL_ID),
        conflicts_with: &[],
        default_value: None,
    },
    CliArgumentContract {
        id: CLI_ARGUMENT_ALL_ID,
        long_name: "all",
        display_label: "All",
        value_name: None,
        help_summary: "Run every target directory discovered under the watch root.",
        value_kind: CliArgumentValueKind::Flag,
        repeatable: false,
        required: false,
        required_unless_present: None,
        conflicts_with: &[CLI_ARGUMENT_TARGET_ID],
        default_value: None,
    },
    CliArgumentContract {
        id: CLI_ARGUMENT_JOBS_ID,
        long_name: "jobs",
        display_label: "Jobs",
        value_name: Some("N"),
        help_summary: "Maximum concurrent target runs.",
        value_kind: CliArgumentValueKind::PositiveInteger,
        repeatable: false,
        required: false,
        required_unless_present: None,
        conflicts_with: &[],
        default_value: Some("1"),
    },
    CliArgumentContract {
        id: CLI_ARGUMENT_DRY_RUN_ID,
        long_name: "dry-run",
        display_label: "Dry Run",
        value_name: None,
        help_summary: "Run validation, fetch, extraction, and comparison under the shared run lock without live state/report mutations.",
        value_kind: CliArgumentValueKind::Flag,
        repeatable: false,
        required: false,
        required_unless_present: None,
        conflicts_with: &[],
        default_value: None,
    },
];

const STATUS_ARGUMENTS: &[CliArgumentContract] = &[
    CliArgumentContract {
        id: CLI_ARGUMENT_WATCH_ROOT_ID,
        long_name: "watch-root",
        display_label: "Watch Root",
        value_name: Some("PATH"),
        help_summary: "Watch-root directory containing per-target subdirectories.",
        value_kind: CliArgumentValueKind::Path,
        repeatable: false,
        required: false,
        required_unless_present: None,
        conflicts_with: &[],
        default_value: Some("watchlist"),
    },
    CliArgumentContract {
        id: CLI_ARGUMENT_TARGET_ID,
        long_name: "target",
        display_label: "Target",
        value_name: Some("ID"),
        help_summary: "Target id under the watch root.",
        value_kind: CliArgumentValueKind::String,
        repeatable: false,
        required: true,
        required_unless_present: None,
        conflicts_with: &[],
        default_value: None,
    },
];

const RUN_INVOCATIONS: &[CliInvocationContract] = &[
    CliInvocationContract {
        id: CLI_INVOCATION_RUN_SINGLE_ID,
        operation_id: CLI_OPERATION_RUN_ID,
        usage: "ffhn run --target <id>",
        output_document_id: RUN_REPORT_SCHEMA_NAME,
        analysis_summary: "single-target execution",
    },
    CliInvocationContract {
        id: CLI_INVOCATION_RUN_BATCH_ID,
        operation_id: CLI_OPERATION_RUN_ID,
        usage: "ffhn run --target <a> --target <b>",
        output_document_id: BATCH_RUN_REPORT_SCHEMA_NAME,
        analysis_summary: "explicit multi-target batch",
    },
    CliInvocationContract {
        id: CLI_INVOCATION_RUN_ALL_ID,
        operation_id: CLI_OPERATION_RUN_ID,
        usage: "ffhn run --all",
        output_document_id: BATCH_RUN_REPORT_SCHEMA_NAME,
        analysis_summary: "watch-root discovery",
    },
];

const STATUS_INVOCATIONS: &[CliInvocationContract] = &[CliInvocationContract {
    id: CLI_INVOCATION_STATUS_ID,
    operation_id: CLI_OPERATION_STATUS_ID,
    usage: "ffhn status --target <id>",
    output_document_id: STATUS_REPORT_SCHEMA_NAME,
    analysis_summary: "status inspection; valid targets may create `lock/run.lock`",
}];

const OPERATIONS: &[CliOperationContract] = &[
    CliOperationContract {
        id: CLI_OPERATION_RUN_ID,
        display_label: "Run",
        help_summary: "Run one or more configured targets once.",
        arguments: RUN_ARGUMENTS,
        invocations: RUN_INVOCATIONS,
    },
    CliOperationContract {
        id: CLI_OPERATION_STATUS_ID,
        display_label: "Status",
        help_summary: "Read one target's current machine-readable status.",
        arguments: STATUS_ARGUMENTS,
        invocations: STATUS_INVOCATIONS,
    },
];

const EXECUTION_MODES: &[ExecutionModeContract] = &[
    ExecutionModeContract {
        mode: RunMode::Live,
        id: "live",
        display_label: "Live",
        summary: "Validation, fetch, extraction, comparison, persistence, and notifications.",
        writes_state: true,
        writes_last_run: true,
        delivers_notifications: true,
    },
    ExecutionModeContract {
        mode: RunMode::DryRun,
        id: "dry_run",
        display_label: "Dry Run",
        summary: "Validation, fetch, extraction, and comparison under the shared run lock without live state/report mutations.",
        writes_state: false,
        writes_last_run: false,
        delivers_notifications: false,
    },
];

const HARD_LIMITS: &[CliHardLimitContract] = &[
    CliHardLimitContract {
        id: CLI_LIMIT_POSITIVE_BATCH_CONCURRENCY_ID,
        operation_id: Some(CLI_OPERATION_RUN_ID),
        display_label: "Positive Batch Concurrency",
        summary: "`--jobs` must be a positive integer; `0` is invalid CLI usage.",
        cli_usage_error_template: "must be a positive integer",
    },
    CliHardLimitContract {
        id: CLI_LIMIT_UNIQUE_TARGET_IDS_ID,
        operation_id: Some(CLI_OPERATION_RUN_ID),
        display_label: "Unique Explicit Target Ids",
        summary: "Repeated `--target` values must be unique within one request.",
        cli_usage_error_template: "duplicate --target values are not allowed: {detail}",
    },
    CliHardLimitContract {
        id: CLI_LIMIT_IMMEDIATE_DISCOVERY_DEPTH_ID,
        operation_id: Some(CLI_OPERATION_RUN_ID),
        display_label: "Immediate Watch-Root Discovery",
        summary: "`--all` only discovers immediate subdirectories of the watch root.",
        cli_usage_error_template: "`--all` only discovers immediate subdirectories of the watch root",
    },
];

const DOCUMENTS: &[UserFacingDocumentContract] = &[
    UserFacingDocumentContract {
        id: TARGET_SCHEMA_NAME,
        display_label: "Target Document",
    },
    UserFacingDocumentContract {
        id: EXTRACTION_RECORD_SCHEMA_NAME,
        display_label: "Extraction Record",
    },
    UserFacingDocumentContract {
        id: STATE_SCHEMA_NAME,
        display_label: "State Document",
    },
    UserFacingDocumentContract {
        id: RUN_REPORT_SCHEMA_NAME,
        display_label: "Run Report",
    },
    UserFacingDocumentContract {
        id: BATCH_RUN_REPORT_SCHEMA_NAME,
        display_label: "Batch Run Report",
    },
    UserFacingDocumentContract {
        id: STATUS_REPORT_SCHEMA_NAME,
        display_label: "Status Report",
    },
];

const CLI_CONTRACT: CliContractCatalog = CliContractCatalog {
    operations: OPERATIONS,
    execution_modes: EXECUTION_MODES,
    hard_limits: HARD_LIMITS,
    documents: DOCUMENTS,
};

/// Returns the canonical FFHN CLI contract catalog.
pub fn cli_contract() -> &'static CliContractCatalog {
    &CLI_CONTRACT
}

/// Looks up one canonical CLI operation by id.
pub fn cli_operation(id: &str) -> Option<&'static CliOperationContract> {
    cli_contract()
        .operations
        .iter()
        .find(|operation| operation.id == id)
}

/// Looks up one canonical execution mode by runtime enum.
pub fn cli_execution_mode(mode: RunMode) -> &'static ExecutionModeContract {
    match mode {
        RunMode::Live => &EXECUTION_MODES[0],
        RunMode::DryRun => &EXECUTION_MODES[1],
    }
}

/// Looks up one canonical CLI hard limit by id.
pub fn cli_hard_limit(id: &str) -> Option<&'static CliHardLimitContract> {
    cli_contract()
        .hard_limits
        .iter()
        .find(|limit| limit.id == id)
}

/// Looks up one registered user-facing document id.
pub fn cli_document(id: &str) -> Option<&'static UserFacingDocumentContract> {
    cli_contract()
        .documents
        .iter()
        .find(|document| document.id == id)
}

/// Returns the canonical CLI write-failure error for one registered user-facing document id.
pub fn document_write_error(document_id: &str) -> Option<String> {
    cli_document(document_id).map(|document| document.render_cli_write_error())
}

/// Returns the canonical CLI-usage error for invalid batch concurrency.
pub fn positive_batch_concurrency_usage_error() -> &'static str {
    cli_hard_limit(CLI_LIMIT_POSITIVE_BATCH_CONCURRENCY_ID)
        .expect("positive batch concurrency limit")
        .cli_usage_error_template
}

/// Returns the canonical CLI-usage error for duplicate explicit target ids.
pub fn duplicate_target_ids_usage_error(duplicate_target_id: &str) -> String {
    cli_hard_limit(CLI_LIMIT_UNIQUE_TARGET_IDS_ID)
        .expect("unique explicit target ids limit")
        .render_cli_usage_error(Some(duplicate_target_id))
}

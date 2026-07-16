use crate::{
    BATCH_RUN_REPORT_SCHEMA_NAME, RESET_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_NAME, RunMode,
    STATUS_REPORT_SCHEMA_NAME, TARGET_SCHEMA_NAME,
};

use super::types::{
    CliArgumentContract, CliArgumentValueKind, CliContractCatalog, CliHardLimitContract,
    CliInvocationContract, CliOperationContract, ExecutionModeContract, UserFacingDocumentContract,
};

/// Canonical CLI operation id for measurement runs.
pub const CLI_OPERATION_RUN_ID: &str = "run";
/// Canonical CLI operation id for status inspection.
pub const CLI_OPERATION_STATUS_ID: &str = "status";
/// Canonical CLI operation id for blind v2 storage reset.
pub const CLI_OPERATION_RESET_ID: &str = "reset";
/// Canonical CLI argument id for `--watch-root`.
pub const CLI_ARGUMENT_WATCH_ROOT_ID: &str = "watch_root";
/// Canonical CLI argument id for `--target`.
pub const CLI_ARGUMENT_TARGET_ID: &str = "target";
/// Canonical CLI argument id for `--all`.
pub const CLI_ARGUMENT_ALL_ID: &str = "all";
/// Canonical CLI argument id for `--jobs`.
pub const CLI_ARGUMENT_JOBS_ID: &str = "jobs";
/// Canonical CLI argument id for `--dry-run`.
pub const CLI_ARGUMENT_DRY_RUN_ID: &str = "dry_run";
/// Canonical CLI argument id for `--format`.
pub const CLI_ARGUMENT_FORMAT_ID: &str = "format";

const RUN_ARGUMENTS: &[CliArgumentContract] = &[
    watch_root_argument(),
    CliArgumentContract {
        id: CLI_ARGUMENT_TARGET_ID,
        long_name: "target",
        display_label: "Target",
        value_name: Some("ID"),
        help_summary: "One or more target ids under the watch root.",
        value_kind: CliArgumentValueKind::String,
        repeatable: true,
        required: false,
        conflicts_with: &[],
        default_value: None,
    },
    CliArgumentContract {
        id: CLI_ARGUMENT_ALL_ID,
        long_name: "all",
        display_label: "All",
        value_name: None,
        help_summary: "Run each immediate target directory containing target.toml.",
        value_kind: CliArgumentValueKind::Flag,
        repeatable: false,
        required: false,
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
        conflicts_with: &[],
        default_value: Some("1"),
    },
    CliArgumentContract {
        id: CLI_ARGUMENT_DRY_RUN_ID,
        long_name: "dry-run",
        display_label: "Dry Run",
        value_name: None,
        help_summary: "Fetch, acquire, and type the observation without writing v2 state.",
        value_kind: CliArgumentValueKind::Flag,
        repeatable: false,
        required: false,
        conflicts_with: &[],
        default_value: None,
    },
    format_argument(),
];

const TARGET_OPERATION_ARGUMENTS: &[CliArgumentContract] = &[
    watch_root_argument(),
    CliArgumentContract {
        id: CLI_ARGUMENT_TARGET_ID,
        long_name: "target",
        display_label: "Target",
        value_name: Some("ID"),
        help_summary: "One target id under the watch root.",
        value_kind: CliArgumentValueKind::String,
        repeatable: false,
        required: true,
        conflicts_with: &[],
        default_value: None,
    },
    format_argument(),
];

const RESET_ARGUMENTS: &[CliArgumentContract] = &[
    watch_root_argument(),
    CliArgumentContract {
        id: CLI_ARGUMENT_TARGET_ID,
        long_name: "target",
        display_label: "Target",
        value_name: Some("ID"),
        help_summary: "One target id whose isolated v2 storage will be deleted blindly.",
        value_kind: CliArgumentValueKind::String,
        repeatable: false,
        required: true,
        conflicts_with: &[],
        default_value: None,
    },
    format_argument(),
];

const RUN_INVOCATIONS: &[CliInvocationContract] = &[
    CliInvocationContract {
        id: "run_single_target",
        operation_id: CLI_OPERATION_RUN_ID,
        usage: "ffhn run --target <id>",
        output_document_id: RUN_REPORT_SCHEMA_NAME,
        analysis_summary: "single-target measurement",
    },
    CliInvocationContract {
        id: "run_explicit_batch",
        operation_id: CLI_OPERATION_RUN_ID,
        usage: "ffhn run --target <a> --target <b>",
        output_document_id: BATCH_RUN_REPORT_SCHEMA_NAME,
        analysis_summary: "explicit target batch",
    },
    CliInvocationContract {
        id: "run_discovered_batch",
        operation_id: CLI_OPERATION_RUN_ID,
        usage: "ffhn run --all",
        output_document_id: BATCH_RUN_REPORT_SCHEMA_NAME,
        analysis_summary: "watch-root target batch",
    },
];
const STATUS_INVOCATIONS: &[CliInvocationContract] = &[CliInvocationContract {
    id: "status_single_target",
    operation_id: CLI_OPERATION_STATUS_ID,
    usage: "ffhn status --target <id>",
    output_document_id: STATUS_REPORT_SCHEMA_NAME,
    analysis_summary: "stable v2 status inspection",
}];
const RESET_INVOCATIONS: &[CliInvocationContract] = &[CliInvocationContract {
    id: "reset_single_target",
    operation_id: CLI_OPERATION_RESET_ID,
    usage: "ffhn reset --target <id>",
    output_document_id: RESET_REPORT_SCHEMA_NAME,
    analysis_summary: "blind isolated v2 storage deletion",
}];

const OPERATIONS: &[CliOperationContract] = &[
    CliOperationContract {
        id: CLI_OPERATION_RUN_ID,
        display_label: "Run",
        help_summary: "Acquire and type one or more JSON or HTML measurements.",
        usage: "ffhn run (--target <ID>... | --all) [--watch-root <PATH>] [--jobs <N>] [--dry-run] [--format <FORMAT>]",
        arguments: RUN_ARGUMENTS,
        invocations: RUN_INVOCATIONS,
        examples: &[
            "ffhn run --target demo",
            "ffhn run --all --jobs 4",
            "ffhn run --target demo --dry-run",
        ],
        output_notes: &[
            "A single target produces ffhn.run_report; batches produce ffhn.batch_run_report.",
            "A contract-digest mismatch is refused without mutation and directs the operator to reset.",
        ],
        operational_notes: &[
            "The watch root must already exist and be a directory.",
            "Valid live runs persist only v2 state under the target's isolated .ffhn storage root.",
        ],
    },
    CliOperationContract {
        id: CLI_OPERATION_STATUS_ID,
        display_label: "Status",
        help_summary: "Read one target's current v2 measurement state.",
        usage: "ffhn status --target <ID> [--watch-root <PATH>] [--format <FORMAT>]",
        arguments: TARGET_OPERATION_ARGUMENTS,
        invocations: STATUS_INVOCATIONS,
        examples: &["ffhn status --target demo"],
        output_notes: &["Produces one ffhn.status_report."],
        operational_notes: &["Status waits behind an active target run before reading state."],
    },
    CliOperationContract {
        id: CLI_OPERATION_RESET_ID,
        display_label: "Reset",
        help_summary: "Blindly delete one target's isolated v2 storage root.",
        usage: "ffhn reset --target <ID> [--watch-root <PATH>] [--format <FORMAT>]",
        arguments: RESET_ARGUMENTS,
        invocations: RESET_INVOCATIONS,
        examples: &["ffhn reset --target demo"],
        output_notes: &["Produces one ffhn.reset_report."],
        operational_notes: &[
            "Reset acquires the target lock and does not inspect storage contents.",
        ],
    },
];

const EXECUTION_MODES: &[ExecutionModeContract] = &[
    ExecutionModeContract {
        mode: RunMode::Live,
        id: "live",
        display_label: "Live",
        summary: "Fetch, JSON acquisition, typed parsing, and v2 state persistence.",
        writes_state: true,
    },
    ExecutionModeContract {
        mode: RunMode::DryRun,
        id: "dry_run",
        display_label: "Dry Run",
        summary: "Fetch, JSON acquisition, and typed parsing without persistent mutation.",
        writes_state: false,
    },
];

const HARD_LIMITS: &[CliHardLimitContract] = &[
    CliHardLimitContract {
        id: "positive_batch_concurrency",
        operation_id: Some(CLI_OPERATION_RUN_ID),
        display_label: "Positive Batch Concurrency",
        summary: "--jobs must be positive.",
        cli_usage_error_template: "`--jobs <N>` must be a positive integer",
    },
    CliHardLimitContract {
        id: "run_target_selection",
        operation_id: Some(CLI_OPERATION_RUN_ID),
        display_label: "Run Target Selection",
        summary: "One of --target or --all is required.",
        cli_usage_error_template: "one of '--target <ID>' or '--all' is required",
    },
    CliHardLimitContract {
        id: "unique_target_ids",
        operation_id: Some(CLI_OPERATION_RUN_ID),
        display_label: "Unique Target IDs",
        summary: "Repeated explicit targets are not allowed.",
        cli_usage_error_template: "duplicate --target values are not allowed: {detail}",
    },
];

const DOCUMENTS: &[UserFacingDocumentContract] = &[
    UserFacingDocumentContract {
        id: TARGET_SCHEMA_NAME,
        display_label: "Target Document",
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
    UserFacingDocumentContract {
        id: RESET_REPORT_SCHEMA_NAME,
        display_label: "Reset Report",
    },
];

const CLI_CONTRACT: CliContractCatalog = CliContractCatalog {
    operations: OPERATIONS,
    execution_modes: EXECUTION_MODES,
    hard_limits: HARD_LIMITS,
    documents: DOCUMENTS,
};

/// Returns the canonical v2 CLI catalog.
pub fn cli_contract() -> &'static CliContractCatalog {
    &CLI_CONTRACT
}
/// Looks up an operation by canonical id.
pub fn cli_operation(id: &str) -> Option<&'static CliOperationContract> {
    cli_contract()
        .operations
        .iter()
        .find(|operation| operation.id == id)
}
/// Returns the run operation.
pub fn run_operation() -> &'static CliOperationContract {
    cli_operation(CLI_OPERATION_RUN_ID).expect("run operation")
}
/// Returns the status operation.
pub fn status_operation() -> &'static CliOperationContract {
    cli_operation(CLI_OPERATION_STATUS_ID).expect("status operation")
}
/// Returns the reset operation.
pub fn reset_operation() -> &'static CliOperationContract {
    cli_operation(CLI_OPERATION_RESET_ID).expect("reset operation")
}
/// Returns the positive-job usage error.
pub fn positive_batch_concurrency_usage_error() -> &'static str {
    HARD_LIMITS[0].cli_usage_error_template
}
/// Returns the target-selection usage error.
pub fn run_target_selection_usage_error() -> &'static str {
    HARD_LIMITS[1].cli_usage_error_template
}
/// Returns the duplicate-target usage error.
pub fn duplicate_target_ids_usage_error(target: &str) -> String {
    HARD_LIMITS[2].render_cli_usage_error(Some(target))
}

const fn watch_root_argument() -> CliArgumentContract {
    CliArgumentContract {
        id: CLI_ARGUMENT_WATCH_ROOT_ID,
        long_name: "watch-root",
        display_label: "Watch Root",
        value_name: Some("PATH"),
        help_summary: "Watch root containing per-target target.toml directories.",
        value_kind: CliArgumentValueKind::Path,
        repeatable: false,
        required: false,
        conflicts_with: &[],
        default_value: Some("watchlist"),
    }
}
const fn format_argument() -> CliArgumentContract {
    CliArgumentContract {
        id: CLI_ARGUMENT_FORMAT_ID,
        long_name: "format",
        display_label: "Format",
        value_name: Some("FORMAT"),
        help_summary: "Output format: json, json-pretty, or summary.",
        value_kind: CliArgumentValueKind::String,
        repeatable: false,
        required: false,
        conflicts_with: &[],
        default_value: Some("json"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lookup_limits_and_argument_factories_are_complete() {
        assert_eq!(cli_contract().operations.len(), 3);
        assert!(cli_operation("missing").is_none());
        assert_eq!(run_operation().id, CLI_OPERATION_RUN_ID);
        assert_eq!(status_operation().id, CLI_OPERATION_STATUS_ID);
        assert_eq!(reset_operation().id, CLI_OPERATION_RESET_ID);
        assert!(positive_batch_concurrency_usage_error().contains("jobs"));
        assert!(run_target_selection_usage_error().contains("target"));
        assert!(duplicate_target_ids_usage_error("demo").contains("demo"));
        let watch_root = watch_root_argument();
        assert_eq!(watch_root.id, CLI_ARGUMENT_WATCH_ROOT_ID);
        assert_eq!(watch_root.value_kind, CliArgumentValueKind::Path);
        let format = format_argument();
        assert_eq!(format.id, CLI_ARGUMENT_FORMAT_ID);
        assert_eq!(format.default_value, Some("json"));
    }
}

use crate::{
    BATCH_RUN_REPORT_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_NAME, RUN_REPORT_SCHEMA_NAME, RunMode,
    STATE_SCHEMA_NAME, STATUS_REPORT_SCHEMA_NAME, TARGET_SCHEMA_NAME,
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

/// One user-facing CLI operation argument kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliArgumentValueKind {
    /// Boolean flag.
    Flag,
    /// Filesystem path value.
    Path,
    /// Free-form string value.
    String,
    /// Positive integer value.
    PositiveInteger,
}

/// One canonical user-facing CLI argument description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CliArgumentContract {
    /// Stable argument id.
    pub id: &'static str,
    /// Long-option name without the `--` prefix.
    pub long_name: &'static str,
    /// Human-facing label.
    pub display_label: &'static str,
    /// Value placeholder shown in help output when applicable.
    pub value_name: Option<&'static str>,
    /// User-facing help summary.
    pub help_summary: &'static str,
    /// Parser/value shape.
    pub value_kind: CliArgumentValueKind,
    /// Whether the argument may repeat.
    pub repeatable: bool,
    /// Whether the argument is required.
    pub required: bool,
    /// Companion argument id that satisfies the requirement when present.
    pub required_unless_present: Option<&'static str>,
    /// Conflicting argument ids.
    pub conflicts_with: &'static [&'static str],
    /// Default value shown in help when applicable.
    pub default_value: Option<&'static str>,
}

/// One canonical user-facing CLI invocation summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CliInvocationContract {
    /// Stable invocation id.
    pub id: &'static str,
    /// Owning CLI operation id.
    pub operation_id: &'static str,
    /// Canonical usage pattern.
    pub usage: &'static str,
    /// Structured stdout document id.
    pub output_document_id: &'static str,
    /// Machine-usable summary used in catalog tables.
    pub analysis_summary: &'static str,
}

/// One canonical user-facing CLI operation description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CliOperationContract {
    /// Stable operation id.
    pub id: &'static str,
    /// Human-facing label.
    pub display_label: &'static str,
    /// Canonical help summary.
    pub help_summary: &'static str,
    /// Operation arguments in display order.
    pub arguments: &'static [CliArgumentContract],
    /// Canonical invocation patterns for the operation.
    pub invocations: &'static [CliInvocationContract],
}

/// One canonical execution-mode description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionModeContract {
    /// Stable serialized mode value.
    pub mode: RunMode,
    /// Stable mode id.
    pub id: &'static str,
    /// Human-facing label.
    pub display_label: &'static str,
    /// User-facing mode summary.
    pub summary: &'static str,
    /// Whether the mode persists `state.json`.
    pub writes_state: bool,
    /// Whether the mode persists `last_run.json`.
    pub writes_last_run: bool,
    /// Whether the mode delivers notification hooks.
    pub delivers_notifications: bool,
}

/// One canonical CLI hard limitation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CliHardLimitContract {
    /// Stable hard-limit id.
    pub id: &'static str,
    /// Owning operation id when the limit is operation-specific.
    pub operation_id: Option<&'static str>,
    /// Human-facing label.
    pub display_label: &'static str,
    /// User-facing summary.
    pub summary: &'static str,
    /// CLI-usage error template. Use `{detail}` for one appended value.
    pub cli_usage_error_template: &'static str,
}

impl CliHardLimitContract {
    /// Renders the canonical CLI-usage error for this hard limit.
    pub fn render_cli_usage_error(self, detail: Option<&str>) -> String {
        match detail {
            Some(detail) if self.cli_usage_error_template.contains("{detail}") => {
                self.cli_usage_error_template.replace("{detail}", detail)
            }
            _ => self.cli_usage_error_template.to_owned(),
        }
    }
}

/// One user-facing document contract owned by FFHN core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserFacingDocumentContract {
    /// Stable document id.
    pub id: &'static str,
    /// Human-facing label.
    pub display_label: &'static str,
}

impl UserFacingDocumentContract {
    /// Renders the canonical CLI write-failure text for this document.
    pub fn render_cli_write_error(self) -> String {
        format!(
            "could not write {}",
            self.display_label.to_ascii_lowercase()
        )
    }
}

/// Canonical user-facing FFHN CLI contract catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CliContractCatalog {
    /// Registered CLI operations.
    pub operations: &'static [CliOperationContract],
    /// Registered execution modes.
    pub execution_modes: &'static [ExecutionModeContract],
    /// Registered hard limitations.
    pub hard_limits: &'static [CliHardLimitContract],
    /// Registered user-facing document ids.
    pub documents: &'static [UserFacingDocumentContract],
}

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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn cli_contract_catalog_keeps_ids_unique() {
        let operation_ids = cli_contract()
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(operation_ids.len(), cli_contract().operations.len());

        let invocation_ids = cli_contract()
            .operations
            .iter()
            .flat_map(|operation| operation.invocations.iter().map(|invocation| invocation.id))
            .collect::<BTreeSet<_>>();
        assert_eq!(invocation_ids.len(), 4);

        let document_ids = cli_contract()
            .documents
            .iter()
            .map(|document| document.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(document_ids.len(), cli_contract().documents.len());
    }

    #[test]
    fn cli_contract_lookups_cover_present_and_missing_ids() {
        assert_eq!(
            cli_operation(CLI_OPERATION_RUN_ID)
                .expect("run operation")
                .display_label,
            "Run"
        );
        assert!(cli_operation("bogus").is_none());

        assert_eq!(
            cli_document(RUN_REPORT_SCHEMA_NAME)
                .expect("run report document")
                .display_label,
            "Run Report"
        );
        assert_eq!(
            cli_document(RUN_REPORT_SCHEMA_NAME)
                .expect("run report document")
                .render_cli_write_error(),
            "could not write run report"
        );
        assert!(cli_document("ffhn.unknown_report").is_none());

        assert_eq!(
            cli_hard_limit(CLI_LIMIT_IMMEDIATE_DISCOVERY_DEPTH_ID)
                .expect("watch-root discovery limit")
                .display_label,
            "Immediate Watch-Root Discovery"
        );
        assert!(cli_hard_limit("bogus-limit").is_none());
    }

    #[test]
    fn execution_modes_and_usage_errors_use_the_canonical_contract() {
        let live = cli_execution_mode(RunMode::Live);
        let dry_run = cli_execution_mode(RunMode::DryRun);

        assert_eq!(live.id, "live");
        assert!(live.writes_state);
        assert!(live.writes_last_run);
        assert!(live.delivers_notifications);

        assert_eq!(dry_run.id, "dry_run");
        assert!(!dry_run.writes_state);
        assert!(!dry_run.writes_last_run);
        assert!(!dry_run.delivers_notifications);

        assert_eq!(
            positive_batch_concurrency_usage_error(),
            "must be a positive integer"
        );
        assert_eq!(
            duplicate_target_ids_usage_error("demo"),
            "duplicate --target values are not allowed: demo"
        );
        assert_eq!(
            document_write_error(STATUS_REPORT_SCHEMA_NAME).expect("status write error"),
            "could not write status report"
        );
        assert!(document_write_error("ffhn.unknown_report").is_none());
    }

    #[test]
    fn hard_limit_rendering_covers_plain_and_placeholder_templates() {
        let plain = cli_hard_limit(CLI_LIMIT_POSITIVE_BATCH_CONCURRENCY_ID)
            .expect("positive batch concurrency limit");
        let placeholder =
            cli_hard_limit(CLI_LIMIT_UNIQUE_TARGET_IDS_ID).expect("unique target ids limit");

        assert_eq!(
            plain.render_cli_usage_error(Some("ignored")),
            "must be a positive integer"
        );
        assert_eq!(
            placeholder.render_cli_usage_error(None),
            "duplicate --target values are not allowed: {detail}"
        );
        assert_eq!(
            placeholder.render_cli_usage_error(Some("demo")),
            "duplicate --target values are not allowed: demo"
        );
    }
}

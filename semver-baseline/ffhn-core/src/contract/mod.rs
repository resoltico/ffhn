mod catalog;
#[cfg(test)]
mod tests;
mod types;

pub use catalog::{
    CLI_ARGUMENT_ALL_ID, CLI_ARGUMENT_DRY_RUN_ID, CLI_ARGUMENT_JOBS_ID, CLI_ARGUMENT_TARGET_ID,
    CLI_ARGUMENT_WATCH_ROOT_ID, CLI_INVOCATION_RUN_ALL_ID, CLI_INVOCATION_RUN_BATCH_ID,
    CLI_INVOCATION_RUN_SINGLE_ID, CLI_INVOCATION_STATUS_ID, CLI_LIMIT_IMMEDIATE_DISCOVERY_DEPTH_ID,
    CLI_LIMIT_POSITIVE_BATCH_CONCURRENCY_ID, CLI_LIMIT_UNIQUE_TARGET_IDS_ID, CLI_OPERATION_RUN_ID,
    CLI_OPERATION_STATUS_ID, cli_contract, cli_document, cli_execution_mode, cli_hard_limit,
    cli_operation, document_write_error, duplicate_target_ids_usage_error,
    positive_batch_concurrency_usage_error,
};
pub use types::{
    CliArgumentContract, CliArgumentValueKind, CliContractCatalog, CliHardLimitContract,
    CliInvocationContract, CliOperationContract, ExecutionModeContract, UserFacingDocumentContract,
};

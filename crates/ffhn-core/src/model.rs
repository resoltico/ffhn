mod delivery;
mod failure;
mod observation;
mod outbox;
mod policy;
mod report;
mod state;
mod target;
#[cfg(test)]
mod tests;
mod validate;
mod value;

pub use delivery::{DeliveryAdapter, DeliveryRoute, OutboxPolicy, RouteFamily, RouteId};
pub(crate) use delivery::{
    ProcessStdinEventKey, ProcessStdinEventKind, ProcessStdinPayload,
    validate_process_stdin_payload_bytes,
};
pub use failure::{PermanentErrorCode, SourceSuspectReason};
pub(crate) use observation::HtmlObservationInput;
pub(crate) use observation::select_json_scalar_token;
pub use observation::{
    AcquisitionKind, HtmlcutDiagnostic, Observation, PARSER_GRAMMAR_VERSION, PARSER_ID,
};
pub use outbox::OutboxOverflow;
pub(crate) use outbox::StagedOutboxRecord;
pub use policy::{
    Condition, ConditionContext, ConditionEvaluation, ConditionId, ConditionIssue,
    ConditionOutcome, ConditionPredicate, ConditionReference, OnRunEventCause, PolicyRunInput,
    StagedEventEligibility, StagedPolicyRun, ThresholdDirection,
};
pub(crate) use report::RunReportParts;
pub use report::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, BatchRunReport, DeliveryOutcome,
    DeliveryStatus, HtmlcutFailureDetails, ProcessErrorDetail, ProcessErrorKind,
    RESET_REPORT_SCHEMA_NAME, RESET_REPORT_SCHEMA_VERSION, RUN_REPORT_SCHEMA_NAME,
    RUN_REPORT_SCHEMA_VERSION, ResetReport, RunMode, RunOutcome, RunReport,
    STATUS_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_VERSION, StatusKind, StatusReport,
};
pub use state::{STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION, StateDocument};
pub use target::{
    DeclaredType, FetchConfig, FetchEngine, HtmlSelection, HttpMethod, NumericLocale, Projection,
    TARGET_SCHEMA_NAME, TARGET_SCHEMA_VERSION, TargetDocument, TargetSource, TypeParams,
};
pub use value::TargetId;

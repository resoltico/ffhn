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

pub use delivery::{
    DeliveryAdapter, DeliveryEventKind, DeliveryRoute, OutboxPolicy, RouteFamily, RouteId,
};
pub(crate) use delivery::{
    ProcessStdinEventKey, ProcessStdinPayload, read_validated_process_stdin_payload_bytes,
};
pub use failure::{IntegrationFaultCode, PermanentErrorCode, SourceSuspectReason};
pub use observation::{
    AcquisitionKind, HtmlcutByteRange, HtmlcutDiagnostic, HtmlcutDiagnosticCode,
    HtmlcutDiagnosticDetails, HtmlcutDiagnosticLevel, HtmlcutSelectorParse,
    HtmlcutSelectorParseErrorClass, HtmlcutSliceMarkupMatch, Observation, PARSER_GRAMMAR_VERSION,
    PARSER_ID,
};
pub(crate) use observation::{HtmlObservationInput, parse::select_json_scalar_token};
pub use outbox::OutboxOverflow;
pub(crate) use outbox::StagedOutboxRecord;
#[cfg(test)]
pub(crate) use policy::POLICY_EVALUATION_SEMANTICS_VERSION;
pub use policy::{
    Condition, ConditionContext, ConditionEvaluation, ConditionId, ConditionIssue,
    ConditionOutcome, ConditionPredicate, ConditionReference, ConditionReferenceEvidence,
    OnRunEventCause, PolicyRunInput, StagedEventEligibility, StagedPolicyRun, ThresholdDirection,
};
#[cfg(test)]
pub(crate) use report::source_health_detail_matches_reason_for_test;
pub use report::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, BatchRunReport,
    DeliveryFailurePrimary, DeliveryOutcome, DeliveryProcessAttempt, DeliveryStatus,
    DiagnosticDetail, DiagnosticKind, DiagnosticMessageTruncation, DiagnosticOperation,
    ExactByteCount, FetchFailureDetails, HtmlcutBoundaryEvidence, HtmlcutErrorClass,
    HtmlcutFailureDetails, IntegrationFaultEpisodeSnapshot, IoErrorClass, LifecycleFacet,
    LifecycleSnapshot, PermanentErrorEpisodeSnapshot, PolicyConditionResult, PolicyEvaluation,
    PolicyEventEligibility, PolicyReferenceEvidence, RESET_REPORT_SCHEMA_NAME,
    RESET_REPORT_SCHEMA_VERSION, RUN_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_VERSION, ResetReport,
    RunMode, RunOutcome, RunReport, STATUS_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_VERSION,
    SourceHealthSnapshot, SourceHealthState, StatusKind, StatusReport, StderrCapture,
    StderrCaptureProblem, StderrEncoding, StderrOutcome, TerminalOutcome, WriterOutcome,
};
pub(crate) use report::{
    RunReportParts, StatusReportParts, delivery_failure_detail, delivery_observability_detail,
    detail_from_core_error, fetch_detail, htmlcut_detail, integration_detail, io_detail,
    plain_detail, require_canonical_utc_rfc3339, validate_source_health_evidence,
};
pub use state::{STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION, StateDocument};
pub(crate) use target::PermanentTargetError;
pub use target::{
    DeclaredType, FetchConfig, FetchEngine, HtmlSelection, HttpMethod, NumericLocale, Projection,
    TARGET_SCHEMA_NAME, TARGET_SCHEMA_VERSION, TargetDocument, TargetSource, TypeParams,
};
pub use value::TargetId;

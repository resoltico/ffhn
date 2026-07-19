//! Stable report schemas split by diagnostics, delivery evidence, report records, and accessors.

mod accessors;
mod delivery;
mod diagnostic;
mod lifecycle;
mod policy;
mod records;

pub use delivery::{DeliveryOutcome, DeliveryStatus};
pub use diagnostic::{
    DeliveryFailurePrimary, DeliveryProcessAttempt, DiagnosticDetail, DiagnosticKind,
    DiagnosticMessageTruncation, DiagnosticOperation, ExactByteCount, FetchFailureDetails,
    HtmlcutBoundaryEvidence, HtmlcutErrorClass, HtmlcutFailureDetails, IoErrorClass, StderrCapture,
    StderrCaptureProblem, StderrEncoding, StderrOutcome, TerminalOutcome, WriterOutcome,
};
pub(crate) use diagnostic::{
    delivery_failure_detail, delivery_observability_detail, detail_from_core_error, fetch_detail,
    htmlcut_detail, integration_detail, io_detail, plain_detail,
};
#[cfg(test)]
pub(crate) use lifecycle::source_health_detail_matches_reason_for_test;
pub use lifecycle::{
    IntegrationFaultEpisodeSnapshot, LifecycleFacet, LifecycleSnapshot,
    PermanentErrorEpisodeSnapshot, SourceHealthSnapshot, SourceHealthState,
};
pub(crate) use lifecycle::{require_canonical_utc_rfc3339, validate_source_health_evidence};
pub use policy::{
    PolicyConditionResult, PolicyEvaluation, PolicyEventEligibility, PolicyReferenceEvidence,
};
pub use records::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, BatchRunReport,
    RESET_REPORT_SCHEMA_NAME, RESET_REPORT_SCHEMA_VERSION, RUN_REPORT_SCHEMA_NAME,
    RUN_REPORT_SCHEMA_VERSION, ResetReport, RunMode, RunOutcome, RunReport,
    STATUS_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_VERSION, StatusKind, StatusReport,
};
pub(crate) use records::{RunReportParts, StatusReportParts};

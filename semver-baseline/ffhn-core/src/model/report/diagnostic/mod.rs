//! Closed diagnostic vocabulary and the only FFHN-owned construction boundary.

mod types;

pub(crate) use types::construction::{
    delivery_failure_detail, delivery_observability_detail, detail_from_core_error, fetch_detail,
    htmlcut_detail, integration_detail, io_detail, plain_detail,
};
pub use types::{
    DeliveryFailurePrimary, DeliveryProcessAttempt, DiagnosticDetail, DiagnosticKind,
    DiagnosticMessageTruncation, DiagnosticOperation, ExactByteCount, FetchFailureDetails,
    HtmlcutBoundaryEvidence, HtmlcutErrorClass, HtmlcutFailureDetails, IoErrorClass, StderrCapture,
    StderrCaptureProblem, StderrEncoding, StderrOutcome, TerminalOutcome, WriterOutcome,
};

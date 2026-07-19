//! Closed diagnostic and process-attempt value objects.

use serde::{Deserialize, Serialize};

use crate::stable_json::{sha256_hex, stable_json};
use crate::{CoreError, IntegrationFaultCode};

#[path = "construction.rs"]
pub(crate) mod construction;
#[path = "count.rs"]
mod count;
#[path = "htmlcut.rs"]
mod htmlcut;
#[path = "io.rs"]
mod io;
#[path = "process.rs"]
mod process;

pub use count::ExactByteCount;
pub use htmlcut::{HtmlcutBoundaryEvidence, HtmlcutErrorClass, HtmlcutFailureDetails};
pub use io::IoErrorClass;
pub use process::{
    DeliveryFailurePrimary, DeliveryProcessAttempt, StderrCapture, StderrCaptureProblem,
    StderrEncoding, StderrOutcome, TerminalOutcome, WriterOutcome,
};

/// Maximum byte length of a diagnostic payload message.
pub(crate) const DIAGNOSTIC_MESSAGE_LIMIT: usize = 1_024;
/// Maximum stable-JSON byte length of one persisted delivery failure detail.
pub(crate) const DURABLE_DELIVERY_DETAIL_LIMIT: usize = 4_096;

/// Typed evidence that a bounded diagnostic message retains only its UTF-8 prefix.
///
/// The message itself remains the unclassified explanatory payload. The truncation fact is
/// deliberately separate so a payload that happens to end with a human-looking marker is never
/// mistaken for FFHN metadata. The original length and digest identify the complete input without
/// serializing an unbounded foreign message into FFHN's public contracts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticMessageTruncation {
    original_len_bytes: ExactByteCount,
    original_sha256: String,
}

impl DiagnosticMessageTruncation {
    fn from_original(message: &str) -> Self {
        let original_sha256 = sha256_hex(message.as_bytes());
        Self {
            original_len_bytes: ExactByteCount::from_usize(message.len()),
            original_sha256,
        }
    }

    fn validate(&self, retained_len_bytes: usize) -> Result<(), CoreError> {
        if !self
            .original_len_bytes
            .compare_usize(retained_len_bytes)
            .is_gt()
        {
            return Err(CoreError::contract(
                "diagnostic message truncation must omit at least one original byte",
            ));
        }
        if !is_sha256(&self.original_sha256) {
            return Err(CoreError::contract(
                "diagnostic message truncation original_sha256 must be lowercase SHA-256",
            ));
        }
        Ok(())
    }

    /// Returns the exact byte length of the pre-truncation message.
    pub const fn original_len_bytes(&self) -> &ExactByteCount {
        &self.original_len_bytes
    }

    /// Returns the lowercase SHA-256 digest of the pre-truncation message bytes.
    pub fn original_sha256(&self) -> &str {
        &self.original_sha256
    }
}

/// Stable diagnostic-category vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticKind {
    /// Contract or configuration violation.
    Contract,
    /// Filesystem or network I/O failure.
    Io,
    /// JSON syntax failure.
    Json,
    /// HTMLCut selection or projection failure.
    Htmlcut,
    /// TOML syntax failure.
    Toml,
    /// Semantic value parsing failure.
    ValueUnparseable,
    /// FFHN could not uphold the invariant that makes a policy decision exact.
    PolicyInvariant,
    /// Process-delivery failure or successful-delivery observability anomaly.
    Delivery,
}

impl DiagnosticKind {
    /// Returns the stable report-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Contract => "contract",
            Self::Io => "io",
            Self::Json => "json",
            Self::Htmlcut => "htmlcut",
            Self::Toml => "toml",
            Self::ValueUnparseable => "value_unparseable",
            Self::PolicyInvariant => "policy_invariant",
            Self::Delivery => "delivery",
        }
    }
}

/// Closed operation vocabulary for every serialized FFHN diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticOperation {
    /// Reading or parsing `target.toml`.
    TargetLoad,
    /// Target-contract validation, including projection validation.
    TargetValidation,
    /// Acquiring a target lock.
    LockAcquire,
    /// State envelope preflight, decoding, or self-validation.
    StateLoad,
    /// Atomic staged state/outbox commit.
    StateCommit,
    /// HTTP acquisition.
    HttpFetch,
    /// File-engine acquisition.
    FileRead,
    /// RFC 6901 JSON Pointer selection.
    JsonPointerSelection,
    /// HTMLCut interoperation.
    HtmlExtraction,
    /// Parsing into the declared type.
    ValueParse,
    /// Condition evaluation or exactness invariant enforcement.
    PolicyEvaluation,
    /// Process-stdin delivery execution.
    DeliveryProcess,
    /// Drain-time failure before any durable record update.
    OutboxDrain,
    /// Durable pending-record update or removal.
    OutboxStateCommit,
}

impl DiagnosticOperation {
    /// Returns the stable report-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TargetLoad => "target_load",
            Self::TargetValidation => "target_validation",
            Self::LockAcquire => "lock_acquire",
            Self::StateLoad => "state_load",
            Self::StateCommit => "state_commit",
            Self::HttpFetch => "http_fetch",
            Self::FileRead => "file_read",
            Self::JsonPointerSelection => "json_pointer_selection",
            Self::HtmlExtraction => "html_extraction",
            Self::ValueParse => "value_parse",
            Self::PolicyEvaluation => "policy_evaluation",
            Self::DeliveryProcess => "delivery_process",
            Self::OutboxDrain => "outbox_drain",
            Self::OutboxStateCommit => "outbox_state_commit",
        }
    }
}

/// Closed non-native evidence from FFHN's bounded file and HTTP acquisition boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FetchFailureDetails {
    /// An HTTP response carried a non-success status.
    HttpStatus {
        /// The observed HTTP status code.
        status: u16,
    },
    /// The HTTP `Content-Length` header exceeded the configured limit before body reading began.
    HttpContentLengthExceeded {
        /// Configured upper bound in bytes.
        configured_max_bytes: usize,
        /// Exact advertised response length in bytes.
        content_length: usize,
    },
    /// A source body exceeded the configured limit while FFHN was reading it.
    BodyBytesExceeded {
        /// Configured upper bound in bytes.
        configured_max_bytes: usize,
        /// Exact bytes FFHN drained before rejecting the body.
        observed_bytes: usize,
    },
    /// A completed source body was not valid UTF-8.
    InvalidUtf8,
}

impl FetchFailureDetails {
    fn validate_for(&self, operation: DiagnosticOperation) -> Result<(), CoreError> {
        match (operation, self) {
            (DiagnosticOperation::HttpFetch, Self::HttpStatus { status })
                if !(200..300).contains(status) =>
            {
                Ok(())
            }
            (
                DiagnosticOperation::HttpFetch,
                Self::HttpContentLengthExceeded {
                    configured_max_bytes,
                    content_length,
                },
            ) if content_length > configured_max_bytes => Ok(()),
            (
                DiagnosticOperation::HttpFetch | DiagnosticOperation::FileRead,
                Self::BodyBytesExceeded {
                    configured_max_bytes,
                    observed_bytes,
                },
            ) if observed_bytes > configured_max_bytes => Ok(()),
            (DiagnosticOperation::HttpFetch | DiagnosticOperation::FileRead, Self::InvalidUtf8) => {
                Ok(())
            }
            _ => Err(CoreError::contract(
                "fetch_failure evidence does not match its diagnostic operation",
            )),
        }
    }
}

/// Tagged detail for process-delivery failure or delivered-event observability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum DeliveryProcessDetail {
    /// A failed delivery attempt with all independent execution facts.
    Failure {
        /// Complete attempt facts.
        attempt: DeliveryProcessAttempt,
        /// Deterministic primary failure category derived from `attempt`.
        primary: DeliveryFailurePrimary,
    },
    /// An anomalous stderr capture after delivery succeeded.
    Observability {
        /// The completed-delivery stderr capture problem.
        stderr_capture_problem: StderrCaptureProblem,
    },
}

/// Stable structured diagnostic detail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticDetail {
    kind: DiagnosticKind,
    operation: DiagnosticOperation,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_truncation: Option<Box<DiagnosticMessageTruncation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    io_error_class: Option<IoErrorClass>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fetch_failure: Option<FetchFailureDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    htmlcut_failure: Option<Box<HtmlcutFailureDetails>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_fault_code: Option<IntegrationFaultCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_process: Option<Box<DeliveryProcessDetail>>,
}

impl DiagnosticDetail {
    /// Returns the closed diagnostic category.
    pub const fn kind(&self) -> DiagnosticKind {
        self.kind
    }

    /// Returns the closed operation where the diagnostic arose.
    pub const fn operation(&self) -> DiagnosticOperation {
        self.operation
    }

    /// Returns the unclassified payload message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns typed evidence when FFHN retained only a bounded prefix of the message payload.
    pub fn message_truncation(&self) -> Option<&DiagnosticMessageTruncation> {
        self.message_truncation.as_deref()
    }

    /// Returns the affected path when the operation is path-specific.
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Returns the closed operating-system error class when a native I/O boundary supplied one.
    pub const fn io_error_class(&self) -> Option<IoErrorClass> {
        self.io_error_class
    }

    /// Returns closed HTTP or bounded-body evidence when acquisition failed without a native I/O error.
    pub const fn fetch_failure(&self) -> Option<&FetchFailureDetails> {
        self.fetch_failure.as_ref()
    }

    /// Returns retained HTMLCut failure evidence when applicable.
    pub fn htmlcut_failure(&self) -> Option<&HtmlcutFailureDetails> {
        self.htmlcut_failure.as_deref()
    }

    /// Returns a closed integration-fault code when applicable.
    pub const fn integration_fault_code(&self) -> Option<IntegrationFaultCode> {
        self.integration_fault_code
    }

    /// Returns failed process attempt facts when this is a delivery failure detail.
    pub fn delivery_failure_attempt(&self) -> Option<&DeliveryProcessAttempt> {
        match self.delivery_process.as_deref() {
            Some(DeliveryProcessDetail::Failure { attempt, .. }) => Some(attempt),
            Some(DeliveryProcessDetail::Observability { .. }) | None => None,
        }
    }

    /// Returns the deterministic process-delivery primary failure category when applicable.
    pub fn delivery_failure_primary(&self) -> Option<DeliveryFailurePrimary> {
        match self.delivery_process.as_deref() {
            Some(DeliveryProcessDetail::Failure { primary, .. }) => Some(*primary),
            Some(DeliveryProcessDetail::Observability { .. }) | None => None,
        }
    }

    /// Returns the paired failed-process facts whose association is validated as one carrier.
    pub fn delivery_failure_facts(
        &self,
    ) -> Option<(&DeliveryProcessAttempt, DeliveryFailurePrimary)> {
        match self.delivery_process.as_deref() {
            Some(DeliveryProcessDetail::Failure { attempt, primary }) => Some((attempt, *primary)),
            Some(DeliveryProcessDetail::Observability { .. }) | None => None,
        }
    }

    /// Returns a delivered-event stderr observability problem when applicable.
    pub fn stderr_capture_problem(&self) -> Option<&StderrCaptureProblem> {
        match self.delivery_process.as_deref() {
            Some(DeliveryProcessDetail::Observability {
                stderr_capture_problem,
            }) => Some(stderr_capture_problem),
            Some(DeliveryProcessDetail::Failure { .. }) | None => None,
        }
    }

    /// Validates the serialized diagnostic shape and all closed derivations.
    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        if self.message.is_empty() || self.message.len() > DIAGNOSTIC_MESSAGE_LIMIT {
            return Err(CoreError::contract(
                "diagnostic message must be non-empty and at most 1024 bytes",
            ));
        }
        if let Some(truncation) = &self.message_truncation {
            truncation.validate(self.message.len())?;
        }
        let owns_htmlcut_evidence = self.kind == DiagnosticKind::Htmlcut
            && self.operation == DiagnosticOperation::HtmlExtraction
            && self.htmlcut_failure.is_some();
        if (self.kind == DiagnosticKind::Htmlcut) != owns_htmlcut_evidence
            || self.htmlcut_failure.is_some() != owns_htmlcut_evidence
        {
            return Err(CoreError::contract(
                "HTMLCut diagnostics require htmlcut_failure evidence and html_extraction operation",
            ));
        }
        if let Some(failure) = &self.htmlcut_failure {
            failure.validate()?;
        }
        if self.io_error_class.is_some() && self.kind != DiagnosticKind::Io {
            return Err(CoreError::contract(
                "io_error_class requires io diagnostic kind",
            ));
        }
        if self.io_error_class.is_some() && self.fetch_failure.is_some() {
            return Err(CoreError::contract(
                "native I/O and fetch_failure evidence are mutually exclusive",
            ));
        }
        if self.kind == DiagnosticKind::Io
            && self.io_error_class.is_none()
            && self.fetch_failure.is_none()
        {
            return Err(CoreError::contract(
                "io diagnostics require native I/O or fetch_failure evidence",
            ));
        }
        if let Some(fetch_failure) = &self.fetch_failure {
            if self.kind != DiagnosticKind::Io {
                return Err(CoreError::contract(
                    "fetch_failure requires io diagnostic kind",
                ));
            }
            fetch_failure.validate_for(self.operation)?;
        }
        let owns_delivery_process = self.kind == DiagnosticKind::Delivery
            && self.operation == DiagnosticOperation::DeliveryProcess
            && self.delivery_process.is_some();
        if (self.kind == DiagnosticKind::Delivery) != owns_delivery_process
            || self.delivery_process.is_some() != owns_delivery_process
        {
            return Err(CoreError::contract(
                "delivery diagnostic requires delivery_process evidence",
            ));
        }
        if let Some(code) = self.integration_fault_code {
            let valid_owner = matches!(
                (
                    code,
                    self.kind,
                    self.operation,
                    self.htmlcut_failure
                        .as_deref()
                        .map(HtmlcutFailureDetails::error_class),
                ),
                (
                    IntegrationFaultCode::HtmlcutInternalError,
                    DiagnosticKind::Htmlcut,
                    DiagnosticOperation::HtmlExtraction,
                    Some(HtmlcutErrorClass::InternalError),
                ) | (
                    IntegrationFaultCode::FfhnBoundaryInvariantViolation,
                    DiagnosticKind::Htmlcut,
                    DiagnosticOperation::HtmlExtraction,
                    Some(HtmlcutErrorClass::FfhnBoundaryInvariantViolation),
                ) | (
                    IntegrationFaultCode::FfhnPolicyInvariantViolation,
                    DiagnosticKind::PolicyInvariant,
                    DiagnosticOperation::PolicyEvaluation,
                    None,
                )
            );
            if !valid_owner {
                return Err(CoreError::contract(
                    "integration_fault_code must match its diagnostic kind and operation",
                ));
            }
        }
        if self.kind == DiagnosticKind::PolicyInvariant
            && (self.operation != DiagnosticOperation::PolicyEvaluation
                || self.integration_fault_code
                    != Some(IntegrationFaultCode::FfhnPolicyInvariantViolation))
        {
            return Err(CoreError::contract(
                "policy-invariant diagnostics require the policy-evaluation integration-fault code",
            ));
        }
        if let Some(failure) = self.htmlcut_failure.as_deref() {
            let required_integration_fault_code = match failure.error_class() {
                HtmlcutErrorClass::InternalError => {
                    Some(IntegrationFaultCode::HtmlcutInternalError)
                }
                HtmlcutErrorClass::FfhnBoundaryInvariantViolation => {
                    Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation)
                }
                HtmlcutErrorClass::PlanInvalid
                | HtmlcutErrorClass::NoMatch
                | HtmlcutErrorClass::AmbiguousMatch
                | HtmlcutErrorClass::MissingAttribute => None,
            };
            if self.integration_fault_code != required_integration_fault_code {
                return Err(CoreError::contract(
                    "HTMLCut error class and integration-fault code must agree exactly",
                ));
            }
        }
        match self.delivery_process.as_deref() {
            Some(DeliveryProcessDetail::Failure { attempt, primary }) => {
                attempt.validate()?;
                if attempt.primary() != Some(*primary) {
                    return Err(CoreError::contract(
                        "delivery primary must equal the total attempt derivation",
                    ));
                }
            }
            Some(DeliveryProcessDetail::Observability {
                stderr_capture_problem,
            }) => match stderr_capture_problem {
                StderrCaptureProblem::ReadFailed { partial, .. } => partial.validate()?,
                StderrCaptureProblem::ReaderUnavailable | StderrCaptureProblem::ReaderPanicked => {}
            },
            None => {}
        }
        Ok(())
    }

    /// Validates the narrow persisted delivery-failure shape and its durable JSON budget.
    pub(crate) fn validate_durable_delivery_failure(&self) -> Result<(), CoreError> {
        self.validate()?;
        if !matches!(
            self.delivery_process.as_deref(),
            Some(DeliveryProcessDetail::Failure { .. })
        ) {
            return Err(CoreError::contract(
                "outbox last_error_detail must be a delivery_process failure",
            ));
        }
        let stable = stable_json(self)?;
        if stable.len() > DURABLE_DELIVERY_DETAIL_LIMIT {
            return Err(CoreError::contract(
                "outbox last_error_detail exceeds the durable stable-JSON budget",
            ));
        }
        Ok(())
    }

    pub(crate) fn fit_durable_delivery_failure(mut self) -> Result<Self, CoreError> {
        self.validate()?;
        while stable_json(&self)?.len() > DURABLE_DELIVERY_DETAIL_LIMIT {
            let changed = match self.delivery_process.as_deref_mut() {
                Some(DeliveryProcessDetail::Failure { attempt, .. }) => {
                    attempt.shorten_stderr_one_byte()
                }
                Some(DeliveryProcessDetail::Observability { .. }) | None => false,
            };
            if !changed {
                return Err(CoreError::internal(
                    "bounded delivery failure detail cannot fit its durable JSON budget",
                ));
            }
        }
        self.validate_durable_delivery_failure()?;
        Ok(self)
    }

    pub(crate) fn is_delivery_failure(&self) -> bool {
        matches!(
            self.delivery_process.as_deref(),
            Some(DeliveryProcessDetail::Failure { .. })
        )
    }

    pub(crate) fn is_delivery_observability(&self) -> bool {
        matches!(
            self.delivery_process.as_deref(),
            Some(DeliveryProcessDetail::Observability { .. })
        )
    }
}

pub(crate) fn bounded_message_evidence(
    mut message: String,
) -> (String, Option<Box<DiagnosticMessageTruncation>>) {
    if message.len() <= DIAGNOSTIC_MESSAGE_LIMIT {
        return (message, None);
    }
    let truncation = Box::new(DiagnosticMessageTruncation::from_original(&message));
    truncate_utf8(&mut message, DIAGNOSTIC_MESSAGE_LIMIT);
    (message, Some(truncation))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn truncate_utf8(value: &mut String, limit: usize) -> bool {
    if value.len() <= limit {
        return false;
    }
    let mut boundary = limit;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
    true
}

#[cfg(test)]
#[path = "types/tests.rs"]
mod tests;

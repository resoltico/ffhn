//! The sole FFHN-owned translation boundary for serialized diagnostics.

use serde::{Deserialize, Deserializer};

use crate::{CoreError, IntegrationFaultCode};

use super::{
    DeliveryProcessAttempt, DeliveryProcessDetail, DiagnosticDetail, DiagnosticKind,
    DiagnosticOperation, FetchFailureDetails, HtmlcutFailureDetails, IoErrorClass,
    StderrCaptureProblem, bounded_message_evidence,
};

/// Deserialization wire form. It is deliberately private so only this translation module can
/// create a `DiagnosticDetail`, including from persisted or report JSON.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticDetailWire {
    kind: DiagnosticKind,
    operation: DiagnosticOperation,
    message: String,
    #[serde(default)]
    message_truncation: Option<Box<super::DiagnosticMessageTruncation>>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    io_error_class: Option<IoErrorClass>,
    #[serde(default)]
    fetch_failure: Option<FetchFailureDetails>,
    #[serde(default)]
    htmlcut_failure: Option<Box<HtmlcutFailureDetails>>,
    #[serde(default)]
    integration_fault_code: Option<IntegrationFaultCode>,
    #[serde(default)]
    delivery_process: Option<DeliveryProcessDetail>,
}

impl<'de> Deserialize<'de> for DiagnosticDetail {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DiagnosticDetailWire::deserialize(deserializer)?;
        let detail = assembled(
            wire.kind,
            wire.operation,
            wire.message,
            wire.message_truncation,
            wire.path,
            wire.io_error_class,
            wire.fetch_failure,
            wire.htmlcut_failure,
            wire.integration_fault_code,
            wire.delivery_process,
        );
        detail.validate().map_err(serde::de::Error::custom)?;
        Ok(detail)
    }
}

/// Builds a plain FFHN-owned diagnostic with classification separate from its payload message.
pub(crate) fn plain_detail(
    kind: DiagnosticKind,
    operation: DiagnosticOperation,
    message: impl Into<String>,
    path: Option<String>,
) -> DiagnosticDetail {
    let (message, message_truncation) =
        bounded_message_evidence(strip_diagnostic_classification_prefixes(message.into()));
    assembled(
        kind,
        operation,
        message,
        message_truncation,
        path,
        None,
        None,
        None,
        None,
        None,
    )
}

/// Decomposes one core error into FFHN's closed diagnostic dimensions.
pub(crate) fn detail_from_core_error(
    error: &CoreError,
    operation: DiagnosticOperation,
    path: Option<String>,
) -> DiagnosticDetail {
    match error {
        CoreError::Io { source, .. } => io_detail(
            IoErrorClass::from_error(source),
            operation,
            "the operating-system I/O operation did not complete",
            path,
        ),
        CoreError::Json(_) => plain_detail(
            DiagnosticKind::Json,
            operation,
            "the JSON document could not be decoded or encoded",
            path,
        ),
        CoreError::Toml(_) => plain_detail(
            DiagnosticKind::Toml,
            operation,
            "the TOML document could not be decoded",
            path,
        ),
        CoreError::TargetDecode(error) => plain_detail(
            DiagnosticKind::Toml,
            operation,
            error.diagnostic_message(),
            path,
        ),
        CoreError::Url(_) => plain_detail(
            DiagnosticKind::Contract,
            operation,
            "a required URL could not be interpreted",
            path,
        ),
        CoreError::TimeFormat(_) | CoreError::TimeParse(_) => plain_detail(
            DiagnosticKind::Contract,
            operation,
            "a required timestamp could not be interpreted",
            path,
        ),
        CoreError::PolicyInvariant(message) => integration_detail(
            DiagnosticKind::PolicyInvariant,
            DiagnosticOperation::PolicyEvaluation,
            message.clone(),
            IntegrationFaultCode::FfhnPolicyInvariantViolation,
        ),
        CoreError::Contract(message) | CoreError::Internal(message) => {
            plain_detail(DiagnosticKind::Contract, operation, message.clone(), path)
        }
    }
}

/// Builds an I/O diagnostic from the closed native-error fact, never its rendered prose.
pub(crate) fn io_detail(
    io_error_class: IoErrorClass,
    operation: DiagnosticOperation,
    message: impl Into<String>,
    path: Option<String>,
) -> DiagnosticDetail {
    let (message, message_truncation) =
        bounded_message_evidence(strip_diagnostic_classification_prefixes(message.into()));
    assembled(
        DiagnosticKind::Io,
        operation,
        message,
        message_truncation,
        path,
        Some(io_error_class),
        None,
        None,
        None,
        None,
    )
}

/// Builds a typed bounded-source or HTTP-response acquisition failure.
pub(crate) fn fetch_detail(
    operation: DiagnosticOperation,
    message: impl Into<String>,
    path: Option<String>,
    fetch_failure: FetchFailureDetails,
) -> DiagnosticDetail {
    let (message, message_truncation) =
        bounded_message_evidence(strip_diagnostic_classification_prefixes(message.into()));
    assembled(
        DiagnosticKind::Io,
        operation,
        message,
        message_truncation,
        path,
        None,
        Some(fetch_failure),
        None,
        None,
        None,
    )
}

/// Builds an HTMLCut diagnostic while preserving the one validated upstream evidence boundary.
pub(crate) fn htmlcut_detail(
    message: impl Into<String>,
    failure: HtmlcutFailureDetails,
    integration_fault_code: Option<IntegrationFaultCode>,
) -> DiagnosticDetail {
    assembled(
        DiagnosticKind::Htmlcut,
        DiagnosticOperation::HtmlExtraction,
        // HTMLCut owns and validates this public message boundary. FFHN must retain it verbatim:
        // truncating an upstream contract violation would be a local workaround, not evidence.
        message.into(),
        None,
        None,
        None,
        None,
        Some(Box::new(failure)),
        integration_fault_code,
        None,
    )
}

/// Builds a non-HTMLCut integration-fault diagnostic without encoding classification into prose.
pub(crate) fn integration_detail(
    kind: DiagnosticKind,
    operation: DiagnosticOperation,
    message: impl Into<String>,
    code: IntegrationFaultCode,
) -> DiagnosticDetail {
    let (message, message_truncation) = bounded_message_evidence(message.into());
    assembled(
        kind,
        operation,
        message,
        message_truncation,
        None,
        None,
        None,
        None,
        Some(code),
        None,
    )
}

/// Builds a failed process-delivery diagnostic after preserving all independent attempt facts.
pub(crate) fn delivery_failure_detail(
    attempt: DeliveryProcessAttempt,
) -> Result<DiagnosticDetail, CoreError> {
    attempt.validate()?;
    let Some(primary) = attempt.primary() else {
        return Err(CoreError::contract(
            "successful delivery cannot carry a failure diagnostic",
        ));
    };
    assembled(
        DiagnosticKind::Delivery,
        DiagnosticOperation::DeliveryProcess,
        "delivery process did not complete successfully".to_owned(),
        None,
        None,
        None,
        None,
        None,
        None,
        Some(DeliveryProcessDetail::Failure { primary, attempt }),
    )
    .fit_durable_delivery_failure()
}

/// Builds a successful-delivery stderr-capture observability diagnostic.
pub(crate) fn delivery_observability_detail(problem: StderrCaptureProblem) -> DiagnosticDetail {
    assembled(
        DiagnosticKind::Delivery,
        DiagnosticOperation::DeliveryProcess,
        "delivery completed but stderr capture was incomplete".to_owned(),
        None,
        None,
        None,
        None,
        None,
        None,
        Some(DeliveryProcessDetail::Observability {
            stderr_capture_problem: problem,
        }),
    )
}

/// Creates a diagnostic after every public or durable carrier has crossed this one owner.
#[allow(clippy::too_many_arguments)]
fn assembled(
    kind: DiagnosticKind,
    operation: DiagnosticOperation,
    message: String,
    message_truncation: Option<Box<super::DiagnosticMessageTruncation>>,
    path: Option<String>,
    io_error_class: Option<IoErrorClass>,
    fetch_failure: Option<FetchFailureDetails>,
    htmlcut_failure: Option<Box<HtmlcutFailureDetails>>,
    integration_fault_code: Option<IntegrationFaultCode>,
    delivery_process: Option<DeliveryProcessDetail>,
) -> DiagnosticDetail {
    DiagnosticDetail {
        kind,
        operation,
        message,
        message_truncation,
        path,
        io_error_class,
        fetch_failure,
        htmlcut_failure,
        integration_fault_code,
        delivery_process: delivery_process.map(Box::new),
    }
}

/// Builds deliberately malformed carriers for closed-contract tests without creating a second
/// production construction path. Test scenarios need to exercise `DiagnosticDetail::validate`
/// before the serde boundary rejects an invalid wire shape.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn unvalidated_detail_for_contract_test(
    kind: DiagnosticKind,
    operation: DiagnosticOperation,
    message: String,
    path: Option<String>,
    io_error_class: Option<IoErrorClass>,
    htmlcut_failure: Option<Box<HtmlcutFailureDetails>>,
    integration_fault_code: Option<IntegrationFaultCode>,
    delivery_process: Option<DeliveryProcessDetail>,
) -> DiagnosticDetail {
    assembled(
        kind,
        operation,
        message,
        None,
        path,
        io_error_class,
        None,
        htmlcut_failure,
        integration_fault_code,
        delivery_process,
    )
}

/// Builds a deliberately malformed fetch-evidence owner for closed-contract tests.
#[cfg(test)]
pub(super) fn unvalidated_fetch_detail_for_contract_test(
    kind: DiagnosticKind,
    operation: DiagnosticOperation,
    fetch_failure: FetchFailureDetails,
) -> DiagnosticDetail {
    assembled(
        kind,
        operation,
        "test diagnostic".to_owned(),
        None,
        None,
        None,
        Some(fetch_failure),
        None,
        None,
        None,
    )
}

#[cfg(test)]
#[path = "construction/tests.rs"]
mod tests;

/// Removes a closed diagnostic-category prefix that an earlier boundary rendered into prose.
/// The public `kind` field owns classification; `message` remains payload only.
fn strip_diagnostic_classification_prefixes(mut message: String) -> String {
    const PREFIXES: [&str; 9] = [
        "contract error: ",
        "io: ",
        "json error: ",
        "toml error: ",
        "url parse error: ",
        "time formatting error: ",
        "time parsing error: ",
        "policy invariant error: ",
        "internal error: ",
    ];
    loop {
        let Some(prefix) = PREFIXES.iter().find(|prefix| message.starts_with(**prefix)) else {
            return message;
        };
        message = message[prefix.len()..].to_owned();
    }
}

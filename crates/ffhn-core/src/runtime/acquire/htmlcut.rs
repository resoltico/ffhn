use std::collections::BTreeMap;

use htmlcut_core::interop::v1::{
    ErrorCode, HtmlInput, HttpUrl, InteropError, SelectedMatch, SelectedMatchMetadata, execute_plan,
};

use crate::model::{htmlcut_detail, plain_detail};
use crate::{
    DiagnosticKind, DiagnosticOperation, HtmlcutBoundaryEvidence, HtmlcutDiagnostic,
    HtmlcutDiagnosticCode, HtmlcutErrorClass, HtmlcutFailureDetails, IntegrationFaultCode,
    PermanentErrorCode, SourceSuspectReason, TargetDocument, TargetSource,
};

use super::{AcquisitionFailure, MeasurementAcquisitionFailure};

/// Closed FFHN-side reasons an otherwise valid target cannot cross the HTMLCut input boundary.
enum HtmlcutInputIssue {
    EmptyLabel,
    InvalidHttpBaseUrl,
}

impl HtmlcutInputIssue {
    const fn message(self) -> &'static str {
        match self {
            Self::EmptyLabel => "the HTML input label is invalid",
            Self::InvalidHttpBaseUrl => "the HTTP base URL is invalid for HTMLCut",
        }
    }
}

pub(in crate::runtime) fn html_text_comparison_projection(
    dom_canonicalization_requested: bool,
    comparison_text_output: Option<String>,
    raw_selected: &str,
    plan_digest_sha256: &str,
    candidate_count: usize,
    diagnostics: &[HtmlcutDiagnostic],
) -> Result<String, MeasurementAcquisitionFailure> {
    match (dom_canonicalization_requested, comparison_text_output) {
        (true, Some(comparison_projection)) => Ok(comparison_projection),
        (false, None) => Ok(raw_selected.to_owned()),
        (true, None) => Err(html_boundary_invariant_violation(
            "HTMLCut omitted the detached-clone comparison text requested by dom_canonicalization",
            plan_digest_sha256.to_owned(),
            Some(candidate_count),
            diagnostics.to_vec(),
        )),
        (false, Some(_)) => Err(html_boundary_invariant_violation(
            "HTMLCut returned detached-clone comparison text without dom_canonicalization",
            plan_digest_sha256.to_owned(),
            Some(candidate_count),
            diagnostics.to_vec(),
        )),
    }
}

/// Requires the first-class DOM plain-text evidence carried by a successful CSS selection.
pub(in crate::runtime) fn required_plain_text_output(
    plain_text_output: Option<String>,
    plan_digest_sha256: &str,
    candidate_count: usize,
    diagnostics: &[HtmlcutDiagnostic],
) -> Result<String, MeasurementAcquisitionFailure> {
    plain_text_output.ok_or_else(|| {
        html_boundary_invariant_violation(
            "HTMLCut omitted plain-text evidence for a successful CSS selection",
            plan_digest_sha256.to_owned(),
            Some(candidate_count),
            diagnostics.to_vec(),
        )
    })
}

pub(in crate::runtime) fn required_selected_match(
    selected_matches: Vec<SelectedMatch>,
    plan_digest_sha256: &str,
    candidate_count: usize,
    diagnostics: &[HtmlcutDiagnostic],
) -> Result<SelectedMatch, MeasurementAcquisitionFailure> {
    match selected_matches.as_slice() {
        [selected] => Ok(selected.clone()),
        _ => Err(html_boundary_invariant_violation_with_evidence(
            "HTMLCut did not produce exactly one selected match for a successful exact-one extraction",
            plan_digest_sha256.to_owned(),
            Some(candidate_count),
            diagnostics.to_vec(),
            HtmlcutBoundaryEvidence::SelectedMatchCount {
                selected_match_count: selected_matches.len(),
            },
        )),
    }
}

pub(in crate::runtime) fn css_attributes_or_failure(
    metadata: SelectedMatchMetadata,
    plan_digest_sha256: &str,
    candidate_count: usize,
    diagnostics: &[HtmlcutDiagnostic],
) -> Result<BTreeMap<String, String>, MeasurementAcquisitionFailure> {
    match metadata {
        SelectedMatchMetadata::CssSelector { attributes, .. } => Ok(attributes),
        SelectedMatchMetadata::DelimiterPair { .. } => Err(html_boundary_invariant_violation(
            "HTMLCut returned non-CSS metadata for an html_attribute projection",
            plan_digest_sha256.to_owned(),
            Some(candidate_count),
            diagnostics.to_vec(),
        )),
    }
}

pub(in crate::runtime) fn required_html_attribute(
    attributes: &BTreeMap<String, String>,
    name: &str,
    plan_digest_sha256: &str,
    candidate_count: usize,
    diagnostics: &[HtmlcutDiagnostic],
) -> Result<String, MeasurementAcquisitionFailure> {
    attributes.get(name).cloned().ok_or_else(|| {
        html_missing_attribute_failure(
            "HTMLCut selected CSS metadata omitted the required attribute",
            plan_digest_sha256.to_owned(),
            Some(candidate_count),
            diagnostics.to_vec(),
            HtmlcutBoundaryEvidence::RequestedCssAttribute {
                attribute: name.to_owned(),
            },
        )
    })
}

pub(in crate::runtime) fn execute_htmlcut_plan(
    target: &TargetDocument,
    body: &str,
    effective_http_url: Option<&url::Url>,
    plan: htmlcut_core::interop::v1::Plan,
) -> Result<htmlcut_core::interop::v1::InteropResult, MeasurementAcquisitionFailure> {
    let source = html_input(target, body, effective_http_url)?;
    execute_plan(&source, &plan).map_err(|error| classify_htmlcut_error(*error))
}

pub(in crate::runtime) fn html_input(
    target: &TargetDocument,
    body: &str,
    effective_http_url: Option<&url::Url>,
) -> Result<HtmlInput, MeasurementAcquisitionFailure> {
    match target.source() {
        TargetSource::File { file_path } => htmlcut_input_from_label(file_path, body),
        TargetSource::Http { source_url } => {
            let input = htmlcut_input_from_label(source_url.as_str(), body)?;
            let base_url = htmlcut_http_base_url(
                effective_http_url
                    .cloned()
                    .unwrap_or_else(|| source_url.clone()),
            )?;
            Ok(input.with_input_base_url(base_url))
        }
    }
}

pub(in crate::runtime) fn htmlcut_input_from_label(
    label: &str,
    body: &str,
) -> Result<HtmlInput, MeasurementAcquisitionFailure> {
    HtmlInput::new(label, body).map_err(|_| htmlcut_input_failure(HtmlcutInputIssue::EmptyLabel))
}

pub(in crate::runtime) fn htmlcut_http_base_url(
    value: url::Url,
) -> Result<HttpUrl, MeasurementAcquisitionFailure> {
    HttpUrl::try_from(value)
        .map_err(|_| htmlcut_input_failure(HtmlcutInputIssue::InvalidHttpBaseUrl))
}

fn htmlcut_input_failure(issue: HtmlcutInputIssue) -> MeasurementAcquisitionFailure {
    MeasurementAcquisitionFailure::Permanent {
        code: PermanentErrorCode::HtmlcutInputInvalid,
        detail: plain_detail(
            DiagnosticKind::Contract,
            DiagnosticOperation::HtmlExtraction,
            issue.message(),
            None,
        ),
    }
}

pub(in crate::runtime) fn classify_htmlcut_error(
    error: InteropError,
) -> MeasurementAcquisitionFailure {
    let plan_digest_sha256 = error.plan_digest_sha256.clone();
    let htmlcut_failure = match HtmlcutFailureDetails::from_interop_error(&error) {
        Ok(failure) => failure,
        Err(_) => {
            return html_boundary_invariant_violation(
                "HTMLCut returned error evidence outside FFHN's pinned contract",
                plan_digest_sha256,
                None,
                Vec::new(),
            );
        }
    };
    let core_diagnostic_code = htmlcut_failure.core_diagnostic_code();
    let detail = htmlcut_detail(
        error.message.clone(),
        htmlcut_failure,
        (error.error_code == ErrorCode::InternalError)
            .then_some(IntegrationFaultCode::HtmlcutInternalError),
    );
    match error.error_code {
        ErrorCode::InternalError => MeasurementAcquisitionFailure::Integration { detail },
        ErrorCode::PlanInvalid => MeasurementAcquisitionFailure::Permanent {
            code: permanent_code_for_plan_invalid_diagnostic(core_diagnostic_code),
            detail,
        },
        ErrorCode::NoMatch => MeasurementAcquisitionFailure::SourceSuspect(AcquisitionFailure {
            reason: source_suspect_reason_for_no_match_diagnostic(core_diagnostic_code),
            detail,
        }),
        ErrorCode::AmbiguousMatch => {
            MeasurementAcquisitionFailure::SourceSuspect(AcquisitionFailure {
                reason: SourceSuspectReason::HtmlcutAmbiguousMatch,
                detail,
            })
        }
        ErrorCode::MissingAttribute => {
            MeasurementAcquisitionFailure::SourceSuspect(AcquisitionFailure {
                reason: SourceSuspectReason::HtmlcutMissingAttribute,
                detail,
            })
        }
    }
}

pub(in crate::runtime) fn permanent_code_for_plan_invalid_diagnostic(
    core_diagnostic_code: Option<HtmlcutDiagnosticCode>,
) -> PermanentErrorCode {
    match core_diagnostic_code {
        Some(HtmlcutDiagnosticCode::InvalidSelector) => PermanentErrorCode::HtmlcutInvalidSelector,
        Some(HtmlcutDiagnosticCode::InvalidSlicePattern) => {
            PermanentErrorCode::HtmlcutInvalidSlicePattern
        }
        _ => PermanentErrorCode::HtmlcutPlanInvalid,
    }
}

pub(in crate::runtime) fn source_suspect_reason_for_no_match_diagnostic(
    core_diagnostic_code: Option<HtmlcutDiagnosticCode>,
) -> SourceSuspectReason {
    match core_diagnostic_code {
        Some(HtmlcutDiagnosticCode::MatchIndexOutOfRange) => {
            SourceSuspectReason::HtmlcutMatchIndexOutOfRange
        }
        _ => SourceSuspectReason::HtmlcutNoMatch,
    }
}

fn html_missing_attribute_failure(
    message: impl Into<String>,
    plan_digest_sha256: String,
    candidate_count: Option<usize>,
    diagnostics: Vec<HtmlcutDiagnostic>,
    boundary_evidence: HtmlcutBoundaryEvidence,
) -> MeasurementAcquisitionFailure {
    let failure = HtmlcutFailureDetails::new(
        HtmlcutErrorClass::MissingAttribute,
        candidate_count,
        plan_digest_sha256,
        diagnostics,
    )
    .with_boundary_evidence(boundary_evidence);
    MeasurementAcquisitionFailure::SourceSuspect(AcquisitionFailure {
        reason: SourceSuspectReason::HtmlcutMissingAttribute,
        detail: htmlcut_detail(message, failure, None),
    })
}

pub(in crate::runtime) fn html_boundary_invariant_violation(
    message: impl Into<String>,
    plan_digest_sha256: String,
    candidate_count: Option<usize>,
    diagnostics: Vec<HtmlcutDiagnostic>,
) -> MeasurementAcquisitionFailure {
    html_boundary_invariant_violation_with_optional_evidence(
        message,
        plan_digest_sha256,
        candidate_count,
        diagnostics,
        None,
    )
}

fn html_boundary_invariant_violation_with_evidence(
    message: impl Into<String>,
    plan_digest_sha256: String,
    candidate_count: Option<usize>,
    diagnostics: Vec<HtmlcutDiagnostic>,
    boundary_evidence: HtmlcutBoundaryEvidence,
) -> MeasurementAcquisitionFailure {
    html_boundary_invariant_violation_with_optional_evidence(
        message,
        plan_digest_sha256,
        candidate_count,
        diagnostics,
        Some(boundary_evidence),
    )
}

fn html_boundary_invariant_violation_with_optional_evidence(
    message: impl Into<String>,
    plan_digest_sha256: String,
    candidate_count: Option<usize>,
    diagnostics: Vec<HtmlcutDiagnostic>,
    boundary_evidence: Option<HtmlcutBoundaryEvidence>,
) -> MeasurementAcquisitionFailure {
    MeasurementAcquisitionFailure::Integration {
        detail: htmlcut_detail(
            message,
            HtmlcutFailureDetails::new(
                HtmlcutErrorClass::FfhnBoundaryInvariantViolation,
                candidate_count,
                plan_digest_sha256,
                diagnostics,
            )
            .with_optional_boundary_evidence(boundary_evidence),
            Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation),
        ),
    }
}

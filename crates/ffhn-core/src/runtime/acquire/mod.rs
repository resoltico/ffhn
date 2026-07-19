//! Measurement acquisition decomposed into projection coordination, the HTMLCut boundary, and source transport.

mod fetch;
mod htmlcut;
#[cfg(test)]
mod tests;

use htmlcut_core::interop::v1::InteropDiagnostic;

use crate::model::HtmlObservationInput;
use crate::{
    AcquisitionKind, DiagnosticDetail, HtmlSelection, HtmlcutDiagnostic, PermanentErrorCode,
    Projection, SourceSuspectReason, TargetDocument,
};

pub(super) use fetch::*;
pub(super) use htmlcut::*;

#[derive(Debug)]
pub(super) struct AcquisitionFailure {
    pub(super) detail: DiagnosticDetail,
    pub(super) reason: SourceSuspectReason,
}

pub(super) enum AcquiredMeasurement {
    JsonScalar(String),
    Html(HtmlObservationInput),
}

/// Decoded source text plus the HTTP representation that actually supplied it.
///
/// An HTMLCut input base must follow a successful redirect, because relative URL resolution is a
/// property of the received representation rather than the configured request URL.
pub(super) struct FetchedSource {
    pub(super) body: String,
    pub(super) effective_http_url: Option<url::Url>,
}

#[derive(Debug)]
pub(super) enum MeasurementAcquisitionFailure {
    SourceSuspect(AcquisitionFailure),
    Permanent {
        code: PermanentErrorCode,
        detail: DiagnosticDetail,
    },
    Integration {
        detail: DiagnosticDetail,
    },
}

pub(super) fn acquire_json_scalar(pointer: &str, body: &str) -> Result<String, AcquisitionFailure> {
    crate::model::select_json_scalar_token(body, pointer).map_err(|failure| AcquisitionFailure {
        reason: failure.reason(),
        detail: failure.into_detail(),
    })
}

pub(super) fn acquire_measurement(
    target: &TargetDocument,
    body: &str,
    effective_http_url: Option<&url::Url>,
) -> Result<AcquiredMeasurement, MeasurementAcquisitionFailure> {
    match target.projection() {
        Projection::JsonPointer { pointer } => acquire_json_scalar(pointer, body)
            .map(AcquiredMeasurement::JsonScalar)
            .map_err(MeasurementAcquisitionFailure::SourceSuspect),
        Projection::HtmlText { selection } => acquire_html_text(
            target,
            body,
            effective_http_url,
            selection,
            HtmlTextProjection::Plain,
        ),
        Projection::HtmlRenderedText { selection } => acquire_html_text(
            target,
            body,
            effective_http_url,
            selection,
            HtmlTextProjection::Rendered,
        ),
        Projection::HtmlAttribute { selection, name } => {
            let result = execute_htmlcut_plan(
                target,
                body,
                effective_http_url,
                selection.structured_plan(),
            )?;
            let plan_digest_sha256 = result.plan_digest_sha256;
            let candidate_count = result.candidate_count;
            let diagnostics = retain_htmlcut_diagnostics(
                result.diagnostics,
                &plan_digest_sha256,
                candidate_count,
            )?;
            let selected = required_selected_match(
                result.selected_matches,
                &plan_digest_sha256,
                candidate_count,
                &diagnostics,
            )?;
            let attributes = css_attributes_or_failure(
                selected.metadata,
                &plan_digest_sha256,
                candidate_count,
                &diagnostics,
            )?;
            let attribute = required_html_attribute(
                &attributes,
                name.as_str(),
                &plan_digest_sha256,
                candidate_count,
                &diagnostics,
            )?;
            Ok(AcquiredMeasurement::Html(HtmlObservationInput {
                raw_selected: attribute.clone(),
                comparison_projection: attribute,
                acquisition_kind: AcquisitionKind::HtmlAttribute,
                plan_digest_sha256,
                candidate_count,
                diagnostics,
            }))
        }
    }
}

#[derive(Clone, Copy)]
enum HtmlTextProjection {
    Plain,
    Rendered,
}

fn acquire_html_text(
    target: &TargetDocument,
    body: &str,
    effective_http_url: Option<&url::Url>,
    selection: &HtmlSelection,
    projection: HtmlTextProjection,
) -> Result<AcquiredMeasurement, MeasurementAcquisitionFailure> {
    let result = execute_htmlcut_plan(
        target,
        body,
        effective_http_url,
        selection.structured_plan(),
    )?;
    let plan_digest_sha256 = result.plan_digest_sha256;
    let candidate_count = result.candidate_count;
    let diagnostics =
        retain_htmlcut_diagnostics(result.diagnostics, &plan_digest_sha256, candidate_count)?;
    let selected = required_selected_match(
        result.selected_matches,
        &plan_digest_sha256,
        candidate_count,
        &diagnostics,
    )?;
    let (raw_selected, comparison_output, acquisition_kind) = match projection {
        HtmlTextProjection::Plain => (
            required_plain_text_output(
                selected.plain_text_output,
                &plan_digest_sha256,
                candidate_count,
                &diagnostics,
            )?,
            selected.comparison_plain_text_output,
            AcquisitionKind::HtmlPlainText,
        ),
        HtmlTextProjection::Rendered => (
            selected.text_output,
            selected.comparison_text_output,
            AcquisitionKind::HtmlRenderedText,
        ),
    };
    let comparison_projection = html_text_comparison_projection(
        selection.dom_canonicalization().is_some(),
        comparison_output,
        &raw_selected,
        &plan_digest_sha256,
        candidate_count,
        &diagnostics,
    )?;
    Ok(AcquiredMeasurement::Html(HtmlObservationInput {
        raw_selected,
        comparison_projection,
        acquisition_kind,
        plan_digest_sha256,
        candidate_count,
        diagnostics,
    }))
}

fn retain_htmlcut_diagnostics(
    diagnostics: Vec<InteropDiagnostic>,
    plan_digest_sha256: &str,
    candidate_count: usize,
) -> Result<Vec<HtmlcutDiagnostic>, MeasurementAcquisitionFailure> {
    diagnostics
        .into_iter()
        .map(HtmlcutDiagnostic::from_interop)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            html_boundary_invariant_violation(
                "HTMLCut returned diagnostic evidence outside FFHN's pinned contract",
                plan_digest_sha256.to_owned(),
                Some(candidate_count),
                Vec::new(),
            )
        })
}

//! Prepared, source-shared measurement projection over one in-memory document.

use htmlcut_core::interop::v1::{
    ErrorCode, HtmlInput, HttpUrl, InteropDiagnosticCode, PlanStrategy, SelectedMatchMetadata,
    Selection, ValidatedPlan, execute_validated_plan, prepare_plan,
};

use crate::{CoreError, Observation, Projection};

use super::{
    ExtractionFailureReason, GraphIntegrationFaultCode, MeasurementDocument, SourceDocument,
    SourceDocumentBytes,
};

/// The only projection-time failures a config-valid, lineage-valid measurement can produce.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasurementProjectionFailure {
    /// The source document could not satisfy this measurement's extraction or typed-value contract.
    Extraction(ExtractionFailureReason),
    /// FFHN or HTMLCut violated an established integration boundary.
    Integration(GraphIntegrationFaultCode),
}

/// One projection prepared exactly once while a measurement configuration is loaded.
#[derive(Clone, Debug)]
pub enum PreparedMeasurementProjection {
    /// An RFC 6901 JSON scalar projection.
    Json {
        /// Validated RFC 6901 selector for one scalar token.
        pointer: String,
    },
    /// A prepared HTMLCut plan with FFHN-owned output interpretation.
    Html(Box<PreparedHtmlProjection>),
}

/// HTMLCut plan and output interpretation owned by one measurement projection.
#[derive(Clone, Debug)]
pub struct PreparedHtmlProjection {
    plan: ValidatedPlan,
    output: HtmlOutput,
}

#[derive(Clone, Debug)]
enum HtmlOutput {
    PlainText { canonicalization_requested: bool },
    RenderedText { canonicalization_requested: bool },
    Attribute { name: String },
}

impl PreparedMeasurementProjection {
    /// Validates and prepares the fixed projection part of one measurement contract.
    pub fn prepare(measurement: &MeasurementDocument) -> Result<Self, CoreError> {
        measurement.validate()?;
        match measurement.projection() {
            Projection::JsonPointer { pointer } => {
                crate::model::validate_json_pointer(pointer).map_err(CoreError::contract)?;
                Ok(Self::Json {
                    pointer: pointer.clone(),
                })
            }
            Projection::HtmlText { selection } => {
                require_css_strategy(selection.strategy(), "html_text")?;
                Self::prepare_html(
                    selection.structured_plan(),
                    HtmlOutput::PlainText {
                        canonicalization_requested: selection.dom_canonicalization().is_some(),
                    },
                )
            }
            Projection::HtmlRenderedText { selection } => Self::prepare_html(
                selection.structured_plan(),
                HtmlOutput::RenderedText {
                    canonicalization_requested: selection.dom_canonicalization().is_some(),
                },
            ),
            Projection::HtmlAttribute { selection, name } => {
                require_css_strategy(selection.strategy(), "html_attribute")?;
                if selection.dom_canonicalization().is_some() {
                    return Err(CoreError::contract(
                        "html_attribute does not accept DOM canonicalization",
                    ));
                }
                Self::prepare_html(
                    selection.structured_plan(),
                    HtmlOutput::Attribute {
                        name: name.as_str().to_owned(),
                    },
                )
            }
        }
    }

    /// Executes the projection once against the complete in-memory source document.
    pub fn execute(
        &self,
        measurement: &MeasurementDocument,
        source: &SourceDocument,
        document: &SourceDocumentBytes,
    ) -> Result<Observation, MeasurementProjectionFailure> {
        match self {
            Self::Json { pointer } => execute_json(measurement, &document.body, pointer),
            Self::Html(prepared) => prepared.execute(measurement, source, document),
        }
    }

    fn prepare_html(
        plan: htmlcut_core::interop::v1::Plan,
        output: HtmlOutput,
    ) -> Result<Self, CoreError> {
        let plan = prepare_plan(&plan).map_err(|_| {
            CoreError::contract(
                "measurement HTML projection does not satisfy the pinned HTMLCut contract",
            )
        })?;
        if matches!(plan.plan().selection, Selection::All) {
            return Err(CoreError::contract(
                "measurement HTML projection must select exactly one scalar candidate",
            ));
        }
        Ok(Self::Html(Box::new(PreparedHtmlProjection {
            plan,
            output,
        })))
    }
}

fn require_css_strategy(strategy: &PlanStrategy, projection: &str) -> Result<(), CoreError> {
    if matches!(strategy, PlanStrategy::CssSelector { .. }) {
        Ok(())
    } else {
        Err(CoreError::contract(format!(
            "{projection} requires a CSS selector strategy"
        )))
    }
}

impl PreparedHtmlProjection {
    fn execute(
        &self,
        measurement: &MeasurementDocument,
        source: &SourceDocument,
        document: &SourceDocumentBytes,
    ) -> Result<Observation, MeasurementProjectionFailure> {
        let input = html_input(source, document)?;
        let result = execute_validated_plan(&input, &self.plan).map_err(|error| {
            classify_htmlcut_error(
                error.error_code,
                error.diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == InteropDiagnosticCode::MatchIndexOutOfRange
                }),
            )
        })?;
        self.interpret_result(measurement, result)
    }

    fn interpret_result(
        &self,
        measurement: &MeasurementDocument,
        result: htmlcut_core::interop::v1::InteropResult,
    ) -> Result<Observation, MeasurementProjectionFailure> {
        let diagnostics = result
            .diagnostics
            .into_iter()
            .map(crate::HtmlcutDiagnostic::from_interop)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| {
                MeasurementProjectionFailure::Integration(
                    GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation,
                )
            })?;
        let selected = match result.selected_matches.as_slice() {
            [selected] => selected,
            _ => {
                return Err(MeasurementProjectionFailure::Integration(
                    GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation,
                ));
            }
        };
        let (raw_selected, comparison_projection, acquisition_kind) = match &self.output {
            HtmlOutput::PlainText {
                canonicalization_requested,
            } => {
                let raw = selected.plain_text_output.clone().ok_or(
                    MeasurementProjectionFailure::Integration(
                        GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation,
                    ),
                )?;
                let comparison = comparison_text(
                    *canonicalization_requested,
                    selected.comparison_plain_text_output.clone(),
                    &raw,
                )?;
                (raw, comparison, crate::AcquisitionKind::HtmlPlainText)
            }
            HtmlOutput::RenderedText {
                canonicalization_requested,
            } => {
                let raw = selected.text_output.clone();
                let comparison = comparison_text(
                    *canonicalization_requested,
                    selected.comparison_text_output.clone(),
                    &raw,
                )?;
                (raw, comparison, crate::AcquisitionKind::HtmlRenderedText)
            }
            HtmlOutput::Attribute { name } => {
                let SelectedMatchMetadata::CssSelector { attributes, .. } = &selected.metadata
                else {
                    return Err(MeasurementProjectionFailure::Integration(
                        GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation,
                    ));
                };
                let value = attributes.get(name).cloned().ok_or(
                    MeasurementProjectionFailure::Extraction(
                        ExtractionFailureReason::HtmlcutMissingAttribute,
                    ),
                )?;
                (value.clone(), value, crate::AcquisitionKind::HtmlAttribute)
            }
        };
        let input = crate::model::HtmlObservationInput {
            raw_selected,
            comparison_projection,
            acquisition_kind,
            plan_digest_sha256: result.plan_digest_sha256,
            candidate_count: result.candidate_count,
            diagnostics,
        };
        crate::model::parse_html_projection_for_contract(
            measurement.declared_type(),
            measurement.type_params(),
            input,
        )
        .map_err(|_| {
            MeasurementProjectionFailure::Extraction(ExtractionFailureReason::ValueUnparseable)
        })
    }
}

const fn classify_htmlcut_error(
    code: ErrorCode,
    indexed_no_match: bool,
) -> MeasurementProjectionFailure {
    match code {
        ErrorCode::NoMatch => MeasurementProjectionFailure::Extraction(if indexed_no_match {
            ExtractionFailureReason::HtmlcutMatchIndexOutOfRange
        } else {
            ExtractionFailureReason::HtmlcutNoMatch
        }),
        ErrorCode::AmbiguousMatch => {
            MeasurementProjectionFailure::Extraction(ExtractionFailureReason::HtmlcutAmbiguousMatch)
        }
        ErrorCode::MissingAttribute => MeasurementProjectionFailure::Extraction(
            ExtractionFailureReason::HtmlcutMissingAttribute,
        ),
        ErrorCode::InternalError => MeasurementProjectionFailure::Integration(
            GraphIntegrationFaultCode::HtmlcutInternalError,
        ),
        ErrorCode::PlanInvalid => MeasurementProjectionFailure::Integration(
            GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation,
        ),
    }
}

fn execute_json(
    measurement: &MeasurementDocument,
    body: &str,
    pointer: &str,
) -> Result<Observation, MeasurementProjectionFailure> {
    let raw =
        crate::model::select_json_scalar_token(body, pointer).map_err(|failure| match failure {
            crate::model::JsonAcquisitionFailure::Malformed => {
                MeasurementProjectionFailure::Extraction(ExtractionFailureReason::JsonMalformed)
            }
            crate::model::JsonAcquisitionFailure::MissingPointerTarget => {
                MeasurementProjectionFailure::Extraction(
                    ExtractionFailureReason::JsonMissingPointerTarget,
                )
            }
            crate::model::JsonAcquisitionFailure::NonScalarPointerTarget => {
                MeasurementProjectionFailure::Extraction(
                    ExtractionFailureReason::JsonNonScalarPointerTarget,
                )
            }
        })?;
    crate::model::parse_json_scalar_token_for_contract(
        measurement.declared_type(),
        measurement.type_params(),
        raw,
    )
    .map_err(|_| {
        MeasurementProjectionFailure::Extraction(ExtractionFailureReason::ValueUnparseable)
    })
}

fn html_input(
    source: &SourceDocument,
    document: &SourceDocumentBytes,
) -> Result<HtmlInput, MeasurementProjectionFailure> {
    match source.fetch() {
        super::SourceFetch::File { file_path, .. } => {
            build_html_input(file_path, &document.body, None)
        }
        super::SourceFetch::Http { source_url, .. } => {
            let base = document
                .effective_http_url
                .as_ref()
                .unwrap_or(source_url)
                .clone();
            build_html_input(source_url.as_str(), &document.body, Some(base))
        }
    }
}

fn build_html_input(
    source_id: &str,
    body: &str,
    base: Option<url::Url>,
) -> Result<HtmlInput, MeasurementProjectionFailure> {
    let input = HtmlInput::new(source_id, body).map_err(|_| {
        MeasurementProjectionFailure::Integration(
            GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation,
        )
    })?;
    match base {
        None => Ok(input),
        Some(base) => HttpUrl::try_from(base)
            .map(|base| input.with_input_base_url(base))
            .map_err(|_| {
                MeasurementProjectionFailure::Integration(
                    GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation,
                )
            }),
    }
}

fn comparison_text(
    canonicalization_requested: bool,
    comparison_from_htmlcut: Option<String>,
    raw: &str,
) -> Result<String, MeasurementProjectionFailure> {
    match (canonicalization_requested, comparison_from_htmlcut) {
        (false, None) => Ok(raw.to_owned()),
        (true, Some(value)) => Ok(value),
        (false, Some(_)) | (true, None) => Err(MeasurementProjectionFailure::Integration(
            GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation,
        )),
    }
}

#[cfg(test)]
#[path = "projection/tests.rs"]
mod tests;

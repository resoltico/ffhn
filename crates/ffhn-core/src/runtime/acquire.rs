use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::time::Duration;

#[cfg(test)]
use htmlcut_core::interop::v1::ErrorCode;
use htmlcut_core::interop::v1::{
    HtmlInput, HttpUrl, InteropError, SelectedMatch, SelectedMatchMetadata, execute_plan,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use ureq::{
    ResponseExt,
    tls::{RootCerts, TlsConfig},
};

use crate::model::HtmlObservationInput;
use crate::{
    AcquisitionKind, FetchConfig, HtmlcutDiagnostic, HtmlcutFailureDetails, PermanentErrorCode,
    ProcessErrorDetail, ProcessErrorKind, Projection, SourceSuspectReason, TargetDocument,
    TargetSource,
};

#[derive(Debug)]
pub(super) struct AcquisitionFailure {
    pub(super) detail: ProcessErrorDetail,
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
        detail: ProcessErrorDetail,
    },
}

pub(super) fn acquire_json_scalar(
    target: &TargetDocument,
    body: &str,
) -> Result<String, AcquisitionFailure> {
    let pointer = match target.projection() {
        Projection::JsonPointer { pointer } => pointer,
        Projection::HtmlText { .. } | Projection::HtmlAttribute { .. } => {
            return Err(AcquisitionFailure {
                reason: SourceSuspectReason::HtmlcutInternalFailure,
                detail: ProcessErrorDetail::new(
                    ProcessErrorKind::Contract,
                    "JSON scalar acquisition was requested for an HTML projection",
                    None,
                ),
            });
        }
    };
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
        Projection::JsonPointer { .. } => acquire_json_scalar(target, body)
            .map(AcquiredMeasurement::JsonScalar)
            .map_err(MeasurementAcquisitionFailure::SourceSuspect),
        Projection::HtmlText { selection } => {
            let result = execute_htmlcut_plan(
                target,
                body,
                effective_http_url,
                selection.structured_plan(),
            )?;
            let plan_digest_sha256 = result.plan_digest_sha256;
            let candidate_count = result.candidate_count;
            let diagnostics = result
                .diagnostics
                .into_iter()
                .map(HtmlcutDiagnostic::from_interop)
                .collect::<Vec<_>>();
            let selected = required_selected_match(
                result.selected_matches,
                &plan_digest_sha256,
                candidate_count,
                &diagnostics,
            )?;
            let raw_selected = selected.text_output;
            let comparison_projection = html_text_comparison_projection(
                selection.dom_canonicalization().is_some(),
                selected.comparison_text_output,
                &raw_selected,
                &plan_digest_sha256,
                candidate_count,
                &diagnostics,
            )?;
            Ok(AcquiredMeasurement::Html(HtmlObservationInput {
                raw_selected,
                comparison_projection,
                acquisition_kind: AcquisitionKind::HtmlText,
                plan_digest_sha256,
                candidate_count,
                diagnostics,
            }))
        }
        Projection::HtmlAttribute { selection, name } => {
            let result = execute_htmlcut_plan(
                target,
                body,
                effective_http_url,
                selection.structured_plan(),
            )?;
            let plan_digest_sha256 = result.plan_digest_sha256;
            let candidate_count = result.candidate_count;
            let diagnostics = result
                .diagnostics
                .into_iter()
                .map(HtmlcutDiagnostic::from_interop)
                .collect::<Vec<_>>();
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

fn html_text_comparison_projection(
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
        (true, None) => Err(html_source_suspect_failure(
            SourceSuspectReason::HtmlcutInternalFailure,
            "HTMLCut omitted the detached-clone comparison text requested by dom_canonicalization",
            plan_digest_sha256.to_owned(),
            Some(candidate_count),
            diagnostics.to_vec(),
        )),
        (false, Some(_)) => Err(html_source_suspect_failure(
            SourceSuspectReason::HtmlcutInternalFailure,
            "HTMLCut returned detached-clone comparison text without dom_canonicalization",
            plan_digest_sha256.to_owned(),
            Some(candidate_count),
            diagnostics.to_vec(),
        )),
    }
}

fn required_selected_match(
    selected_matches: Vec<SelectedMatch>,
    plan_digest_sha256: &str,
    candidate_count: usize,
    diagnostics: &[HtmlcutDiagnostic],
) -> Result<SelectedMatch, MeasurementAcquisitionFailure> {
    match selected_matches.as_slice() {
        [selected] => Ok(selected.clone()),
        _ => Err(html_source_suspect_failure(
            SourceSuspectReason::HtmlcutInternalFailure,
            format!(
                "HTMLCut produced {} selected matches for a successful exact-one extraction",
                selected_matches.len()
            ),
            plan_digest_sha256.to_owned(),
            Some(candidate_count),
            diagnostics.to_vec(),
        )),
    }
}

fn css_attributes_or_failure(
    metadata: SelectedMatchMetadata,
    plan_digest_sha256: &str,
    candidate_count: usize,
    diagnostics: &[HtmlcutDiagnostic],
) -> Result<BTreeMap<String, String>, MeasurementAcquisitionFailure> {
    match metadata {
        SelectedMatchMetadata::CssSelector { attributes, .. } => Ok(attributes),
        SelectedMatchMetadata::DelimiterPair { .. } => Err(html_source_suspect_failure(
            SourceSuspectReason::HtmlcutInternalFailure,
            "HTMLCut returned non-CSS metadata for an html_attribute projection",
            plan_digest_sha256.to_owned(),
            Some(candidate_count),
            diagnostics.to_vec(),
        )),
    }
}

fn required_html_attribute(
    attributes: &BTreeMap<String, String>,
    name: &str,
    plan_digest_sha256: &str,
    candidate_count: usize,
    diagnostics: &[HtmlcutDiagnostic],
) -> Result<String, MeasurementAcquisitionFailure> {
    attributes.get(name).cloned().ok_or_else(|| {
        html_source_suspect_failure(
            SourceSuspectReason::HtmlcutMissingAttribute,
            format!("HTMLCut selected CSS match does not have attribute {name:?}"),
            plan_digest_sha256.to_owned(),
            Some(candidate_count),
            diagnostics.to_vec(),
        )
    })
}

fn execute_htmlcut_plan(
    target: &TargetDocument,
    body: &str,
    effective_http_url: Option<&url::Url>,
    plan: htmlcut_core::interop::v1::Plan,
) -> Result<htmlcut_core::interop::v1::InteropResult, MeasurementAcquisitionFailure> {
    let source = html_input(target, body, effective_http_url)?;
    execute_plan(&source, &plan).map_err(|error| classify_htmlcut_error(*error))
}

pub(super) fn html_input(
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

fn htmlcut_input_from_label(
    label: &str,
    body: &str,
) -> Result<HtmlInput, MeasurementAcquisitionFailure> {
    HtmlInput::new(label, body).map_err(|error| {
        htmlcut_input_failure(format!("could not construct HTMLCut input: {error}"))
    })
}

fn htmlcut_http_base_url(value: url::Url) -> Result<HttpUrl, MeasurementAcquisitionFailure> {
    HttpUrl::try_from(value).map_err(|error| {
        htmlcut_input_failure(format!(
            "configured HTTP source cannot be used as an HTMLCut base URL: {error}"
        ))
    })
}

fn htmlcut_input_failure(message: String) -> MeasurementAcquisitionFailure {
    MeasurementAcquisitionFailure::Permanent {
        code: PermanentErrorCode::HtmlcutInputInvalid,
        detail: ProcessErrorDetail::new(ProcessErrorKind::Contract, message, None),
    }
}

fn classify_htmlcut_error(error: InteropError) -> MeasurementAcquisitionFailure {
    let htmlcut_failure = HtmlcutFailureDetails::from_interop_error(&error);
    let reason = htmlcut_failure.reason().to_owned();
    let detail = ProcessErrorDetail::new(ProcessErrorKind::Htmlcut, error.message.clone(), None)
        .with_htmlcut_failure(htmlcut_failure);
    match reason.as_str() {
        "INVALID_SELECTOR" | "invalid_selector" => MeasurementAcquisitionFailure::Permanent {
            code: PermanentErrorCode::HtmlcutInvalidSelector,
            detail,
        },
        "INVALID_SLICE_PATTERN" | "invalid_slice_pattern" => {
            MeasurementAcquisitionFailure::Permanent {
                code: PermanentErrorCode::HtmlcutInvalidSlicePattern,
                detail,
            }
        }
        "UNSUPPORTED_VALUE_TYPE" | "unsupported_value_type" => {
            MeasurementAcquisitionFailure::Permanent {
                code: PermanentErrorCode::HtmlcutUnsupportedValueType,
                detail,
            }
        }
        "plan_invalid" => MeasurementAcquisitionFailure::Permanent {
            code: PermanentErrorCode::HtmlcutPlanInvalid,
            detail,
        },
        "NO_MATCH" | "no_match" => {
            MeasurementAcquisitionFailure::SourceSuspect(AcquisitionFailure {
                reason: SourceSuspectReason::HtmlcutNoMatch,
                detail,
            })
        }
        "AMBIGUOUS_MATCH" | "ambiguous_match" => {
            MeasurementAcquisitionFailure::SourceSuspect(AcquisitionFailure {
                reason: SourceSuspectReason::HtmlcutAmbiguousMatch,
                detail,
            })
        }
        "MISSING_ATTRIBUTE" | "missing_attribute" => {
            MeasurementAcquisitionFailure::SourceSuspect(AcquisitionFailure {
                reason: SourceSuspectReason::HtmlcutMissingAttribute,
                detail,
            })
        }
        "MATCH_INDEX_OUT_OF_RANGE" | "match_index_out_of_range" => {
            MeasurementAcquisitionFailure::SourceSuspect(AcquisitionFailure {
                reason: SourceSuspectReason::HtmlcutMatchIndexOutOfRange,
                detail,
            })
        }
        _ => MeasurementAcquisitionFailure::SourceSuspect(AcquisitionFailure {
            reason: SourceSuspectReason::HtmlcutInternalFailure,
            detail,
        }),
    }
}

fn html_source_suspect_failure(
    reason: SourceSuspectReason,
    message: impl Into<String>,
    plan_digest_sha256: String,
    candidate_count: Option<usize>,
    diagnostics: Vec<HtmlcutDiagnostic>,
) -> MeasurementAcquisitionFailure {
    let reason_text = match reason {
        SourceSuspectReason::HtmlcutNoMatch => "NO_MATCH",
        SourceSuspectReason::HtmlcutAmbiguousMatch => "AMBIGUOUS_MATCH",
        SourceSuspectReason::HtmlcutMissingAttribute => "MISSING_ATTRIBUTE",
        SourceSuspectReason::HtmlcutMatchIndexOutOfRange => "MATCH_INDEX_OUT_OF_RANGE",
        SourceSuspectReason::HtmlcutInternalFailure => "INTERNAL_ERROR",
        _ => reason.as_str(),
    }
    .to_owned();
    MeasurementAcquisitionFailure::SourceSuspect(AcquisitionFailure {
        reason,
        detail: ProcessErrorDetail::new(ProcessErrorKind::Htmlcut, message, None)
            .with_htmlcut_failure(HtmlcutFailureDetails::new(
                reason_text,
                candidate_count,
                plan_digest_sha256,
                diagnostics,
            )),
    })
}

pub(super) fn fetch_source(target: &TargetDocument) -> Result<FetchedSource, ProcessErrorDetail> {
    match (target.source(), target.fetch()) {
        (crate::TargetSource::File { file_path }, FetchConfig::File { max_bytes }) => {
            read_file_source(file_path, *max_bytes).map(|body| FetchedSource {
                body,
                effective_http_url: None,
            })
        }
        (
            crate::TargetSource::Http { source_url },
            FetchConfig::Http {
                timeout_ms,
                max_bytes,
                user_agent,
                follow_redirects,
                accept,
                headers,
                ..
            },
        ) => fetch_http_response(
            source_url,
            *timeout_ms,
            *max_bytes,
            user_agent,
            *follow_redirects,
            accept,
            headers,
        ),
        _ => Err(ProcessErrorDetail::new(
            ProcessErrorKind::Contract,
            "target source and fetch.engine must agree",
            None,
        )),
    }
}

pub(super) fn read_file_source(
    file_path: &str,
    max_bytes: usize,
) -> Result<String, ProcessErrorDetail> {
    let mut file = File::open(file_path).map_err(|error| {
        ProcessErrorDetail::new(
            ProcessErrorKind::Io,
            format!("could not read configured file source: {error}"),
            Some(file_path.to_owned()),
        )
    })?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            ProcessErrorDetail::new(
                ProcessErrorKind::Io,
                format!("could not read configured file source: {error}"),
                Some(file_path.to_owned()),
            )
        })?;
    if bytes.len() > max_bytes {
        return Err(ProcessErrorDetail::new(
            ProcessErrorKind::Io,
            format!("file source exceeded fetch.max_bytes ({max_bytes})"),
            Some(file_path.to_owned()),
        ));
    }
    String::from_utf8(bytes).map_err(|error| {
        ProcessErrorDetail::new(
            ProcessErrorKind::Io,
            format!("configured file source is not valid UTF-8: {error}"),
            Some(file_path.to_owned()),
        )
    })
}

#[cfg(test)]
pub(super) fn fetch_http_source(
    url: &url::Url,
    timeout_ms: u64,
    max_bytes: usize,
    user_agent: &str,
    follow_redirects: bool,
    accept: &str,
    headers: &std::collections::BTreeMap<String, String>,
) -> Result<String, ProcessErrorDetail> {
    fetch_http_response(
        url,
        timeout_ms,
        max_bytes,
        user_agent,
        follow_redirects,
        accept,
        headers,
    )
    .map(|source| source.body)
}

pub(super) fn fetch_http_response(
    url: &url::Url,
    timeout_ms: u64,
    max_bytes: usize,
    user_agent: &str,
    follow_redirects: bool,
    accept: &str,
    headers: &std::collections::BTreeMap<String, String>,
) -> Result<FetchedSource, ProcessErrorDetail> {
    let tls_config = TlsConfig::builder()
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls_config)
        .timeout_global(Some(Duration::from_millis(timeout_ms)))
        .max_redirects(if follow_redirects { 10 } else { 0 })
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .build()
        .into();
    let mut request = agent
        .get(url.as_str())
        .header("Accept", accept)
        .header("User-Agent", user_agent);
    for (name, value) in headers {
        request = request.header(name, value);
    }
    let mut response = request.call().map_err(|error| {
        ProcessErrorDetail::new(
            ProcessErrorKind::Io,
            format!("HTTP fetch failed: {error}"),
            Some(url.to_string()),
        )
    })?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(ProcessErrorDetail::new(
            ProcessErrorKind::Io,
            format!("HTTP request returned status {status}"),
            Some(response.get_uri().to_string()),
        ));
    }
    if let Some(length) = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        && length > max_bytes
    {
        return Err(ProcessErrorDetail::new(
            ProcessErrorKind::Io,
            format!("HTTP response exceeded fetch.max_bytes ({max_bytes})"),
            Some(response.get_uri().to_string()),
        ));
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            ProcessErrorDetail::new(
                ProcessErrorKind::Io,
                format!("could not read HTTP response: {error}"),
                Some(response.get_uri().to_string()),
            )
        })?;
    if bytes.len() > max_bytes {
        return Err(ProcessErrorDetail::new(
            ProcessErrorKind::Io,
            format!("HTTP response exceeded fetch.max_bytes ({max_bytes})"),
            Some(response.get_uri().to_string()),
        ));
    }
    let effective_http_url = parse_effective_http_url(response.get_uri().to_string().as_str())?;
    String::from_utf8(bytes)
        .map(|body| FetchedSource {
            body,
            effective_http_url: Some(effective_http_url),
        })
        .map_err(|error| {
            ProcessErrorDetail::new(
                ProcessErrorKind::Io,
                format!("HTTP response is not valid UTF-8: {error}"),
                Some(response.get_uri().to_string()),
            )
        })
}

fn parse_effective_http_url(value: &str) -> Result<url::Url, ProcessErrorDetail> {
    url::Url::parse(value).map_err(|error| {
        ProcessErrorDetail::new(
            ProcessErrorKind::Io,
            format!("HTTP response supplied an invalid effective URL: {error}"),
            Some(value.to_owned()),
        )
    })
}

pub(super) fn now_utc() -> Result<String, crate::CoreError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(crate::CoreError::from)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use htmlcut_core::interop::v1::{
        InteropDiagnostic, InteropDiagnosticCode, InteropDiagnosticLevel,
    };
    use serde_json::json;

    use super::*;

    const PLAN_DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn css_selected_match() -> SelectedMatch {
        let candidate_index = NonZeroUsize::new(1).expect("nonzero candidate index");
        SelectedMatch {
            candidate_index,
            output_value: json!({"kind": "structured"}),
            text_output: "selected text".to_owned(),
            comparison_text_output: None,
            selected_html_output: None,
            inner_html_output: "selected text".to_owned(),
            outer_html_output: "<article>selected text</article>".to_owned(),
            metadata: SelectedMatchMetadata::CssSelector {
                candidate_count: 1,
                candidate_index,
                path: "html > body > article".to_owned(),
                tag_name: "article".to_owned(),
                attributes: BTreeMap::new(),
            },
        }
    }

    fn interop_error(error_code: ErrorCode, diagnostic_code: Option<&str>) -> InteropError {
        let mut details = BTreeMap::new();
        if let Some(diagnostic_code) = diagnostic_code {
            details.insert("core_diagnostic_code".to_owned(), json!(diagnostic_code));
        }
        details.insert("context".to_owned(), json!({"candidateCount": 3}));
        InteropError::new(
            PLAN_DIGEST,
            error_code,
            "HTMLCut diagnostic",
            None,
            details,
            vec![InteropDiagnostic {
                level: InteropDiagnosticLevel::Warning,
                code: InteropDiagnosticCode::MultipleMatches,
                message: "warning retained as FFHN evidence".to_owned(),
                details: None,
            }],
        )
    }

    fn source_suspect_reason(
        failure: MeasurementAcquisitionFailure,
    ) -> Option<SourceSuspectReason> {
        match failure {
            MeasurementAcquisitionFailure::SourceSuspect(failure) => Some(failure.reason),
            MeasurementAcquisitionFailure::Permanent { .. } => None,
        }
    }

    fn source_suspect_htmlcut_evidence(
        failure: MeasurementAcquisitionFailure,
    ) -> Option<HtmlcutFailureDetails> {
        match failure {
            MeasurementAcquisitionFailure::SourceSuspect(failure) => {
                failure.detail.htmlcut_failure().cloned()
            }
            MeasurementAcquisitionFailure::Permanent { .. } => None,
        }
    }

    fn permanent_error_code(failure: MeasurementAcquisitionFailure) -> Option<PermanentErrorCode> {
        match failure {
            MeasurementAcquisitionFailure::Permanent { code, .. } => Some(code),
            MeasurementAcquisitionFailure::SourceSuspect(_) => None,
        }
    }

    fn html_text_target(selector: &str, dom_canonicalization: &str) -> TargetDocument {
        let document: TargetDocument = toml::from_str(&format!(
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"html\"\ndisplay_name = \"HTML\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = \"/tmp/source.html\"\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"html_text\"\n[projection.selection.strategy]\nkind = \"css_selector\"\nselector = {selector:?}\n[projection.selection.selection]\nmode = \"single\"\n[projection.selection.rendering]\nwhitespace = \"rendered\"\nrewrite_urls = false\n{dom_canonicalization}\n"
        ))
        .expect("HTML text target");
        document.validate().expect("valid HTML text target");
        document
    }

    fn html_observation(measurement: AcquiredMeasurement) -> HtmlObservationInput {
        match measurement {
            AcquiredMeasurement::Html(observation) => observation,
            AcquiredMeasurement::JsonScalar(_) => {
                panic!("HTML fixture target must acquire an HTML observation")
            }
        }
    }

    #[test]
    #[should_panic(expected = "HTML fixture target must acquire an HTML observation")]
    fn html_observation_fixture_helper_rejects_a_json_measurement() {
        let _ = html_observation(AcquiredMeasurement::JsonScalar("1.00".to_owned()));
    }

    #[test]
    fn htmlcut_boundary_guards_classify_each_published_diagnostic_and_retain_evidence() {
        for (error_code, diagnostic_code, expected) in [
            (
                ErrorCode::NoMatch,
                Some("NO_MATCH"),
                SourceSuspectReason::HtmlcutNoMatch,
            ),
            (
                ErrorCode::AmbiguousMatch,
                Some("AMBIGUOUS_MATCH"),
                SourceSuspectReason::HtmlcutAmbiguousMatch,
            ),
            (
                ErrorCode::MissingAttribute,
                Some("MISSING_ATTRIBUTE"),
                SourceSuspectReason::HtmlcutMissingAttribute,
            ),
            (
                ErrorCode::NoMatch,
                Some("MATCH_INDEX_OUT_OF_RANGE"),
                SourceSuspectReason::HtmlcutMatchIndexOutOfRange,
            ),
            (
                ErrorCode::InternalError,
                Some("unrecognized_htmlcut_diagnostic"),
                SourceSuspectReason::HtmlcutInternalFailure,
            ),
        ] {
            assert_eq!(
                source_suspect_reason(classify_htmlcut_error(interop_error(
                    error_code,
                    diagnostic_code,
                ))),
                Some(expected)
            );
        }

        for (diagnostic_code, expected) in [
            (
                "INVALID_SELECTOR",
                PermanentErrorCode::HtmlcutInvalidSelector,
            ),
            (
                "INVALID_SLICE_PATTERN",
                PermanentErrorCode::HtmlcutInvalidSlicePattern,
            ),
            (
                "UNSUPPORTED_VALUE_TYPE",
                PermanentErrorCode::HtmlcutUnsupportedValueType,
            ),
        ] {
            assert_eq!(
                permanent_error_code(classify_htmlcut_error(interop_error(
                    ErrorCode::PlanInvalid,
                    Some(diagnostic_code),
                ))),
                Some(expected)
            );
        }
        assert_eq!(
            permanent_error_code(classify_htmlcut_error(interop_error(
                ErrorCode::PlanInvalid,
                None,
            ))),
            Some(PermanentErrorCode::HtmlcutPlanInvalid)
        );
        assert_eq!(
            source_suspect_reason(classify_htmlcut_error(interop_error(
                ErrorCode::PlanInvalid,
                None,
            ))),
            None
        );
        assert_eq!(
            permanent_error_code(classify_htmlcut_error(interop_error(
                ErrorCode::NoMatch,
                Some("NO_MATCH"),
            ))),
            None
        );

        assert!(
            source_suspect_htmlcut_evidence(classify_htmlcut_error(interop_error(
                ErrorCode::PlanInvalid,
                None
            ),))
            .is_none()
        );
        let nested_evidence = source_suspect_htmlcut_evidence(classify_htmlcut_error(
            interop_error(ErrorCode::NoMatch, Some("NO_MATCH")),
        ))
        .expect("a no-match failure retains HTMLCut evidence");
        assert_eq!(nested_evidence.candidate_count(), Some(3));

        for reason in [
            SourceSuspectReason::HtmlcutNoMatch,
            SourceSuspectReason::HtmlcutAmbiguousMatch,
            SourceSuspectReason::HtmlcutMissingAttribute,
            SourceSuspectReason::HtmlcutMatchIndexOutOfRange,
            SourceSuspectReason::HtmlcutInternalFailure,
        ] {
            assert_eq!(
                source_suspect_reason(html_source_suspect_failure(
                    reason,
                    "boundary failure",
                    PLAN_DIGEST.to_owned(),
                    Some(1),
                    Vec::new(),
                )),
                Some(reason)
            );
        }
    }

    #[test]
    fn impossible_success_shapes_are_rejected_at_the_ffhn_htmlcut_boundary() {
        let selected = css_selected_match();
        assert_eq!(
            required_selected_match(vec![selected.clone()], PLAN_DIGEST, 1, &[])
                .expect("one selected match is required")
                .text_output,
            "selected text"
        );
        assert_eq!(
            source_suspect_reason(
                required_selected_match(Vec::new(), PLAN_DIGEST, 1, &[])
                    .expect_err("successful exact-one HTMLCut result must contain one match"),
            ),
            Some(SourceSuspectReason::HtmlcutInternalFailure)
        );
        assert_eq!(
            source_suspect_reason(
                required_selected_match(vec![selected.clone(), selected], PLAN_DIGEST, 1, &[])
                    .expect_err("successful exact-one HTMLCut result must not contain two matches"),
            ),
            Some(SourceSuspectReason::HtmlcutInternalFailure)
        );

        let delimiter = SelectedMatchMetadata::DelimiterPair {
            candidate_count: 1,
            candidate_index: NonZeroUsize::new(1).expect("nonzero"),
            selected_range: htmlcut_core::interop::v1::ByteRange { start: 0, end: 1 },
            inner_range: htmlcut_core::interop::v1::ByteRange { start: 0, end: 1 },
            outer_range: htmlcut_core::interop::v1::ByteRange { start: 0, end: 1 },
            include_start: false,
            include_end: false,
            matched_start: "<start>".to_owned(),
            matched_end: "<end>".to_owned(),
        };
        assert_eq!(
            source_suspect_reason(
                css_attributes_or_failure(delimiter, PLAN_DIGEST, 1, &[])
                    .expect_err("html_attribute requires CSS metadata"),
            ),
            Some(SourceSuspectReason::HtmlcutInternalFailure)
        );
        assert_eq!(
            source_suspect_reason(
                required_html_attribute(&BTreeMap::new(), "content", PLAN_DIGEST, 1, &[])
                    .expect_err("missing public CSS metadata attribute"),
            ),
            Some(SourceSuspectReason::HtmlcutMissingAttribute)
        );
    }

    #[test]
    fn dom_canonicalization_keeps_raw_html_evidence_and_candidate_identity() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            name: String,
            selector: String,
            source: String,
            ignore_attributes: Vec<String>,
            strip_whitespace_nodes: bool,
            raw_selected: String,
            comparison_projection: String,
        }

        let fixtures: Vec<Fixture> = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/html-dom-canonicalization.json"
        )))
        .expect("DOM canonicalization fixtures");

        for fixture in fixtures {
            let ignore_attributes = toml::Value::Array(
                fixture
                    .ignore_attributes
                    .iter()
                    .cloned()
                    .map(toml::Value::String)
                    .collect(),
            )
            .to_string();
            let dom_canonicalization = format!(
                "[projection.selection.dom_canonicalization]\nignore_attributes = {ignore_attributes}\nstrip_whitespace_nodes = {}",
                fixture.strip_whitespace_nodes
            );
            let plain = acquire_measurement(
                &html_text_target(&fixture.selector, ""),
                &fixture.source,
                None,
            )
            .expect("plain HTML acquisition");
            let canonicalized = acquire_measurement(
                &html_text_target(&fixture.selector, &dom_canonicalization),
                &fixture.source,
                None,
            )
            .expect("canonicalized HTML acquisition");

            let plain = html_observation(plain);
            let canonicalized = html_observation(canonicalized);
            assert_eq!(plain.raw_selected, fixture.raw_selected, "{}", fixture.name);
            assert_eq!(
                plain.comparison_projection, plain.raw_selected,
                "{}",
                fixture.name
            );
            assert_eq!(
                canonicalized.raw_selected, plain.raw_selected,
                "{}",
                fixture.name
            );
            assert_eq!(
                canonicalized.comparison_projection, fixture.comparison_projection,
                "{}",
                fixture.name
            );
            assert_eq!(
                canonicalized.candidate_count, plain.candidate_count,
                "{}",
                fixture.name
            );
            assert_eq!(canonicalized.candidate_count, 1, "{}", fixture.name);
            assert_eq!(
                canonicalized.diagnostics, plain.diagnostics,
                "{}",
                fixture.name
            );
            assert_ne!(
                canonicalized.plan_digest_sha256, plain.plan_digest_sha256,
                "the HTMLCut plan identity binds canonicalization for {}",
                fixture.name
            );
        }
    }

    #[test]
    fn html_text_comparison_projection_rejects_impossible_htmlcut_result_shapes() {
        assert_eq!(
            html_text_comparison_projection(false, None, "raw", PLAN_DIGEST, 1, &[])
                .expect("raw text is the comparison projection without canonicalization"),
            "raw"
        );
        assert_eq!(
            html_text_comparison_projection(
                true,
                Some("clone".to_owned()),
                "raw",
                PLAN_DIGEST,
                1,
                &[]
            )
            .expect("clone text is the comparison projection with canonicalization"),
            "clone"
        );
        for (requested, comparison_text_output) in [(true, None), (false, Some("clone".to_owned()))]
        {
            assert_eq!(
                source_suspect_reason(
                    html_text_comparison_projection(
                        requested,
                        comparison_text_output,
                        "raw",
                        PLAN_DIGEST,
                        1,
                        &[],
                    )
                    .expect_err("HTMLCut must agree with the requested canonicalization mode"),
                ),
                Some(SourceSuspectReason::HtmlcutInternalFailure)
            );
        }
    }

    #[test]
    fn acquisition_boundary_rejects_impossible_inputs_without_reclassifying_them_as_source_health()
    {
        assert!(matches!(
            htmlcut_input_from_label("", "<p>body</p>"),
            Err(MeasurementAcquisitionFailure::Permanent {
                code: PermanentErrorCode::HtmlcutInputInvalid,
                ..
            })
        ));
        assert!(matches!(
            htmlcut_http_base_url(url::Url::parse("mailto:ops@example.test").expect("URL")),
            Err(MeasurementAcquisitionFailure::Permanent {
                code: PermanentErrorCode::HtmlcutInputInvalid,
                ..
            })
        ));
        assert!(parse_effective_http_url("not an HTTP URL").is_err());
        assert_eq!(
            source_suspect_reason(html_source_suspect_failure(
                SourceSuspectReason::FetchFailed,
                "unexpected non-HTMLCut source failure",
                PLAN_DIGEST.to_owned(),
                None,
                Vec::new(),
            )),
            Some(SourceSuspectReason::FetchFailed)
        );

        let html_target: TargetDocument = toml::from_str(
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"html\"\ndisplay_name = \"HTML\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = \"/tmp/source.html\"\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"html_text\"\n[projection.selection.strategy]\nkind = \"css_selector\"\nselector = \"article\"\n[projection.selection.selection]\nmode = \"single\"\n[projection.selection.rendering]\nwhitespace = \"rendered\"\nrewrite_urls = false\n",
        )
        .expect("HTML target");
        assert_eq!(
            acquire_json_scalar(&html_target, "<article>1</article>")
                .expect_err("JSON acquisition rejects an HTML projection")
                .reason,
            SourceSuspectReason::HtmlcutInternalFailure
        );
    }
}

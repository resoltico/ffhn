use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use htmlcut_core::interop::v1::{
    ErrorCode, InteropDiagnostic, InteropDiagnosticCode, InteropDiagnosticLevel, InteropError,
    SelectedMatch, SelectedMatchMetadata,
};
use serde_json::json;

use super::*;
use crate::{
    HtmlcutBoundaryEvidence, HtmlcutDiagnosticCode, HtmlcutFailureDetails, IntegrationFaultCode,
};

const PLAN_DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn css_selected_match() -> SelectedMatch {
    let candidate_index = NonZeroUsize::new(1).expect("nonzero candidate index");
    SelectedMatch {
        candidate_index,
        output_value: json!({"kind": "structured"}),
        text_output: "selected text".to_owned(),
        comparison_text_output: None,
        plain_text_output: Some("selected text".to_owned()),
        comparison_plain_text_output: None,
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
    details.insert("core_details".to_owned(), json!({"candidateCount": 3}));
    let diagnostics = match diagnostic_code {
        Some("INVALID_SELECTOR") => vec![InteropDiagnostic {
            level: InteropDiagnosticLevel::Error,
            code: InteropDiagnosticCode::InvalidSelector,
            message: "CSS selector is invalid.".to_owned(),
            details: Some(json!({
                "selector_parse": {
                    "line": 1,
                    "column_utf16": 1,
                    "parse_error_class": "invalid_attribute_selector",
                }
            })),
        }],
        _ => Vec::new(),
    };
    if diagnostic_code == Some("INVALID_SELECTOR") {
        details.insert(
            "core_details".to_owned(),
            json!({
                "candidateCount": 3,
                "selector_parse": {
                    "line": 1,
                    "column_utf16": 1,
                    "parse_error_class": "invalid_attribute_selector",
                }
            }),
        );
    }
    InteropError::new(
        PLAN_DIGEST,
        error_code,
        "HTMLCut diagnostic",
        None,
        details,
        diagnostics,
    )
}

fn source_suspect_reason(failure: MeasurementAcquisitionFailure) -> Option<SourceSuspectReason> {
    match failure {
        MeasurementAcquisitionFailure::SourceSuspect(failure) => Some(failure.reason),
        MeasurementAcquisitionFailure::Permanent { .. }
        | MeasurementAcquisitionFailure::Integration { .. } => None,
    }
}

fn source_suspect_htmlcut_evidence(
    failure: MeasurementAcquisitionFailure,
) -> Option<HtmlcutFailureDetails> {
    match failure {
        MeasurementAcquisitionFailure::SourceSuspect(failure) => {
            failure.detail.htmlcut_failure().cloned()
        }
        MeasurementAcquisitionFailure::Permanent { .. }
        | MeasurementAcquisitionFailure::Integration { .. } => None,
    }
}

fn permanent_error_code(failure: MeasurementAcquisitionFailure) -> Option<PermanentErrorCode> {
    match failure {
        MeasurementAcquisitionFailure::Permanent { code, .. } => Some(code),
        MeasurementAcquisitionFailure::SourceSuspect(_)
        | MeasurementAcquisitionFailure::Integration { .. } => None,
    }
}

fn integration_fault_code(failure: MeasurementAcquisitionFailure) -> Option<IntegrationFaultCode> {
    match failure {
        MeasurementAcquisitionFailure::Integration { detail } => detail.integration_fault_code(),
        MeasurementAcquisitionFailure::SourceSuspect(_)
        | MeasurementAcquisitionFailure::Permanent { .. } => None,
    }
}

fn html_rendered_text_target(selector: &str, dom_canonicalization: &str) -> TargetDocument {
    let source_path = crate::test_support::absolute_file_path("source.html");
    let document: TargetDocument = toml::from_str(&format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"html\"\ndisplay_name = \"HTML\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"html_rendered_text\"\n[projection.selection.strategy]\nkind = \"css_selector\"\nselector = {selector:?}\n[projection.selection.selection]\nmode = \"single\"\n[projection.selection.rendering]\nwhitespace = \"rendered\"\nrewrite_urls = false\n{dom_canonicalization}\n"
        ))
        .expect("HTML rendered-text target");
    document
        .validate()
        .expect("valid HTML rendered-text target");
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
fn htmlcut_error_codes_classify_categories_and_diagnostics_only_refine_within_them() {
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
        integration_fault_code(classify_htmlcut_error(interop_error(
            ErrorCode::InternalError,
            Some("NO_MATCH"),
        ))),
        Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation)
    );
    for error_code in [ErrorCode::PlanInvalid, ErrorCode::NoMatch] {
        assert_eq!(
            integration_fault_code(classify_htmlcut_error(interop_error(
                error_code,
                Some("UNFAMILIAR_DIAGNOSTIC"),
            ))),
            Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation)
        );
    }
    assert_eq!(
        integration_fault_code(classify_htmlcut_error(interop_error(
            ErrorCode::NoMatch,
            Some("INVALID_SELECTOR"),
        ))),
        Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation)
    );
    assert_eq!(
        integration_fault_code(classify_htmlcut_error(interop_error(
            ErrorCode::PlanInvalid,
            Some("UNFAMILIAR_DIAGNOSTIC"),
        ))),
        Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation)
    );
    for error_code in [ErrorCode::AmbiguousMatch, ErrorCode::MissingAttribute] {
        assert_eq!(
            integration_fault_code(classify_htmlcut_error(interop_error(
                error_code,
                Some("UNFAMILIAR_DIAGNOSTIC"),
            ))),
            Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation)
        );
    }
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
    let nested_evidence = source_suspect_htmlcut_evidence(classify_htmlcut_error(interop_error(
        ErrorCode::NoMatch,
        Some("NO_MATCH"),
    )))
    .expect("a no-match failure retains HTMLCut evidence");
    assert_eq!(nested_evidence.candidate_count(), Some(3));

    assert_eq!(
        integration_fault_code(classify_htmlcut_error(interop_error(
            ErrorCode::InternalError,
            None,
        ))),
        Some(IntegrationFaultCode::HtmlcutInternalError)
    );

    assert_eq!(
        source_suspect_reason_for_no_match_diagnostic(None),
        SourceSuspectReason::HtmlcutNoMatch
    );
    assert_eq!(
        source_suspect_reason_for_no_match_diagnostic(Some(HtmlcutDiagnosticCode::NoMatch)),
        SourceSuspectReason::HtmlcutNoMatch
    );
    assert_eq!(
        source_suspect_reason_for_no_match_diagnostic(Some(
            HtmlcutDiagnosticCode::MatchIndexOutOfRange
        )),
        SourceSuspectReason::HtmlcutMatchIndexOutOfRange
    );
}

#[test]
fn unmodeled_htmlcut_evidence_becomes_a_typed_boundary_integration_fault() {
    let malformed = InteropDiagnostic {
        level: InteropDiagnosticLevel::Error,
        code: InteropDiagnosticCode::InvalidSelector,
        message: "selector parse failed".to_owned(),
        details: Some(json!({
            "selector_parse": {
                "line": 1,
                "column_utf16": 1,
                "parse_error_class": "future_parser_class",
            }
        })),
    };
    let failure = retain_htmlcut_diagnostics(vec![malformed.clone()], PLAN_DIGEST, 3)
        .expect_err("unmodeled successful diagnostic evidence must fail closed");
    assert_eq!(
        integration_fault_code(failure),
        Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation)
    );

    let mut error = interop_error(ErrorCode::NoMatch, Some("NO_MATCH"));
    error.diagnostics.push(malformed);
    assert_eq!(
        integration_fault_code(classify_htmlcut_error(error)),
        Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation)
    );
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
    for (selected_matches, expected_count) in
        [(Vec::new(), 0), (vec![selected.clone(), selected], 2)]
    {
        let failure = required_selected_match(selected_matches, PLAN_DIGEST, 1, &[]).expect_err(
            "successful exact-one HTMLCut result must retain its contradictory cardinality",
        );
        let (fault_code, evidence) = match failure {
            MeasurementAcquisitionFailure::Integration { detail } => (
                detail.integration_fault_code(),
                detail
                    .htmlcut_failure()
                    .and_then(HtmlcutFailureDetails::boundary_evidence)
                    .cloned(),
            ),
            MeasurementAcquisitionFailure::SourceSuspect(_)
            | MeasurementAcquisitionFailure::Permanent { .. } => {
                panic!("exact-one postcondition violations must be integration faults")
            }
        };
        assert_eq!(
            fault_code,
            Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation)
        );
        assert_eq!(
            evidence,
            Some(HtmlcutBoundaryEvidence::SelectedMatchCount {
                selected_match_count: expected_count,
            })
        );
    }

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
        integration_fault_code(
            css_attributes_or_failure(delimiter, PLAN_DIGEST, 1, &[])
                .expect_err("html_attribute requires CSS metadata"),
        ),
        Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation)
    );
    let missing_attribute =
        required_html_attribute(&BTreeMap::new(), "content", PLAN_DIGEST, 1, &[])
            .expect_err("missing public CSS metadata attribute");
    let (reason, evidence) = match missing_attribute {
        MeasurementAcquisitionFailure::SourceSuspect(failure) => (
            failure.reason,
            failure
                .detail
                .htmlcut_failure()
                .and_then(HtmlcutFailureDetails::boundary_evidence)
                .cloned(),
        ),
        MeasurementAcquisitionFailure::Permanent { .. }
        | MeasurementAcquisitionFailure::Integration { .. } => {
            panic!("missing required CSS metadata must be source-suspect evidence")
        }
    };
    assert_eq!(
        Some(reason),
        Some(SourceSuspectReason::HtmlcutMissingAttribute)
    );
    assert_eq!(
        evidence,
        Some(HtmlcutBoundaryEvidence::RequestedCssAttribute {
            attribute: "content".to_owned(),
        })
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
            &html_rendered_text_target(&fixture.selector, ""),
            &fixture.source,
            None,
        )
        .expect("plain HTML acquisition");
        let canonicalized = acquire_measurement(
            &html_rendered_text_target(&fixture.selector, &dom_canonicalization),
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
        html_text_comparison_projection(true, Some("clone".to_owned()), "raw", PLAN_DIGEST, 1, &[])
            .expect("clone text is the comparison projection with canonicalization"),
        "clone"
    );
    for (requested, comparison_text_output) in [(true, None), (false, Some("clone".to_owned()))] {
        assert_eq!(
            integration_fault_code(
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
            Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation)
        );
    }
}

#[test]
fn plain_text_projection_requires_htmlcut_plain_text_evidence() {
    assert_eq!(
        required_plain_text_output(Some("bare heading".to_owned()), PLAN_DIGEST, 1, &[])
            .expect("present plain text must cross the boundary unchanged"),
        "bare heading"
    );
    assert_eq!(
        integration_fault_code(
            required_plain_text_output(None, PLAN_DIGEST, 1, &[])
                .expect_err("a CSS selection without plain text is an upstream invariant breach"),
        ),
        Some(IntegrationFaultCode::FfhnBoundaryInvariantViolation)
    );
}

#[test]
fn acquisition_boundary_rejects_impossible_inputs_without_reclassifying_them_as_source_health() {
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
        acquire_json_scalar("/value", r#"{"value":"text"}"#)
            .expect("JSON pointer is the complete acquisition input"),
        r#""text""#
    );
}

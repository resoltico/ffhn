use super::*;

const SOURCE: &str = r#"
schema_name = "ffhn.source"
schema_version = 1
source_id = "shop"
display_name = "Shop"
enabled = true
escalate_after = 2
[fetch]
engine = "file"
file_path = "/tmp/shop.json"
max_bytes = 1024
[conditional]
enabled = false
[schedule]
interval_ms = 1000
min_interval_ms = 1000
"#;

const MEASUREMENT: &str = r#"
schema_name = "ffhn.measurement"
schema_version = 1
measurement_id = "price"
display_name = "Price"
enabled = true
escalate_after = 2
declared_type = "integer"
conditions = []
[projection]
kind = "json_pointer"
pointer = "/price"
"#;

fn source() -> SourceDocument {
    toml::from_str(&SOURCE.replace(
        "/tmp/shop.json",
        &crate::graph::test_support::absolute_file_path("shop.json").replace('\\', "\\\\"),
    ))
    .expect("source")
}

#[test]
fn prepared_json_projection_owns_typed_extraction_directly() {
    let source = source();
    let measurement: MeasurementDocument = toml::from_str(MEASUREMENT).expect("measurement");
    let prepared = PreparedMeasurementProjection::prepare(&measurement).expect("prepared");
    let document = SourceDocumentBytes {
        body: r#"{"price":7}"#.to_owned(),
        effective_http_url: None,
        file_content_sha256: Some("a".repeat(64)),
        validators: None,
    };
    assert_eq!(
        prepared
            .execute(&measurement, &source, &document)
            .expect("observation")
            .canonical_value(),
        "7"
    );
    let malformed = SourceDocumentBytes {
        body: "{".to_owned(),
        ..document
    };
    assert_eq!(
        prepared.execute(&measurement, &source, &malformed),
        Err(MeasurementProjectionFailure::Extraction(
            ExtractionFailureReason::JsonMalformed
        ))
    );
}

#[test]
fn prepared_html_projection_executes_the_pinned_htmlcut_plan_against_in_memory_input() {
    let source = source();
    let measurement: MeasurementDocument = toml::from_str(
        r#"
schema_name = "ffhn.measurement"
schema_version = 1
measurement_id = "heading"
display_name = "Heading"
enabled = true
escalate_after = 2
declared_type = "text"
conditions = []
[projection]
kind = "html_text"
[projection.selection.strategy]
kind = "css_selector"
selector = "h1"
[projection.selection.selection]
mode = "single"
[projection.selection.rendering]
whitespace = "rendered"
rewrite_urls = false
"#,
    )
    .expect("measurement");
    let Projection::HtmlText { selection } = measurement.projection() else {
        panic!("HTML text projection");
    };
    assert!(matches!(
        selection.strategy(),
        PlanStrategy::CssSelector { .. }
    ));
    assert!(matches!(selection.selection(), Selection::Single));
    let _ = selection.rendering();
    let prepared = PreparedMeasurementProjection::prepare(&measurement).expect("prepared plan");
    let document = SourceDocumentBytes {
        body: "<main><h1>Example Heading</h1></main>".to_owned(),
        effective_http_url: None,
        file_content_sha256: Some("a".repeat(64)),
        validators: None,
    };
    assert_eq!(
        prepared
            .execute(&measurement, &source, &document)
            .expect("observation")
            .canonical_value(),
        "Example Heading"
    );
}

#[test]
fn dom_text_and_attribute_projections_reject_delimiter_strategies_at_preflight() {
    for projection in ["html_text", "html_attribute"] {
        let name = if projection == "html_attribute" {
            "name = \"content\"\n"
        } else {
            ""
        };
        let measurement: MeasurementDocument = toml::from_str(&format!(
            "schema_name = \"ffhn.measurement\"\nschema_version = 1\nmeasurement_id = \"value\"\ndisplay_name = \"Value\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"text\"\nconditions = []\n[projection]\nkind = \"{projection}\"\n{name}[projection.selection.strategy]\nkind = \"delimiter_pair\"\nstart = \"<h1>\"\nend = \"</h1>\"\nmode = \"literal\"\nboundary_retention = \"exclude_both\"\n[projection.selection.selection]\nmode = \"single\"\n[projection.selection.rendering]\nwhitespace = \"rendered\"\nrewrite_urls = false\n"
        ))
        .expect("structural measurement");
        assert!(
            PreparedMeasurementProjection::prepare(&measurement).is_err(),
            "{projection}"
        );
    }
}

#[test]
fn html_attribute_rejects_inert_dom_canonicalization() {
    let measurement: MeasurementDocument = toml::from_str(
        r#"
schema_name = "ffhn.measurement"
schema_version = 1
measurement_id = "value"
display_name = "Value"
enabled = true
escalate_after = 2
declared_type = "text"
conditions = []
[projection]
kind = "html_attribute"
name = "content"
[projection.selection.strategy]
kind = "css_selector"
selector = "meta"
[projection.selection.selection]
mode = "single"
[projection.selection.rendering]
whitespace = "rendered"
rewrite_urls = false
[projection.selection.dom_canonicalization]
ignore_attributes = ["class"]
"#,
    )
    .expect("structural measurement");
    assert!(PreparedMeasurementProjection::prepare(&measurement).is_err());
}

#[test]
fn shared_projection_helpers_validate_json_pointer_and_attach_dom_canonicalization() {
    assert!(crate::model::validate_json_pointer("").is_ok());
    assert!(crate::model::validate_json_pointer("/a~0b/~1").is_ok());
    assert!(crate::model::validate_json_pointer("relative").is_err());
    assert!(crate::model::validate_json_pointer("/bad~2escape").is_err());

    let measurement: MeasurementDocument = toml::from_str(
        r#"
schema_name = "ffhn.measurement"
schema_version = 1
measurement_id = "heading"
display_name = "Heading"
enabled = true
escalate_after = 2
declared_type = "text"
conditions = []
[projection]
kind = "html_text"
[projection.selection.strategy]
kind = "css_selector"
selector = "h1"
[projection.selection.selection]
mode = "single"
[projection.selection.rendering]
whitespace = "rendered"
rewrite_urls = false
[projection.selection.dom_canonicalization]
ignore_attributes = ["class"]
"#,
    )
    .expect("measurement");
    let Projection::HtmlText { selection } = measurement.projection() else {
        panic!("HTML text");
    };
    assert!(selection.dom_canonicalization().is_some());
    let _ = selection.structured_plan();
}

fn html_measurement(kind: &str, selection: &str, extra: &str) -> MeasurementDocument {
    toml::from_str(&format!(
        r#"
schema_name = "ffhn.measurement"
schema_version = 1
measurement_id = "value"
display_name = "Value"
enabled = true
escalate_after = 2
declared_type = "text"
conditions = []
[projection]
kind = "{kind}"
{extra}[projection.selection.strategy]
kind = "css_selector"
selector = "a"
[projection.selection.selection]
{selection}
[projection.selection.rendering]
whitespace = "rendered"
rewrite_urls = false
"#,
    ))
    .expect("HTML measurement")
}

fn file_document(body: &str) -> SourceDocumentBytes {
    SourceDocumentBytes {
        body: body.to_owned(),
        effective_http_url: None,
        file_content_sha256: Some("a".repeat(64)),
        validators: None,
    }
}

#[test]
fn projection_failure_matrix_distinguishes_json_html_and_typed_value_outcomes() {
    let source = source();
    let measurement: MeasurementDocument = toml::from_str(MEASUREMENT).expect("measurement");
    let prepared = PreparedMeasurementProjection::prepare(&measurement).expect("prepared");
    for (body, reason) in [
        (
            r#"{"other":1}"#,
            ExtractionFailureReason::JsonMissingPointerTarget,
        ),
        (
            r#"{"price":{}}"#,
            ExtractionFailureReason::JsonNonScalarPointerTarget,
        ),
        (
            r#"{"price":"text"}"#,
            ExtractionFailureReason::ValueUnparseable,
        ),
    ] {
        assert_eq!(
            prepared.execute(&measurement, &source, &file_document(body)),
            Err(MeasurementProjectionFailure::Extraction(reason))
        );
    }

    for (measurement, body, reason) in [
        (
            html_measurement("html_text", "mode = \"single\"", ""),
            "<p>none</p>",
            ExtractionFailureReason::HtmlcutNoMatch,
        ),
        (
            html_measurement("html_text", "mode = \"single\"", ""),
            "<a>one</a><a>two</a>",
            ExtractionFailureReason::HtmlcutAmbiguousMatch,
        ),
        (
            html_measurement("html_text", "mode = \"nth\"\nindex = 2", ""),
            "<a>one</a>",
            ExtractionFailureReason::HtmlcutMatchIndexOutOfRange,
        ),
        (
            html_measurement("html_attribute", "mode = \"single\"", "name = \"href\"\n"),
            "<a>one</a>",
            ExtractionFailureReason::HtmlcutMissingAttribute,
        ),
    ] {
        let prepared = PreparedMeasurementProjection::prepare(&measurement).expect("prepared HTML");
        assert_eq!(
            prepared.execute(&measurement, &source, &file_document(body)),
            Err(MeasurementProjectionFailure::Extraction(reason))
        );
    }
}

#[test]
fn rendered_text_attribute_and_html_input_paths_cover_each_successful_output_contract() {
    let source = source();
    for (measurement, body, expected) in [
        (
            html_measurement("html_rendered_text", "mode = \"first\"", ""),
            "<a><strong>Bold</strong></a>",
            "Bold",
        ),
        (
            html_measurement("html_attribute", "mode = \"single\"", "name = \"href\"\n"),
            "<a href=\"/path\">Link</a>",
            "/path",
        ),
    ] {
        let prepared = PreparedMeasurementProjection::prepare(&measurement).expect("prepared");
        assert_eq!(
            prepared
                .execute(&measurement, &source, &file_document(body))
                .expect("observation")
                .canonical_value(),
            expected
        );
    }

    let all = html_measurement("html_rendered_text", "mode = \"all\"", "");
    assert!(PreparedMeasurementProjection::prepare(&all).is_err());
    let invalid_plan: MeasurementDocument = toml::from_str(
        r#"
schema_name = "ffhn.measurement"
schema_version = 1
measurement_id = "invalid"
display_name = "Invalid"
enabled = true
escalate_after = 2
declared_type = "text"
conditions = []
[projection]
kind = "html_rendered_text"
[projection.selection.strategy]
kind = "delimiter_pair"
start = "<a>"
end = "</a>"
mode = "literal"
boundary_retention = "exclude_both"
flags = ["case_insensitive"]
[projection.selection.selection]
mode = "single"
[projection.selection.rendering]
whitespace = "rendered"
rewrite_urls = false
"#,
    )
    .expect("structural invalid plan");
    assert!(PreparedMeasurementProjection::prepare(&invalid_plan).is_err());

    assert_eq!(comparison_text(false, None, "raw").expect("raw"), "raw");
    assert_eq!(
        comparison_text(true, Some("canonical".to_owned()), "raw").expect("canonical"),
        "canonical"
    );
    assert!(comparison_text(false, Some("unexpected".to_owned()), "raw").is_err());
    assert!(comparison_text(true, None, "raw").is_err());

    html_input(&source, &file_document("<a>value</a>")).expect("file HTML input");
    let http_source: SourceDocument = toml::from_str(
        r#"
schema_name = "ffhn.source"
schema_version = 1
source_id = "http"
display_name = "HTTP"
enabled = true
escalate_after = 1
[fetch]
engine = "http"
source_url = "https://example.test/start"
user_agent = "ffhn-test"
accept = "text/html"
max_bytes = 1024
follow_redirects = true
max_redirects = 5
[fetch.timeouts]
connect_ms = 100
read_idle_ms = 100
total_ms = 100
[conditional]
enabled = true
[schedule]
interval_ms = 1000
min_interval_ms = 1000
"#,
    )
    .expect("HTTP source");
    let http_document = SourceDocumentBytes {
        body: "<a>value</a>".to_owned(),
        effective_http_url: Some(url::Url::parse("https://example.test/final").expect("URL")),
        file_content_sha256: None,
        validators: None,
    };
    html_input(&http_source, &http_document).expect("HTTP HTML input");
}

#[test]
fn htmlcut_error_and_success_boundary_defenses_are_total_and_testable() {
    use htmlcut_core::interop::v1::{
        InteropDiagnostic, InteropDiagnosticCode, InteropDiagnosticLevel,
    };

    for (code, indexed, expected) in [
        (
            ErrorCode::NoMatch,
            false,
            MeasurementProjectionFailure::Extraction(ExtractionFailureReason::HtmlcutNoMatch),
        ),
        (
            ErrorCode::NoMatch,
            true,
            MeasurementProjectionFailure::Extraction(
                ExtractionFailureReason::HtmlcutMatchIndexOutOfRange,
            ),
        ),
        (
            ErrorCode::AmbiguousMatch,
            false,
            MeasurementProjectionFailure::Extraction(
                ExtractionFailureReason::HtmlcutAmbiguousMatch,
            ),
        ),
        (
            ErrorCode::MissingAttribute,
            false,
            MeasurementProjectionFailure::Extraction(
                ExtractionFailureReason::HtmlcutMissingAttribute,
            ),
        ),
        (
            ErrorCode::InternalError,
            false,
            MeasurementProjectionFailure::Integration(
                GraphIntegrationFaultCode::HtmlcutInternalError,
            ),
        ),
        (
            ErrorCode::PlanInvalid,
            false,
            MeasurementProjectionFailure::Integration(
                GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation,
            ),
        ),
    ] {
        assert_eq!(classify_htmlcut_error(code, indexed), expected);
    }

    let source = source();
    let measurement = html_measurement("html_text", "mode = \"single\"", "");
    let prepared = PreparedMeasurementProjection::prepare(&measurement).expect("prepared");
    let PreparedMeasurementProjection::Html(prepared) = prepared else {
        panic!("HTML projection");
    };
    let document = file_document("<a>text</a>");
    let input = html_input(&source, &document).expect("input");
    let valid = execute_validated_plan(&input, &prepared.plan).expect("result");

    let mut no_selection = valid.clone();
    no_selection.selected_matches.clear();
    assert_eq!(
        prepared.interpret_result(&measurement, no_selection),
        Err(MeasurementProjectionFailure::Integration(
            GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation
        ))
    );
    let mut missing_plain = valid.clone();
    missing_plain.selected_matches[0].plain_text_output = None;
    assert_eq!(
        prepared.interpret_result(&measurement, missing_plain),
        Err(MeasurementProjectionFailure::Integration(
            GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation
        ))
    );
    let mut invalid_diagnostic = valid;
    invalid_diagnostic.diagnostics.push(InteropDiagnostic {
        level: InteropDiagnosticLevel::Warning,
        code: InteropDiagnosticCode::MultipleMatches,
        message: "diagnostic".to_owned(),
        details: Some(serde_json::json!({"candidateCount": 1, "selectedIndex": 1})),
    });
    assert_eq!(
        prepared.interpret_result(&measurement, invalid_diagnostic),
        Err(MeasurementProjectionFailure::Integration(
            GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation
        ))
    );

    let attribute = html_measurement("html_attribute", "mode = \"single\"", "name = \"href\"\n");
    let PreparedMeasurementProjection::Html(attribute_prepared) =
        PreparedMeasurementProjection::prepare(&attribute).expect("attribute")
    else {
        panic!("attribute projection");
    };
    let attribute_document = file_document("<a href=\"/path\">text</a>");
    let attribute_input = html_input(&source, &attribute_document).expect("input");
    let mut attribute_result =
        execute_validated_plan(&attribute_input, &attribute_prepared.plan).expect("result");

    let delimiter: MeasurementDocument = toml::from_str(
        r#"
schema_name = "ffhn.measurement"
schema_version = 1
measurement_id = "delimiter"
display_name = "Delimiter"
enabled = true
escalate_after = 2
declared_type = "text"
conditions = []
[projection]
kind = "html_rendered_text"
[projection.selection.strategy]
kind = "delimiter_pair"
start = "<a>"
end = "</a>"
mode = "literal"
boundary_retention = "exclude_both"
[projection.selection.selection]
mode = "single"
[projection.selection.rendering]
whitespace = "rendered"
rewrite_urls = false
"#,
    )
    .expect("delimiter");
    let PreparedMeasurementProjection::Html(delimiter_prepared) =
        PreparedMeasurementProjection::prepare(&delimiter).expect("delimiter")
    else {
        panic!("delimiter projection");
    };
    let delimiter_document = file_document("<a>text</a>");
    let delimiter_input = html_input(&source, &delimiter_document).expect("input");
    let delimiter_result =
        execute_validated_plan(&delimiter_input, &delimiter_prepared.plan).expect("result");
    attribute_result.selected_matches[0].metadata =
        delimiter_result.selected_matches[0].metadata.clone();
    assert_eq!(
        attribute_prepared.interpret_result(&attribute, attribute_result),
        Err(MeasurementProjectionFailure::Integration(
            GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation
        ))
    );

    let integer_html: MeasurementDocument = toml::from_str(
        &toml::to_string(&measurement)
            .expect("measurement TOML")
            .replace("declared_type = \"text\"", "declared_type = \"integer\""),
    )
    .expect("integer HTML");
    let prepared_integer =
        PreparedMeasurementProjection::prepare(&integer_html).expect("prepared integer");
    assert_eq!(
        prepared_integer.execute(&integer_html, &source, &file_document("<a>text</a>")),
        Err(MeasurementProjectionFailure::Extraction(
            ExtractionFailureReason::ValueUnparseable
        ))
    );
    assert!(build_html_input("", "body", None).is_err());
    assert!(
        build_html_input(
            "https://example.test/source",
            "body",
            Some(url::Url::parse("file:///tmp/source").expect("file URL")),
        )
        .is_err()
    );
}

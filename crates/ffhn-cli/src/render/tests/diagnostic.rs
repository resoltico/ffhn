use super::super::*;

fn detail(value: serde_json::Value) -> ffhn_core::DiagnosticDetail {
    serde_json::from_value(value).expect("diagnostic detail")
}

#[test]
fn summary_renders_typed_message_truncation_without_a_payload_marker() {
    let detail = detail(serde_json::json!({
        "kind": "contract",
        "operation": "state_load",
        "message": "x".repeat(1_024),
        "message_truncation": {
            "original_len_bytes": "1025",
            "original_sha256": "a".repeat(64),
        },
    }));
    let mut summary = Vec::new();
    render_error_detail(&mut summary, &detail).expect("summary");
    let summary = String::from_utf8(summary).expect("UTF-8 summary");

    assert!(summary.contains("Diagnostic message truncation: original_len_bytes=1025"));
    assert!(!summary.contains("[truncated]"));
}

#[test]
fn summary_renders_every_closed_fetch_failure_fact() {
    let cases = [
        (
            serde_json::json!({"kind": "http_status", "status": 503}),
            "Fetch failure: http_status (status=503)",
        ),
        (
            serde_json::json!({
                "kind": "http_content_length_exceeded",
                "configured_max_bytes": 1_024,
                "content_length": 1_025,
            }),
            "Fetch failure: http_content_length_exceeded (configured_max_bytes=1024, content_length=1025)",
        ),
        (
            serde_json::json!({
                "kind": "body_bytes_exceeded",
                "configured_max_bytes": 1_024,
                "observed_bytes": 1_025,
            }),
            "Fetch failure: body_bytes_exceeded (configured_max_bytes=1024, observed_bytes=1025)",
        ),
        (
            serde_json::json!({"kind": "invalid_utf8"}),
            "Fetch failure: invalid_utf8",
        ),
    ];

    for (fetch_failure, expected) in cases {
        let detail = detail(serde_json::json!({
            "kind": "io",
            "operation": "http_fetch",
            "message": "the HTTP response could not be accepted",
            "fetch_failure": fetch_failure,
        }));
        let mut summary = Vec::new();
        render_error_detail(&mut summary, &detail).expect("fetch summary");
        let summary = String::from_utf8(summary).expect("UTF-8 summary");

        assert!(
            summary.contains(expected),
            "missing {expected:?} in {summary:?}"
        );
    }
}

#[test]
fn summary_renders_every_htmlcut_failure_fact_with_line_safe_text() {
    let detail = detail(serde_json::json!({
        "kind": "htmlcut",
        "operation": "html_extraction",
        "message": "CSS selector is invalid.",
        "htmlcut_failure": {
            "error_class": "plan_invalid",
            "core_diagnostic_code": "INVALID_SELECTOR",
            "candidate_count": 2,
            "plan_digest_sha256": "a".repeat(64),
            "selector_parse": {
                "line": 2,
                "column_utf16": 7,
                "parse_error_class": "invalid_attribute_selector",
            },
            "diagnostics": [
                {
                    "level": "error",
                    "code": "INVALID_SELECTOR",
                    "message": "selector parser\nrejected the input",
                    "details": {
                        "kind": "selector_parse",
                        "selector_parse": {
                            "line": 2,
                            "column_utf16": 7,
                            "parse_error_class": "invalid_attribute_selector",
                        },
                    },
                },
                {
                    "level": "error",
                    "code": "AMBIGUOUS_MATCH",
                    "message": "ambiguous",
                    "details": {
                        "kind": "candidate_selection",
                        "candidate_count": 2,
                    },
                },
                {
                    "level": "error",
                    "code": "MATCH_INDEX_OUT_OF_RANGE",
                    "message": "missing index",
                    "details": {
                        "kind": "candidate_selection",
                        "candidate_count": 2,
                        "requested_index": 3,
                    },
                },
                {
                    "level": "warning",
                    "code": "MULTIPLE_MATCHES",
                    "message": "first selected",
                    "details": {
                        "kind": "candidate_selection",
                        "candidate_count": 2,
                        "selected_index": 1,
                    },
                },
                {
                    "level": "warning",
                    "code": "EFFECTIVE_BASE_URL_UNRESOLVED",
                    "message": "base unresolved",
                    "details": {
                        "kind": "effective_base_url_unresolved",
                        "document_base_href": "https://example.test/base\npath",
                        "rewrite_requested": true,
                    },
                },
                {
                    "level": "warning",
                    "code": "SLICE_SPLITS_MARKUP",
                    "message": "markup split",
                    "details": {
                        "kind": "slice_splits_markup",
                        "affected_matches": [{
                            "match_index": 1,
                            "candidate_index": 2,
                            "selected_range": {"start": 3, "end": 5},
                        }],
                    },
                },
                {
                    "level": "error",
                    "code": "NO_MATCH",
                    "message": "end delimiter missing",
                    "details": {
                        "kind": "slice_pattern",
                        "from": "<start>",
                        "to": "<end>",
                        "offset": 4,
                    },
                },
                {
                    "level": "error",
                    "code": "INVALID_SLICE_PATTERN",
                    "message": "regex invalid",
                    "details": {
                        "kind": "slice_pattern",
                        "pattern": "[",
                        "flags": "i",
                    },
                },
                {
                    "level": "error",
                    "code": "UNSUPPORTED_VALUE_TYPE",
                    "message": "unsupported output",
                    "details": {
                        "kind": "unsupported_value_type",
                        "strategy": "css",
                        "value": "url",
                        "path": "a[href]",
                    },
                },
                {
                    "level": "error",
                    "code": "MISSING_ATTRIBUTE",
                    "message": "CSS attribute missing",
                    "details": {
                        "kind": "missing_attribute",
                        "attribute": "href",
                        "path": "a",
                    },
                },
                {
                    "level": "error",
                    "code": "MISSING_ATTRIBUTE",
                    "message": "slice attribute missing",
                    "details": {
                        "kind": "missing_attribute",
                        "attribute": "href",
                        "selected_range": {"start": 6, "end": 9},
                        "hint": "none",
                    },
                },
                {
                    "level": "info",
                    "code": "SOURCE_LOAD_FAILED",
                    "message": "supplemental source fact",
                },
            ],
        },
    }));
    let mut summary = Vec::new();
    render_error_detail(&mut summary, &detail).expect("HTMLCut summary");
    let summary = String::from_utf8(summary).expect("UTF-8 summary");

    for expected in [
        "HTMLCut error class: plan_invalid",
        "HTMLCut core diagnostic code: INVALID_SELECTOR",
        "HTMLCut candidate count: 2",
        "HTMLCut plan digest: ",
        "HTMLCut selector parse: line=2, column_utf16=7, parse_error_class=invalid_attribute_selector",
        "HTMLCut diagnostics: 12",
        "HTMLCut diagnostic: level=error, code=INVALID_SELECTOR, message=\"selector parser\\nrejected the input\"",
        "HTMLCut candidate selection: candidate_count=2, requested_index=none, selected_index=none",
        "HTMLCut candidate selection: candidate_count=2, requested_index=3, selected_index=none",
        "HTMLCut candidate selection: candidate_count=2, requested_index=none, selected_index=1",
        "HTMLCut effective-base URL: document_base_href=\"https://example.test/base\\npath\", rewrite_requested=true",
        "HTMLCut markup split: match_index=1, candidate_index=2, selected_range=[3, 5)",
        "HTMLCut slice pattern: from=\"<start>\", to=\"<end>\", offset=4, pattern=none, flags=none",
        "HTMLCut slice pattern: from=none, to=none, offset=none, pattern=\"[\", flags=\"i\"",
        "HTMLCut unsupported value type: strategy=css, value=url, path=\"a[href]\"",
        "HTMLCut missing attribute: attribute=href, path=\"a\", selected_range=none, hint=none",
        "HTMLCut missing attribute: attribute=href, path=none, selected_range=[6, 9), hint=\"none\"",
        "HTMLCut diagnostic: level=info, code=SOURCE_LOAD_FAILED, message=supplemental source fact",
    ] {
        assert!(
            summary.contains(expected),
            "missing {expected:?} in {summary:?}"
        );
    }
    assert!(!summary.contains("selector parser\nrejected the input"));
    assert!(!summary.contains("https://example.test/base\npath"));
}

#[test]
fn summary_renders_each_htmlcut_boundary_evidence_variant() {
    let cases = [
        (
            serde_json::json!({
                "error_class": "ffhn_boundary_invariant_violation",
                "boundary_evidence": {
                    "kind": "selected_match_count",
                    "selected_match_count": 2,
                },
            }),
            "HTMLCut boundary evidence: selected_match_count=2",
        ),
        (
            serde_json::json!({
                "error_class": "missing_attribute",
                "boundary_evidence": {
                    "kind": "requested_css_attribute",
                    "attribute": "href",
                },
            }),
            "HTMLCut boundary evidence: requested_css_attribute=href",
        ),
    ];

    for (mut failure, expected) in cases {
        failure["plan_digest_sha256"] = serde_json::json!("b".repeat(64));
        failure["diagnostics"] = serde_json::json!([]);
        let integration_fault_code = (failure["error_class"]
            == serde_json::json!("ffhn_boundary_invariant_violation"))
        .then_some(serde_json::json!("ffhn_boundary_invariant_violation"));
        let detail = detail(serde_json::json!({
            "kind": "htmlcut",
            "operation": "html_extraction",
            "message": "the HTML projection could not produce a value",
            "htmlcut_failure": failure,
            "integration_fault_code": integration_fault_code,
        }));
        let mut summary = Vec::new();
        render_error_detail(&mut summary, &detail).expect("HTMLCut boundary summary");
        let summary = String::from_utf8(summary).expect("UTF-8 summary");

        assert!(
            summary.contains(expected),
            "missing {expected:?} in {summary:?}"
        );
    }
}

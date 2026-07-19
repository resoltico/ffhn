use htmlcut_core::interop::v1::{InteropDiagnostic, InteropDiagnosticCode, InteropDiagnosticLevel};
use serde_json::{Value, json};

use super::support::*;

fn diagnostic(code: InteropDiagnosticCode, details: Option<Value>) -> InteropDiagnostic {
    InteropDiagnostic {
        level: InteropDiagnosticLevel::Warning,
        code,
        message: "HTMLCut diagnostic evidence".to_owned(),
        details,
    }
}

#[test]
fn htmlcut_interop_evidence_accepts_every_reachable_detail_shape_without_losing_a_fact() {
    let accepted = [
        diagnostic(InteropDiagnosticCode::SourceLoadFailed, None),
        diagnostic(InteropDiagnosticCode::UnsupportedSpecVersion, None),
        diagnostic(
            InteropDiagnosticCode::InvalidSelector,
            Some(json!({
                "selector_parse": {
                    "line": 2,
                    "column_utf16": 3,
                    "parse_error_class": "invalid_attribute_selector"
                }
            })),
        ),
        diagnostic(
            InteropDiagnosticCode::InvalidSlicePattern,
            Some(json!({ "flags": "q" })),
        ),
        diagnostic(
            InteropDiagnosticCode::InvalidSlicePattern,
            Some(json!({ "pattern": "[", "flags": "i" })),
        ),
        diagnostic(
            InteropDiagnosticCode::UnsupportedValueType,
            Some(json!({ "strategy": "selector", "value": "selected-html" })),
        ),
        diagnostic(
            InteropDiagnosticCode::UnsupportedValueType,
            Some(json!({
                "strategy": "selector",
                "value": "selected-html",
                "path": "html > body > main"
            })),
        ),
        diagnostic(InteropDiagnosticCode::NoMatch, None),
        diagnostic(
            InteropDiagnosticCode::NoMatch,
            Some(json!({ "from": "<main>", "to": "</main>" })),
        ),
        diagnostic(
            InteropDiagnosticCode::NoMatch,
            Some(json!({ "from": "<main>", "to": "</main>", "offset": 7 })),
        ),
        diagnostic(
            InteropDiagnosticCode::AmbiguousMatch,
            Some(json!({ "candidateCount": 2 })),
        ),
        diagnostic(
            InteropDiagnosticCode::MatchIndexOutOfRange,
            Some(json!({ "requestedIndex": 3, "candidateCount": 2 })),
        ),
        diagnostic(
            InteropDiagnosticCode::MissingAttribute,
            Some(json!({ "attribute": "href", "path": "html > body > main" })),
        ),
        diagnostic(
            InteropDiagnosticCode::MissingAttribute,
            Some(json!({
                "attribute": "href",
                "selectedRange": { "start": 5, "end": 9 },
                "hint": "use --boundary-retention start"
            })),
        ),
        diagnostic(
            InteropDiagnosticCode::MultipleMatches,
            Some(json!({ "candidateCount": 2, "selectedIndex": 1 })),
        ),
        diagnostic(
            InteropDiagnosticCode::EffectiveBaseUrlUnresolved,
            Some(json!({ "documentBaseHref": null, "rewriteRequested": true })),
        ),
        diagnostic(
            InteropDiagnosticCode::SliceSplitsMarkup,
            Some(json!({
                "affectedMatches": [{
                    "matchIndex": 1,
                    "candidateIndex": 2,
                    "selectedRange": { "start": 5, "end": 9 }
                }]
            })),
        ),
    ];

    for upstream in accepted {
        let retained = HtmlcutDiagnostic::from_interop(upstream.clone())
            .expect("reachable HTMLCut evidence must be retained");
        assert_eq!(retained.code(), upstream.code.as_str());
        assert_eq!(retained.message(), upstream.message);
        retained.validate().expect("retained evidence is durable");
    }
}

#[test]
fn selector_parse_evidence_covers_the_complete_pinned_htmlcut_vocabulary() {
    let classes = [
        "unexpected_token",
        "end_of_input",
        "invalid_at_rule",
        "invalid_at_rule_body",
        "invalid_qualified_rule",
        "pseudo_element_expected_colon",
        "pseudo_element_expected_ident",
        "invalid_attribute_selector",
        "empty_selector",
        "dangling_combinator",
        "non_compound_selector",
        "non_pseudo_element_after_slotted",
        "invalid_pseudo_element_after_slotted",
        "invalid_pseudo_element_inside_where",
        "invalid_state",
        "unexpected_token_in_attribute_selector",
        "no_ident_for_pseudo",
        "unsupported_pseudo_class_or_element",
        "unexpected_ident",
        "expected_namespace",
        "expected_bar_in_attribute_selector",
        "invalid_attribute_value",
        "invalid_qualified_name_in_attribute_selector",
        "explicit_namespace_unexpected_token",
        "class_needs_ident",
    ];

    for parse_error_class in classes {
        let retained = HtmlcutDiagnostic::from_interop(diagnostic(
            InteropDiagnosticCode::InvalidSelector,
            Some(json!({
                "selector_parse": {
                    "line": 1,
                    "column_utf16": 1,
                    "parse_error_class": parse_error_class,
                }
            })),
        ))
        .expect("every published selector-parser class remains representable");
        retained
            .validate()
            .expect("retained selector parse evidence");
    }

    let candidate_details = HtmlcutDiagnostic::from_interop(diagnostic(
        InteropDiagnosticCode::AmbiguousMatch,
        Some(json!({ "candidateCount": 2 })),
    ))
    .expect("candidate-selection evidence");
    assert!(
        candidate_details
            .details()
            .and_then(HtmlcutDiagnosticDetails::selector_parse)
            .is_none()
    );
}

#[test]
fn htmlcut_interop_evidence_rejects_unknown_or_incoherent_shapes_at_the_boundary() {
    let rejected = [
        InteropDiagnostic {
            level: InteropDiagnosticLevel::Warning,
            code: InteropDiagnosticCode::NoMatch,
            message: "x".repeat(1_025),
            details: None,
        },
        diagnostic(
            InteropDiagnosticCode::SourceLoadFailed,
            Some(json!({ "source": "unreachable-by-FFHN" })),
        ),
        diagnostic(
            InteropDiagnosticCode::InvalidSelector,
            Some(json!({
                "selector_parse": {
                    "line": 0,
                    "column_utf16": 1,
                    "parse_error_class": "invalid_attribute_selector"
                }
            })),
        ),
        diagnostic(
            InteropDiagnosticCode::InvalidSelector,
            Some(json!({
                "selector_parse": {
                    "line": 1,
                    "column_utf16": 1,
                    "parse_error_class": "future_parser_class"
                }
            })),
        ),
        diagnostic(
            InteropDiagnosticCode::InvalidSlicePattern,
            Some(json!({ "pattern": "[" })),
        ),
        diagnostic(
            InteropDiagnosticCode::UnsupportedValueType,
            Some(json!({ "strategy": "selector", "value": "selected-html", "future": 1 })),
        ),
        diagnostic(
            InteropDiagnosticCode::NoMatch,
            Some(json!({ "from": "<main>", "to": "</main>", "offset": 0 })),
        ),
        diagnostic(
            InteropDiagnosticCode::AmbiguousMatch,
            Some(json!({ "candidateCount": 1 })),
        ),
        diagnostic(
            InteropDiagnosticCode::MatchIndexOutOfRange,
            Some(json!({ "requestedIndex": 2, "candidateCount": 2 })),
        ),
        diagnostic(
            InteropDiagnosticCode::MissingAttribute,
            Some(json!({ "attribute": "href", "path": "x", "hint": "future" })),
        ),
        diagnostic(
            InteropDiagnosticCode::MissingAttribute,
            Some(json!({ "attribute": "href", "path": null })),
        ),
        diagnostic(
            InteropDiagnosticCode::MultipleMatches,
            Some(json!({ "candidateCount": 2, "selectedIndex": 2 })),
        ),
        diagnostic(
            InteropDiagnosticCode::SliceSplitsMarkup,
            Some(json!({ "affectedMatches": [] })),
        ),
        diagnostic(
            InteropDiagnosticCode::SliceSplitsMarkup,
            Some(json!({
                "affectedMatches": [{
                    "matchIndex": 0,
                    "candidateIndex": 1,
                    "selectedRange": { "start": 1, "end": 2 }
                }]
            })),
        ),
        diagnostic(
            InteropDiagnosticCode::SliceSplitsMarkup,
            Some(json!({
                "affectedMatches": [{
                    "matchIndex": 1,
                    "candidateIndex": 1,
                    "selectedRange": { "start": 2, "end": 1 }
                }]
            })),
        ),
        diagnostic(
            InteropDiagnosticCode::NoMatch,
            Some(json!({ "from": "", "to": "</main>" })),
        ),
    ];

    for upstream in rejected {
        assert!(
            HtmlcutDiagnostic::from_interop(upstream).is_err(),
            "unmodeled or incoherent HTMLCut evidence must not cross the FFHN boundary"
        );
    }
}

#[test]
fn persisted_htmlcut_evidence_rejects_invented_detail_families_and_fields() {
    let retained = HtmlcutDiagnostic::from_interop(diagnostic(
        InteropDiagnosticCode::MultipleMatches,
        Some(json!({ "candidateCount": 2, "selectedIndex": 1 })),
    ))
    .expect("valid evidence");
    let mut wire = serde_json::to_value(retained).expect("diagnostic JSON");
    wire["details"] = json!({
        "kind": "candidate_selection",
        "candidate_count": 2,
        "selected_index": 2
    });
    let invented: HtmlcutDiagnostic =
        serde_json::from_value(wire).expect("structurally valid FFHN diagnostic");
    assert!(invented.validate().is_err());

    let mut mismatched_code = serde_json::to_value(&invented).expect("diagnostic JSON");
    mismatched_code["code"] = json!("NO_MATCH");
    let mismatched: HtmlcutDiagnostic =
        serde_json::from_value(mismatched_code).expect("structurally valid FFHN diagnostic");
    assert!(mismatched.validate().is_err());

    let missing_attribute = HtmlcutDiagnostic::from_interop(diagnostic(
        InteropDiagnosticCode::MissingAttribute,
        Some(json!({ "attribute": "href", "path": "html > body > main" })),
    ))
    .expect("valid CSS missing-attribute evidence");
    let mut missing_attribute_wire =
        serde_json::to_value(missing_attribute).expect("missing-attribute JSON");
    missing_attribute_wire["details"]["path"] = Value::Null;
    let missing_attribute: HtmlcutDiagnostic = serde_json::from_value(missing_attribute_wire)
        .expect("structurally valid but incoherent missing-attribute evidence");
    assert!(missing_attribute.validate().is_err());

    let unknown_detail = json!({
        "level": "warning",
        "code": "MULTIPLE_MATCHES",
        "message": "HTMLCut diagnostic evidence",
        "details": {
            "kind": "candidate_selection",
            "candidate_count": 2,
            "selected_index": 1,
            "invented": true
        }
    });
    assert!(serde_json::from_value::<HtmlcutDiagnostic>(unknown_detail).is_err());
}

#[test]
fn persisted_htmlcut_evidence_rejects_every_incoherent_validator_combination() {
    let malformed = vec![
        json!({
            "level": "warning",
            "code": "SLICE_SPLITS_MARKUP",
            "message": "HTMLCut diagnostic evidence",
            "details": {
                "kind": "slice_splits_markup",
                "affected_matches": [{
                    "match_index": 1,
                    "candidate_index": 0,
                    "selected_range": { "start": 1, "end": 2 }
                }]
            }
        }),
        json!({
            "level": "warning",
            "code": "INVALID_SELECTOR",
            "message": "HTMLCut diagnostic evidence",
            "details": {
                "kind": "selector_parse",
                "selector_parse": {
                    "line": 1,
                    "column_utf16": 0,
                    "parse_error_class": "invalid_attribute_selector"
                }
            }
        }),
        json!({
            "level": "warning",
            "code": "INVALID_SLICE_PATTERN",
            "message": "HTMLCut diagnostic evidence",
            "details": { "kind": "slice_pattern" }
        }),
        json!({
            "level": "warning",
            "code": "NO_MATCH",
            "message": "HTMLCut diagnostic evidence",
            "details": {
                "kind": "slice_pattern",
                "from": "x".repeat(1_025),
                "to": "</main>"
            }
        }),
        json!({
            "level": "warning",
            "code": "AMBIGUOUS_MATCH",
            "message": "HTMLCut diagnostic evidence",
            "details": {
                "kind": "candidate_selection",
                "candidate_count": 2,
                "requested_index": 1
            }
        }),
        json!({
            "level": "warning",
            "code": "AMBIGUOUS_MATCH",
            "message": "HTMLCut diagnostic evidence",
            "details": {
                "kind": "candidate_selection",
                "candidate_count": 2,
                "selected_index": 1
            }
        }),
        json!({
            "level": "warning",
            "code": "MATCH_INDEX_OUT_OF_RANGE",
            "message": "HTMLCut diagnostic evidence",
            "details": {
                "kind": "candidate_selection",
                "candidate_count": 2,
                "requested_index": 2
            }
        }),
        json!({
            "level": "warning",
            "code": "MATCH_INDEX_OUT_OF_RANGE",
            "message": "HTMLCut diagnostic evidence",
            "details": {
                "kind": "candidate_selection",
                "candidate_count": 2,
                "requested_index": 3,
                "selected_index": 1
            }
        }),
        json!({
            "level": "warning",
            "code": "MULTIPLE_MATCHES",
            "message": "HTMLCut diagnostic evidence",
            "details": {
                "kind": "candidate_selection",
                "candidate_count": 1,
                "selected_index": 1
            }
        }),
        json!({
            "level": "warning",
            "code": "MULTIPLE_MATCHES",
            "message": "HTMLCut diagnostic evidence",
            "details": {
                "kind": "candidate_selection",
                "candidate_count": 2,
                "requested_index": 1,
                "selected_index": 1
            }
        }),
    ];

    for wire in malformed {
        let diagnostic: HtmlcutDiagnostic =
            serde_json::from_value(wire).expect("structurally valid persisted evidence");
        assert!(diagnostic.validate().is_err());
    }
}

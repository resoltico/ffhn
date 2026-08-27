use htmlcut_core::interop::v1::{InteropDiagnostic, InteropDiagnosticCode, InteropDiagnosticLevel};
use serde_json::{Value, json};

use super::*;

fn diagnostic(code: InteropDiagnosticCode, details: Option<Value>) -> InteropDiagnostic {
    InteropDiagnostic {
        level: InteropDiagnosticLevel::Warning,
        code,
        message: "HTMLCut diagnostic evidence".to_owned(),
        details,
    }
}

#[test]
fn every_reachable_interop_detail_shape_crosses_without_fact_loss() {
    let accepted = [
        InteropDiagnostic {
            level: InteropDiagnosticLevel::Warning,
            code: InteropDiagnosticCode::SourceLoadFailed,
            message: "x".repeat(1_024),
            details: None,
        },
        diagnostic(InteropDiagnosticCode::SourceLoadFailed, None),
        diagnostic(InteropDiagnosticCode::UnsupportedSpecVersion, None),
        diagnostic(
            InteropDiagnosticCode::InvalidSelector,
            Some(
                json!({"selector_parse":{"line":2,"column_utf16":3,"parse_error_class":"invalid_attribute_selector"}}),
            ),
        ),
        diagnostic(
            InteropDiagnosticCode::InvalidSlicePattern,
            Some(json!({"flags":"q"})),
        ),
        diagnostic(
            InteropDiagnosticCode::InvalidSlicePattern,
            Some(json!({"pattern":"[","flags":"i"})),
        ),
        diagnostic(
            InteropDiagnosticCode::UnsupportedValueType,
            Some(json!({"strategy":"selector","value":"selected-html"})),
        ),
        diagnostic(
            InteropDiagnosticCode::UnsupportedValueType,
            Some(
                json!({"strategy":"selector","value":"selected-html","path":"html > body > main"}),
            ),
        ),
        diagnostic(InteropDiagnosticCode::NoMatch, None),
        diagnostic(
            InteropDiagnosticCode::NoMatch,
            Some(json!({"from":"<main>","to":"</main>"})),
        ),
        diagnostic(
            InteropDiagnosticCode::NoMatch,
            Some(json!({"from":"<main>","to":"</main>","offset":7})),
        ),
        diagnostic(
            InteropDiagnosticCode::AmbiguousMatch,
            Some(json!({"candidateCount":2})),
        ),
        diagnostic(
            InteropDiagnosticCode::MatchIndexOutOfRange,
            Some(json!({"requestedIndex":3,"candidateCount":2})),
        ),
        diagnostic(
            InteropDiagnosticCode::MissingAttribute,
            Some(json!({"attribute":"href","path":"html > body > main"})),
        ),
        diagnostic(
            InteropDiagnosticCode::MissingAttribute,
            Some(
                json!({"attribute":"href","selectedRange":{"start":5,"end":9},"hint":"retain boundary"}),
            ),
        ),
        diagnostic(
            InteropDiagnosticCode::MultipleMatches,
            Some(json!({"candidateCount":2,"selectedIndex":1})),
        ),
        diagnostic(
            InteropDiagnosticCode::EffectiveBaseUrlUnresolved,
            Some(json!({"documentBaseHref":null,"rewriteRequested":true})),
        ),
        diagnostic(
            InteropDiagnosticCode::EffectiveBaseUrlUnresolved,
            Some(json!({"documentBaseHref":"https://example.test/","rewriteRequested":true})),
        ),
        diagnostic(
            InteropDiagnosticCode::SliceSplitsMarkup,
            Some(
                json!({"affectedMatches":[{"matchIndex":1,"candidateIndex":2,"selectedRange":{"start":5,"end":9}}]}),
            ),
        ),
    ];
    for upstream in accepted {
        let retained =
            HtmlcutDiagnostic::from_interop(upstream.clone()).expect("reachable HTMLCut evidence");
        assert_eq!(retained.code(), upstream.code.as_str());
        assert_eq!(retained.level(), "warning");
        assert_eq!(retained.message(), upstream.message);
        if let Some(details) = retained.details() {
            match details {
                HtmlcutDiagnosticDetails::SelectorParse { selector_parse } => {
                    assert!(selector_parse.line() > 0);
                    assert!(selector_parse.column_utf16() > 0);
                    let _ = selector_parse.parse_error_class();
                }
                HtmlcutDiagnosticDetails::SliceSplitsMarkup { affected_matches } => {
                    let affected = &affected_matches[0];
                    assert_eq!(affected.match_index(), 1);
                    assert_eq!(affected.candidate_index(), 2);
                    assert_eq!(affected.selected_range().start(), 5);
                    assert_eq!(affected.selected_range().end(), 9);
                }
                HtmlcutDiagnosticDetails::EffectiveBaseUrlUnresolved {
                    rewrite_requested, ..
                } => assert!(*rewrite_requested),
                _ => {}
            }
        }
        retained.validate().expect("durable evidence");
    }

    for (level, expected) in [
        (InteropDiagnosticLevel::Error, "error"),
        (InteropDiagnosticLevel::Info, "info"),
    ] {
        let mut upstream = diagnostic(InteropDiagnosticCode::NoMatch, None);
        upstream.level = level;
        assert_eq!(
            HtmlcutDiagnostic::from_interop(upstream)
                .expect("diagnostic level")
                .level(),
            expected
        );
    }
}

#[test]
fn selector_parse_translation_covers_the_pinned_closed_vocabulary() {
    for parse_error_class in [
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
    ] {
        HtmlcutDiagnostic::from_interop(diagnostic(
            InteropDiagnosticCode::InvalidSelector,
            Some(json!({"selector_parse":{"line":1,"column_utf16":1,"parse_error_class":parse_error_class}})),
        ))
        .expect("published parser class")
        .validate()
        .expect("durable parser class");
    }
}

#[test]
fn unknown_or_incoherent_upstream_shapes_fail_at_the_boundary() {
    let rejected = [
        InteropDiagnostic {
            level: InteropDiagnosticLevel::Warning,
            code: InteropDiagnosticCode::NoMatch,
            message: "x".repeat(1_025),
            details: None,
        },
        diagnostic(
            InteropDiagnosticCode::SourceLoadFailed,
            Some(json!({"source":"future"})),
        ),
        diagnostic(
            InteropDiagnosticCode::InvalidSelector,
            Some(
                json!({"selector_parse":{"line":0,"column_utf16":1,"parse_error_class":"invalid_attribute_selector"}}),
            ),
        ),
        diagnostic(
            InteropDiagnosticCode::InvalidSelector,
            Some(
                json!({"selector_parse":{"line":1,"column_utf16":1,"parse_error_class":"future"}}),
            ),
        ),
        diagnostic(
            InteropDiagnosticCode::InvalidSlicePattern,
            Some(json!({"pattern":"["})),
        ),
        diagnostic(
            InteropDiagnosticCode::UnsupportedValueType,
            Some(json!({"strategy":"selector","value":"selected-html","future":1})),
        ),
        diagnostic(
            InteropDiagnosticCode::NoMatch,
            Some(json!({"from":"<main>","to":"</main>","offset":0})),
        ),
        diagnostic(
            InteropDiagnosticCode::AmbiguousMatch,
            Some(json!({"candidateCount":1})),
        ),
        diagnostic(
            InteropDiagnosticCode::MatchIndexOutOfRange,
            Some(json!({"requestedIndex":2,"candidateCount":2})),
        ),
        diagnostic(
            InteropDiagnosticCode::MissingAttribute,
            Some(json!({"attribute":"href","path":"x","hint":"future"})),
        ),
        diagnostic(
            InteropDiagnosticCode::MissingAttribute,
            Some(json!({"attribute":"href","path":null})),
        ),
        diagnostic(
            InteropDiagnosticCode::MultipleMatches,
            Some(json!({"candidateCount":2,"selectedIndex":2})),
        ),
        diagnostic(
            InteropDiagnosticCode::SliceSplitsMarkup,
            Some(json!({"affectedMatches":[]})),
        ),
        diagnostic(
            InteropDiagnosticCode::SliceSplitsMarkup,
            Some(
                json!({"affectedMatches":[{"matchIndex":0,"candidateIndex":1,"selectedRange":{"start":1,"end":2}}]}),
            ),
        ),
        diagnostic(
            InteropDiagnosticCode::SliceSplitsMarkup,
            Some(
                json!({"affectedMatches":[{"matchIndex":1,"candidateIndex":1,"selectedRange":{"start":2,"end":1}}]}),
            ),
        ),
        diagnostic(
            InteropDiagnosticCode::NoMatch,
            Some(json!({"from":"","to":"</main>"})),
        ),
    ];
    for upstream in rejected {
        assert!(HtmlcutDiagnostic::from_interop(upstream).is_err());
    }
}

#[test]
fn persisted_diagnostic_validation_rejects_crossed_and_invented_shapes() {
    let valid = HtmlcutDiagnostic::from_interop(diagnostic(
        InteropDiagnosticCode::MultipleMatches,
        Some(json!({"candidateCount":2,"selectedIndex":1})),
    ))
    .expect("valid evidence");
    let mut crossed = serde_json::to_value(&valid).expect("wire");
    crossed["code"] = json!("NO_MATCH");
    let crossed: HtmlcutDiagnostic = serde_json::from_value(crossed).expect("structural wire");
    assert!(crossed.validate().is_err());

    let unknown = json!({
        "level":"warning",
        "code":"MULTIPLE_MATCHES",
        "message":"message",
        "details":{"kind":"candidate_selection","candidate_count":2,"selected_index":1,"future":true}
    });
    assert!(serde_json::from_value::<HtmlcutDiagnostic>(unknown).is_err());

    for wire in [
        json!({"level":"warning","code":"SLICE_SPLITS_MARKUP","message":"message","details":{"kind":"slice_splits_markup","affected_matches":[{"match_index":1,"candidate_index":0,"selected_range":{"start":1,"end":2}}]}}),
        json!({"level":"warning","code":"INVALID_SELECTOR","message":"message","details":{"kind":"selector_parse","selector_parse":{"line":1,"column_utf16":0,"parse_error_class":"invalid_attribute_selector"}}}),
        json!({"level":"warning","code":"INVALID_SLICE_PATTERN","message":"message","details":{"kind":"slice_pattern"}}),
        json!({"level":"warning","code":"NO_MATCH","message":"message","details":{"kind":"slice_pattern","from":"x".repeat(1_025),"to":"end"}}),
        json!({"level":"warning","code":"AMBIGUOUS_MATCH","message":"message","details":{"kind":"candidate_selection","candidate_count":2,"requested_index":1}}),
        json!({"level":"warning","code":"AMBIGUOUS_MATCH","message":"message","details":{"kind":"candidate_selection","candidate_count":2,"selected_index":1}}),
        json!({"level":"warning","code":"MATCH_INDEX_OUT_OF_RANGE","message":"message","details":{"kind":"candidate_selection","candidate_count":2,"requested_index":2}}),
        json!({"level":"warning","code":"MATCH_INDEX_OUT_OF_RANGE","message":"message","details":{"kind":"candidate_selection","candidate_count":2,"requested_index":3,"selected_index":1}}),
        json!({"level":"warning","code":"MULTIPLE_MATCHES","message":"message","details":{"kind":"candidate_selection","candidate_count":1,"selected_index":1}}),
        json!({"level":"warning","code":"MULTIPLE_MATCHES","message":"message","details":{"kind":"candidate_selection","candidate_count":2,"requested_index":1,"selected_index":1}}),
        json!({"level":"warning","code":"MISSING_ATTRIBUTE","message":"message","details":{"kind":"missing_attribute","attribute":"href","path":"main > a","selected_range":{"start":0,"end":1}}}),
    ] {
        let diagnostic: HtmlcutDiagnostic = serde_json::from_value(wire).expect("structural wire");
        assert!(diagnostic.validate().is_err());
    }
}

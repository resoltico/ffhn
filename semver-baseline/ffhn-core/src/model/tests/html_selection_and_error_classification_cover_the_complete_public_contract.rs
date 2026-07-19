use super::support::*;

#[test]
fn html_selection_and_error_classification_cover_the_complete_public_contract() {
    use crate::{HtmlcutDiagnosticCode, HtmlcutErrorClass, HtmlcutFailureDetails};

    let selection_target = html_target("html_text", "article", None, "integer", "");
    let Projection::HtmlText { selection } = selection_target.projection() else {
        panic!("fixture must produce an HTML text projection");
    };
    assert_eq!(
        selection.strategy().kind(),
        htmlcut_core::interop::v1::StrategyKind::CssSelector
    );
    assert!(matches!(
        selection.selection(),
        htmlcut_core::interop::v1::Selection::Single
    ));
    assert!(matches!(
        selection.rendering(),
        htmlcut_core::interop::v1::Rendering { .. }
    ));
    assert_eq!(
        selection.structured_plan().output.kind(),
        htmlcut_core::interop::v1::OutputKind::Structured
    );

    for (diagnostic_code, expected) in [
        (
            HtmlcutDiagnosticCode::InvalidSelector,
            PermanentErrorCode::HtmlcutInvalidSelector,
        ),
        (
            HtmlcutDiagnosticCode::InvalidSlicePattern,
            PermanentErrorCode::HtmlcutInvalidSlicePattern,
        ),
    ] {
        let failure = HtmlcutFailureDetails::new(
            HtmlcutErrorClass::PlanInvalid,
            None,
            "a".repeat(64),
            Vec::new(),
        )
        .with_core_diagnostic_code(diagnostic_code);
        assert_eq!(permanent_code_for_htmlcut_failure(&failure), expected);
    }
    let fallback = HtmlcutFailureDetails::new(
        HtmlcutErrorClass::PlanInvalid,
        None,
        "a".repeat(64),
        Vec::new(),
    );
    assert_eq!(
        permanent_code_for_htmlcut_failure(&fallback),
        PermanentErrorCode::HtmlcutPlanInvalid
    );

    let invalid_plan = mutate_target(&selection_target, |wire| {
        wire["projection"]["selection"]["strategy"] = serde_json::json!({
            "kind": "delimiter_pair",
            "start": "<start>",
            "end": "<end>",
            "mode": "literal",
            "boundary_retention": "exclude_both",
            "flags": ["case_insensitive"]
        });
    });
    assert_eq!(
        invalid_plan
            .permanent_error()
            .expect("HTMLCut evidence projection")
            .map(|error| error.code()),
        Some(PermanentErrorCode::HtmlTextRequiresCssSelector)
    );
}

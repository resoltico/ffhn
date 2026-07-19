use super::support::*;
use crate::{HtmlcutDiagnosticCode, HtmlcutErrorClass};

#[test]
fn htmlcut_invalid_selector_begins_a_permanent_contract_error_episode() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(paths.target_dir().join("source.html"), "<main>value</main>").expect("HTML source");
    write_html_target(&paths, "html_text", "[", None, "integer", "");
    let report = run_once(&paths).expect("invalid selector run");
    assert_eq!(report.outcome(), RunOutcome::ConfigInvalid);
    let failure = report
        .error_detail()
        .and_then(DiagnosticDetail::htmlcut_failure)
        .expect("invalid-selector detail");
    assert_eq!(failure.error_class(), HtmlcutErrorClass::PlanInvalid);
    assert_eq!(
        failure.core_diagnostic_code(),
        Some(HtmlcutDiagnosticCode::InvalidSelector)
    );
    let selector_parse = failure
        .selector_parse()
        .expect("invalid-selector details retain HTMLCut's parse location");
    let diagnostic_selector_parse = failure
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "INVALID_SELECTOR")
        .and_then(HtmlcutDiagnostic::details)
        .and_then(|details| match details {
            HtmlcutDiagnosticDetails::SelectorParse { selector_parse } => Some(selector_parse),
            _ => None,
        })
        .expect("invalid-selector diagnostic retains the same parse detail");
    assert_eq!(selector_parse, diagnostic_selector_parse);
    let selector_parse = serde_json::to_value(selector_parse).expect("selector parse JSON");
    assert_eq!(selector_parse["line"], 1);
    assert!(
        selector_parse["column_utf16"]
            .as_u64()
            .is_some_and(|column| column > 0)
    );
    assert!(selector_parse["parse_error_class"].is_string());
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("permanent-error state"))
            .expect("state JSON");
    assert_eq!(
        state["permanent_error_episode"]["error_code"],
        "htmlcut_invalid_selector"
    );
}

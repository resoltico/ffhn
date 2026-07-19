use super::support::*;
use crate::HtmlcutErrorClass;

#[test]
fn htmlcut_failures_keep_candidate_counts_and_closed_classes_in_the_source_health_detail() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(paths.target_dir().join("source.html"), "<main>empty</main>").expect("HTML source");
    write_html_target(&paths, "html_text", ".price", None, "integer", "");
    let report = run_once(&paths).expect("no-match run");
    assert_eq!(report.outcome(), RunOutcome::AcquisitionFailed);
    let failure = report
        .error_detail()
        .and_then(DiagnosticDetail::htmlcut_failure)
        .expect("HTMLCut failure evidence");
    assert_eq!(failure.error_class(), HtmlcutErrorClass::NoMatch);
    assert_eq!(failure.candidate_count(), Some(0));
    assert_eq!(failure.plan_digest_sha256().len(), 64);
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("persisted suspect state"))
            .expect("state JSON");
    assert_eq!(state["source_health"]["reason_class"], "htmlcut_no_match");
    assert_eq!(
        state["source_health"]["last_details"]["htmlcut_failure"]["candidate_count"],
        0
    );

    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(
        paths.target_dir().join("source.html"),
        "<meta id=\"price\">",
    )
    .expect("HTML source");
    write_html_target(
        &paths,
        "html_attribute",
        "meta#price",
        Some("content"),
        "integer",
        "",
    );
    let report = run_once(&paths).expect("missing-attribute run");
    assert_eq!(report.outcome(), RunOutcome::AcquisitionFailed);
    let failure = report
        .error_detail()
        .and_then(DiagnosticDetail::htmlcut_failure)
        .expect("missing-attribute detail");
    assert_eq!(failure.error_class(), HtmlcutErrorClass::MissingAttribute);
    assert_eq!(failure.candidate_count(), Some(1));
}

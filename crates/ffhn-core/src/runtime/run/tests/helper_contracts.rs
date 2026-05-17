use super::support::*;
use crate::runtime::run::outcome::compare_source_for_basis;
use htmlcut_core::interop::v1::{HttpUrl, Selection};

#[test]
fn helper_functions_cover_error_mapping_and_compare_outcomes() {
    assert_eq!(
        failure_cause_for_htmlcut_error(ErrorCode::PlanInvalid),
        RunFailureCause::SelectionContractInvalid
    );
    assert_eq!(
        failure_cause_for_htmlcut_error(ErrorCode::NoMatch),
        RunFailureCause::SelectionNoMatch
    );
    assert_eq!(
        failure_cause_for_htmlcut_error(ErrorCode::AmbiguousMatch),
        RunFailureCause::SelectionAmbiguousMatch
    );
    assert_eq!(
        failure_cause_for_htmlcut_error(ErrorCode::MissingAttribute),
        RunFailureCause::SelectionInternalError
    );
    assert_eq!(
        failure_cause_for_htmlcut_error(ErrorCode::InternalError),
        RunFailureCause::SelectionInternalError
    );

    assert_eq!(
        run_outcome_from_digests(None, DIGEST),
        RunOutcome::Initialized
    );
    assert_eq!(
        run_outcome_from_digests(Some(DIGEST), DIGEST),
        RunOutcome::Unchanged
    );
    assert_eq!(
        run_outcome_from_digests(
            Some(DIGEST),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        ),
        RunOutcome::Changed
    );
    assert_eq!(
        failure_run_outcome(RunFailureCause::FetchTimeout),
        RunOutcome::FailedTransient
    );
    assert_eq!(
        failure_run_outcome(RunFailureCause::ConfigInvalid),
        RunOutcome::FailedPermanent
    );
    assert_eq!(RunOutcome::SkippedDisabled.as_str(), "skipped_disabled");

    let changed = build_change_section(
        Some("keep\nbefore\nsuffix"),
        "keep\nafter\nsuffix",
        RunOutcome::Changed,
    );
    assert_eq!(changed.kind, ChangeKind::Changed);
    assert_eq!(changed.common_prefix_lines, 1);
    assert_eq!(changed.common_suffix_lines, 1);
    assert_eq!(
        changed
            .changed_region
            .as_ref()
            .expect("changed region")
            .current_excerpt
            .as_deref(),
        Some("after")
    );

    let unchanged = build_change_section(Some("same"), "same", RunOutcome::SkippedDisabled);
    assert_eq!(unchanged.kind, ChangeKind::Unchanged);
    assert!(unchanged.changed_region.is_none());
    assert!(split_lines("").is_empty());
    assert_eq!(common_suffix_len(&["a", "b"], &["x", "b"], 0), 1);
    assert_eq!(
        excerpt_from_lines(&["1", "2", "3", "4", "5"]).as_deref(),
        Some("1\n2\n3\n4\n...")
    );
    let long_line = "x".repeat(300);
    let truncated = excerpt_from_lines(&[long_line.as_str()]);
    assert!(truncated.expect("truncated excerpt").ends_with("..."));
}

#[test]
fn required_outer_html_enforces_the_persisted_artifact_contract() {
    let url = Url::parse("https://example.com").expect("url");
    let target = target_document("demo", true, url.clone(), "main", SelectionMatch::Single);
    let plan = super::super::super::interop::build_htmlcut_plan(
        target.selection_config(),
        target.compare_config_internal(),
    )
    .expect("plan");
    let source = HtmlInput::new("demo".to_owned(), "<main>Hello</main>".to_owned())
        .expect("source")
        .with_input_base_url(HttpUrl::try_from(url).expect("http url"));

    let mut result = execute_plan(&source, &plan).expect("result");
    let selected_match = required_selected_match(&result).expect("selected match");
    assert_eq!(
        required_outer_html(selected_match).expect("outer html"),
        "<main>Hello</main>"
    );

    result.selected_matches[0].outer_html_output = "<main>Hello\r\nworld</main>".to_owned();
    let selected_match = required_selected_match(&result).expect("selected match after mutation");
    assert_eq!(
        required_outer_html(selected_match).expect("normalized outer html"),
        "<main>Hello\nworld</main>"
    );
}

#[test]
fn compare_source_for_basis_projects_text_inner_and_outer_html() {
    let url = Url::parse("https://example.com").expect("url");
    let target = target_document("demo", true, url.clone(), "main", SelectionMatch::Single);
    let plan = super::super::super::interop::build_htmlcut_plan(
        target.selection_config(),
        target.compare_config_internal(),
    )
    .expect("plan");
    let source = HtmlInput::new(
        "demo".to_owned(),
        "<main><span>Hello</span></main>".to_owned(),
    )
    .expect("source")
    .with_input_base_url(HttpUrl::try_from(url).expect("http url"));

    let result = execute_plan(&source, &plan).expect("result");
    let selected_match = required_selected_match(&result).expect("selected match");
    assert_eq!(
        compare_source_for_basis(selected_match, CompareBasis::Text).expect("text compare"),
        "Hello"
    );
    assert_eq!(
        compare_source_for_basis(selected_match, CompareBasis::InnerHtml)
            .expect("inner html compare"),
        "<span>Hello</span>"
    );
    assert_eq!(
        compare_source_for_basis(selected_match, CompareBasis::OuterHtml)
            .expect("outer html compare"),
        "<main><span>Hello</span></main>"
    );
}

#[test]
fn required_selected_match_rejects_htmlcut_all_selection_results_for_ffhn() {
    let url = Url::parse("https://example.com").expect("url");
    let target = target_document("demo", true, url.clone(), "main", SelectionMatch::Single);
    let mut plan = super::super::super::interop::build_htmlcut_plan(
        target.selection_config(),
        target.compare_config_internal(),
    )
    .expect("plan");
    plan.selection = Selection::all();
    let source = HtmlInput::new(
        "demo".to_owned(),
        "<main>One</main><main>Two</main>".to_owned(),
    )
    .expect("source")
    .with_input_base_url(HttpUrl::try_from(url).expect("http url"));

    let result = execute_plan(&source, &plan).expect("all-selection result");
    let error = required_selected_match(&result).expect_err("ffhn should reject all-selection");
    assert!(
        error
            .to_string()
            .contains("expects exactly one selected HTMLCut match")
    );
}

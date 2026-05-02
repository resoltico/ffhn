use super::support::*;

#[test]
fn helper_functions_cover_error_mapping_and_compare_outcomes() {
    assert_eq!(
        reason_code_for_htmlcut_error(ErrorCode::PlanInvalid),
        ReasonCode::ExtractionPlanInvalid
    );
    assert_eq!(
        reason_code_for_htmlcut_error(ErrorCode::NoMatch),
        ReasonCode::ExtractionNoMatch
    );
    assert_eq!(
        reason_code_for_htmlcut_error(ErrorCode::AmbiguousMatch),
        ReasonCode::ExtractionAmbiguousMatch
    );
    assert_eq!(
        reason_code_for_htmlcut_error(ErrorCode::InternalError),
        ReasonCode::ExtractionInternalError
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
        failure_run_outcome(ReasonCode::FetchTimeout),
        RunOutcome::FailedTransient
    );
    assert_eq!(
        failure_run_outcome(ReasonCode::Disabled),
        RunOutcome::FailedPermanent
    );
    assert_eq!(
        notification_event(RunOutcome::SkippedDisabled),
        NotificationEvent::SkippedDisabled
    );

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
    let plan = super::super::super::interop::build_htmlcut_plan(&target).expect("plan");
    let source = HtmlInput::new("demo".to_owned(), "<main>Hello</main>".to_owned())
        .expect("source")
        .with_input_base_url(url);

    let mut result = execute_plan(&source, &plan).expect("result");
    assert_eq!(
        required_outer_html(&result).expect("outer html"),
        "<main>Hello</main>"
    );

    result.selected_matches[0].outer_html = None;
    assert!(required_outer_html(&result).is_err());
}

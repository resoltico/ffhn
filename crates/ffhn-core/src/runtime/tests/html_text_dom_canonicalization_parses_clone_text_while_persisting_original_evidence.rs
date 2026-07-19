use super::support::*;

#[test]
fn html_text_dom_canonicalization_parses_clone_text_while_persisting_original_evidence() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(
        paths.target_dir().join("source.html"),
        "<article class=\"price\"><a href=\"/offer\">1.00</a></article>",
    )
    .expect("HTML source");
    write_html_target(
        &paths,
        "html_rendered_text",
        "article.price a",
        None,
        "decimal",
        "",
    );
    let mut target = fs::read_to_string(paths.target_file()).expect("HTML target");
    target.push_str(
        "\n[projection.selection.dom_canonicalization]\nignore_attributes = [\"href\"]\nstrip_whitespace_nodes = false\n",
    );
    fs::write(paths.target_file(), target).expect("canonicalized HTML target");

    let report = run_once(&paths).expect("canonicalized HTML run");
    assert_eq!(report.outcome(), RunOutcome::Initialized);
    let observation = report.observation().expect("HTML observation");
    assert_eq!(observation.raw_selected(), "1.00 [/offer]");
    assert_eq!(observation.comparison_projection(), "1.00");
    assert_eq!(observation.canonical_value(), "1");
    assert_eq!(observation.htmlcut_candidate_count(), Some(1));

    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("persisted HTML state"))
            .expect("state JSON");
    assert_eq!(
        state["accepted_observation"]["raw_selected"],
        "1.00 [/offer]"
    );
    assert_eq!(
        state["accepted_observation"]["comparison_projection"],
        "1.00"
    );
}

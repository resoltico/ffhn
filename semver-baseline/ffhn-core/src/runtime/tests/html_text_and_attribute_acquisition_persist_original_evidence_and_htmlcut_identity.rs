use super::support::*;

#[test]
fn html_text_and_attribute_acquisition_persist_original_evidence_and_htmlcut_identity() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(
        paths.target_dir().join("source.html"),
        "<main><span class=\"price\">1.00</span></main>",
    )
    .expect("HTML source");
    write_html_target(&paths, "html_text", ".price", None, "decimal", "");

    let report = run_once(&paths).expect("HTML text run");
    let observation = report.observation().expect("HTML text observation");
    assert_eq!(
        observation.acquisition_kind(),
        AcquisitionKind::HtmlPlainText
    );
    assert_eq!(observation.raw_selected(), "1.00");
    assert_eq!(observation.comparison_projection(), "1.00");
    assert_eq!(observation.canonical_value(), "1");
    assert_eq!(
        observation.htmlcut_semantics_version(),
        Some(htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION)
    );
    assert_eq!(observation.plan_digest_sha256().map(str::len), Some(64));
    assert_eq!(observation.htmlcut_candidate_count(), Some(1));
    assert!(observation.htmlcut_diagnostics().is_empty());
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("persisted HTML state"))
            .expect("state JSON");
    assert_eq!(
        state["accepted_observation"]["acquisition_kind"],
        "html_plain_text"
    );
    assert_eq!(
        state["accepted_observation"]["htmlcut_semantics_version"],
        htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION
    );

    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(
        paths.target_dir().join("source.html"),
        "<meta id=\"price\" content=\"12.00\"><time id=\"published\" datetime=\"2026-07-15T12:34:56+02:00\">ignored</time>",
    )
    .expect("HTML source");
    write_html_target(
        &paths,
        "html_attribute",
        "meta#price",
        Some("content"),
        "decimal",
        "",
    );
    let report = run_once(&paths).expect("meta content run");
    let observation = report.observation().expect("meta content observation");
    assert_eq!(
        observation.acquisition_kind(),
        AcquisitionKind::HtmlAttribute
    );
    assert_eq!(observation.raw_selected(), "12.00");
    assert_eq!(observation.comparison_projection(), "12.00");
    assert_eq!(observation.canonical_value(), "12");

    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(
        paths.target_dir().join("source.html"),
        "<time id=\"published\" datetime=\"2026-07-15T12:34:56+02:00\">ignored</time>",
    )
    .expect("HTML source");
    write_html_target(
        &paths,
        "html_attribute",
        "time#published",
        Some("datetime"),
        "datetime",
        "[type_params]\nformat = \"rfc3339\"\n",
    );
    let report = run_once(&paths).expect("time datetime run");
    let observation = report.observation().expect("time datetime observation");
    assert_eq!(observation.raw_selected(), "2026-07-15T12:34:56+02:00");
    assert_eq!(observation.canonical_value(), "2026-07-15T10:34:56Z");
}

use super::support::*;

#[test]
fn html_text_uses_dom_descendant_text_and_html_rendered_text_preserves_structural_syntax() {
    let (_temporary, plain_paths) = fixture_paths();
    fs::create_dir_all(plain_paths.target_dir()).expect("create plain-text target directory");
    fs::write(
        plain_paths.target_dir().join("source.html"),
        "<main><h1>Big <em>Title</em></h1></main>",
    )
    .expect("write heading source");
    write_html_target(&plain_paths, "html_text", "h1", None, "text", "");

    let plain_report = run_once(&plain_paths).expect("plain text run");
    let plain_observation = plain_report.observation().expect("plain text observation");
    assert_eq!(
        plain_observation.acquisition_kind(),
        AcquisitionKind::HtmlPlainText
    );
    assert_eq!(plain_observation.raw_selected(), "Big Title");
    assert_eq!(plain_observation.comparison_projection(), "Big Title");

    let (_temporary, rendered_paths) = fixture_paths();
    fs::create_dir_all(rendered_paths.target_dir()).expect("create rendered-text target directory");
    fs::write(
        rendered_paths.target_dir().join("source.html"),
        "<main><h1>Big <em>Title</em></h1></main>",
    )
    .expect("write heading source");
    write_html_target(
        &rendered_paths,
        "html_rendered_text",
        "h1",
        None,
        "text",
        "",
    );

    let rendered_report = run_once(&rendered_paths).expect("rendered text run");
    let rendered_observation = rendered_report
        .observation()
        .expect("rendered text observation");
    assert_eq!(
        rendered_observation.acquisition_kind(),
        AcquisitionKind::HtmlRenderedText
    );
    assert_eq!(rendered_observation.raw_selected(), "# Big Title");
    assert_eq!(rendered_observation.comparison_projection(), "# Big Title");
    assert_ne!(
        plain_report.contract_digest_sha256(),
        rendered_report.contract_digest_sha256(),
        "the declared projection is part of the measurement contract"
    );
}

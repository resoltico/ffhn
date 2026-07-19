use super::support::*;

#[test]
fn htmlcut_canonicalization_preflight_rejections_retain_evidence_for_the_full_lifecycle() {
    let (_temporary, paths) = fixture_paths();
    write_html_target(&paths, "html_text", "article", None, "integer", "");
    let target = fs::read_to_string(paths.target_file()).expect("HTML text target");
    let target = target.replace(
        "kind = \"css_selector\"\nselector = \"article\"",
        "kind = \"delimiter_pair\"\nstart = \"<article>\"\nend = \"</article>\"\nmode = \"literal\"\nboundary_retention = \"exclude_both\"",
    );
    fs::write(
        paths.target_file(),
        format!(
            "{target}\n[projection.selection.dom_canonicalization]\nignore_attributes = []\nstrip_whitespace_nodes = false\n"
        ),
    )
    .expect("non-CSS canonicalized target");
    assert_htmlcut_preflight_permanent_episode(&paths);

    let (_temporary, paths) = fixture_paths();
    write_html_target(
        &paths,
        "html_attribute",
        "meta#price",
        Some("content"),
        "decimal",
        "",
    );
    let target = fs::read_to_string(paths.target_file()).expect("HTML attribute target");
    fs::write(
        paths.target_file(),
        format!(
            "{target}\n[projection.selection.dom_canonicalization]\nignore_attributes = [\"content\"]\nstrip_whitespace_nodes = false\n"
        ),
    )
    .expect("canonicalized attribute target");
    assert_htmlcut_preflight_permanent_episode(&paths);
}

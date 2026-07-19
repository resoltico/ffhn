use super::support::*;

#[test]
fn text_measurements_preserve_configured_projection_identity() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "text", "", "/availability");
    let source_path = paths.target_dir().join("source.json");
    fs::write(&source_path, r#"{"availability":"\u00e9"}"#)
        .expect("write escaped text JSON fixture");
    let initialized = run_once(&paths).expect("initialize escaped text JSON measurement");
    let observation = initialized.observation().expect("escaped text observation");
    assert_eq!(observation.raw_selected(), r#""\u00e9""#);
    assert_eq!(observation.canonical_value(), "é");

    fs::write(&source_path, r#"{"availability":"é"}"#).expect("write literal text JSON fixture");
    let unchanged = run_once(&paths).expect("compare literal text JSON measurement");
    assert_eq!(unchanged.outcome(), RunOutcome::Unchanged);
    let observation = unchanged.observation().expect("literal text observation");
    assert_eq!(observation.raw_selected(), r#""é""#);
    assert_eq!(observation.canonical_value(), "é");

    let (_temporary, paths) = fixture_paths();
    let source_path = paths.target_dir().join("availability.html");
    fs::create_dir_all(paths.target_dir()).expect("create target directory");
    fs::write(&source_path, "<main id=\"availability\">In Stock</main>")
        .expect("write initial HTML fixture");
    fs::write(
        paths.target_file(),
        format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"{}\"\ndisplay_name = \"Availability\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"text\"\n\n[[conditions]]\ncondition_id = \"availability-changed\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"\n\n[target]\nkind = \"file\"\nfile_path = {:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"html_text\"\n\n[projection.selection.strategy]\nkind = \"css_selector\"\nselector = \"#availability\"\n\n[projection.selection.selection]\nmode = \"single\"\n\n[projection.selection.rendering]\nwhitespace = \"rendered\"\nrewrite_urls = false\n",
            paths.target_id(),
            source_path,
        ),
    )
    .expect("write text HTML target");

    let initialized = run_once(&paths).expect("initialize text HTML measurement");
    assert_eq!(initialized.outcome(), RunOutcome::Initialized);
    let observation = initialized.observation().expect("initial text observation");
    assert_eq!(
        observation.acquisition_kind(),
        AcquisitionKind::HtmlPlainText
    );
    assert_eq!(observation.raw_selected(), "In Stock");
    assert_eq!(observation.comparison_projection(), "In Stock");
    assert_eq!(observation.canonical_value(), "In Stock");

    fs::write(
        &source_path,
        "<main id=\"availability\">Out of Stock</main>",
    )
    .expect("write changed HTML fixture");
    let changed = run_once(&paths).expect("evaluate changed text HTML measurement");
    assert_eq!(changed.outcome(), RunOutcome::Changed);
    let condition = changed
        .policy_evaluation()
        .condition_results()
        .expect("evaluated conditions")
        .first()
        .expect("availability condition");
    assert_eq!(condition.condition_id(), "availability-changed");
    assert_eq!(condition.outcome(), ConditionOutcome::Satisfied);
    assert!(condition.triggered());
    assert_eq!(
        condition
            .reference()
            .and_then(|reference| reference.canonical_value()),
        Some("In Stock")
    );

    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create attribute target directory");
    fs::write(
        paths.target_dir().join("source.html"),
        "<meta id=\"availability\" content=\"In Stock\">",
    )
    .expect("write text attribute fixture");
    write_html_target(
        &paths,
        "html_attribute",
        "meta#availability",
        Some("content"),
        "text",
        "",
    );
    let attribute = run_once(&paths).expect("accept text HTML attribute");
    let observation = attribute.observation().expect("text attribute observation");
    assert_eq!(
        observation.acquisition_kind(),
        AcquisitionKind::HtmlAttribute
    );
    assert_eq!(observation.raw_selected(), "In Stock");
    assert_eq!(observation.comparison_projection(), "In Stock");
    assert_eq!(observation.canonical_value(), "In Stock");
}

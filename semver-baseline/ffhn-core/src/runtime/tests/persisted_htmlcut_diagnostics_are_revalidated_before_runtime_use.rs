use super::support::*;

#[test]
fn persisted_htmlcut_diagnostics_cannot_invent_an_incoherent_detail_after_acceptance() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("create HTML target directory");
    fs::write(paths.target_dir().join("source.html"), "<p>1</p><p>1</p>").expect("HTML source");
    write_html_target(&paths, "html_text", "p", None, "integer", "");
    let target = fs::read_to_string(paths.target_file()).expect("target TOML");
    fs::write(
        paths.target_file(),
        target.replace("mode = \"single\"", "mode = \"first\""),
    )
    .expect("first-match target TOML");

    let accepted = run_once(&paths).expect("accepted first-match HTML observation");
    assert_eq!(accepted.outcome(), RunOutcome::Initialized);
    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("state JSON"))
            .expect("state document");
    assert_eq!(
        state["accepted_observation"]["htmlcut_diagnostics"][0]["code"],
        "MULTIPLE_MATCHES"
    );
    state["accepted_observation"]["htmlcut_diagnostics"][0]["details"]["selected_index"] =
        serde_json::json!(2);
    fs::write(
        paths.state_file(),
        serde_json::to_vec(&state).expect("mutated state JSON"),
    )
    .expect("write invented persisted diagnostic");

    let rejected = run_once(&paths).expect("invalid persisted state produces a run report");
    assert_eq!(rejected.outcome(), RunOutcome::StateInvalid);
    assert!(!rejected.state_persisted());
}

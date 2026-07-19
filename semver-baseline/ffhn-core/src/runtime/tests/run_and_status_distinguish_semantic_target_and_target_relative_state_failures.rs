use super::support::*;

#[test]
fn run_and_status_distinguish_semantic_target_and_target_relative_state_failures() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    let target_text = fs::read_to_string(paths.target_file()).expect("target text");
    fs::write(
        paths.target_file(),
        target_text.replacen("max_bytes = 1024", "max_bytes = 0", 1),
    )
    .expect("invalid semantic target");
    assert_eq!(
        status(&paths).expect("invalid target status").kind(),
        StatusKind::InvalidConfig
    );

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":10}"#).expect("source value");
    run_once(&paths).expect("initial persisted state");
    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.state_file()).expect("state bytes"))
            .expect("state JSON");
    state["accepted_observation"]["declared_type"] = serde_json::json!("decimal");
    state["fixed_initial_baseline"]["declared_type"] = serde_json::json!("decimal");
    fs::write(
        paths.state_file(),
        serde_json::to_vec(&state).expect("mutated state JSON"),
    )
    .expect("mutated state");
    assert_eq!(
        run_once(&paths)
            .expect("target-relative invalid state")
            .outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&paths)
            .expect("target-relative invalid state status")
            .kind(),
        StatusKind::InvalidState
    );

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    let target = load_target(&paths).expect("target");
    let empty = StateDocument::new(
        TargetId::new(paths.target_id()).expect("target id"),
        target.contract_digest_sha256().expect("digest"),
    );
    fs::create_dir_all(paths.storage_root()).expect("owned storage");
    fs::write(
        paths.state_file(),
        serde_json::to_vec(&empty).expect("empty state JSON"),
    )
    .expect("empty state");
    assert_eq!(
        status(&paths).expect("empty persisted state status").kind(),
        StatusKind::Pending
    );
}

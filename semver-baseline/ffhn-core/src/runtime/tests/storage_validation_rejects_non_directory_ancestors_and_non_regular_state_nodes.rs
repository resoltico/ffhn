use super::support::*;

#[test]
fn storage_validation_rejects_non_directory_ancestors_and_non_regular_state_nodes() {
    let (_temporary, paths) = fixture_paths();
    fs::write(paths.target_dir(), "not a directory").expect("target-directory blocker");
    assert!(load_state(&paths).is_err());

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::create_dir_all(paths.storage_root()).expect("storage root");
    fs::create_dir(paths.state_file()).expect("state-directory blocker");
    assert_eq!(
        run_once(&paths).expect("state-directory report").outcome(),
        RunOutcome::StateInvalid
    );
}

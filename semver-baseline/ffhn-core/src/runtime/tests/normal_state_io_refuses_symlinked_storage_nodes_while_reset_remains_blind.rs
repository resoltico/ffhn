use super::support::*;

#[test]
fn normal_state_io_refuses_symlinked_storage_nodes_while_reset_remains_blind() {
    let (temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":7}"#).expect("source");
    let outside_root = temporary.path().join("outside-root");
    fs::create_dir_all(&outside_root).expect("outside root");
    let outside_state = outside_root.join("state.json");
    fs::write(&outside_state, "outside state").expect("outside state");
    std::os::unix::fs::symlink(&outside_root, paths.storage_root()).expect("storage symlink");

    assert_eq!(
        run_once(&paths)
            .expect("symlinked storage report")
            .outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&paths).expect("symlinked storage status").kind(),
        StatusKind::InvalidState
    );
    assert_eq!(
        fs::read_to_string(&outside_state).expect("outside state"),
        "outside state"
    );
    reset(&paths).expect("blind reset removes root link");
    assert_eq!(
        fs::read_to_string(&outside_state).expect("outside state survives"),
        "outside state"
    );

    run_once(&paths).expect("fresh owned storage");
    fs::remove_file(paths.state_file()).expect("remove owned state");
    let outside_file = temporary.path().join("outside-file");
    fs::write(&outside_file, "outside file").expect("outside file");
    std::os::unix::fs::symlink(&outside_file, paths.state_file()).expect("state symlink");

    assert_eq!(
        run_once(&paths).expect("symlinked state report").outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&paths).expect("symlinked state status").kind(),
        StatusKind::InvalidState
    );
    reset(&paths).expect("blind reset removes state-link root");
    assert_eq!(
        fs::read_to_string(&outside_file).expect("outside file survives"),
        "outside file"
    );
}

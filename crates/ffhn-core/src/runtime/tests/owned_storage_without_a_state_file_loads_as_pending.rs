use super::support::*;

#[test]
fn owned_storage_without_a_state_file_loads_as_pending() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.storage_root()).expect("create owned storage");
    assert!(
        load_state(&paths)
            .expect("missing owned state is pending")
            .is_none()
    );
}

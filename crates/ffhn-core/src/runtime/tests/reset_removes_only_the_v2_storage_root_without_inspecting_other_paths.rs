use super::support::*;

#[test]
fn reset_removes_only_the_v2_storage_root_without_inspecting_other_paths() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.storage_root()).expect("create v2 storage");
    fs::write(paths.state_file(), [0xff]).expect("write invalid v2 state");
    fs::write(paths.target_dir().join("operator-note.txt"), "keep").expect("write unrelated note");
    fs::create_dir_all(paths.target_dir().join("operator-data")).expect("create unrelated data");
    fs::write(paths.target_dir().join("operator-data/marker"), "keep")
        .expect("write unrelated marker");

    reset(&paths).expect("reset without artifact reads");
    assert!(!paths.storage_root().exists());
    assert!(paths.target_dir().join("operator-note.txt").exists());
    assert!(paths.target_dir().join("operator-data/marker").exists());
}

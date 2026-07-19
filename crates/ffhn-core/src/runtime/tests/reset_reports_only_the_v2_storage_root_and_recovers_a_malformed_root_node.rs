use super::support::*;

#[test]
fn reset_reports_only_the_v2_storage_root_and_recovers_a_malformed_root_node() {
    let (_temporary, paths) = fixture_paths();
    assert_eq!(
        serde_json::to_value(reset(&paths).expect("empty reset")).expect("empty reset JSON")["storage_cleared"],
        false
    );

    fs::create_dir_all(paths.target_dir()).expect("target directory");
    let unrelated_file = paths.target_dir().join("operator-note.txt");
    fs::write(&unrelated_file, "keep").expect("unrelated file");
    assert_eq!(
        serde_json::to_value(reset(&paths).expect("unrelated reset")).expect("reset JSON")["storage_cleared"],
        false
    );
    assert!(unrelated_file.exists());

    fs::write(paths.storage_root(), "malformed storage root").expect("write malformed root");
    assert_eq!(
        serde_json::to_value(reset(&paths).expect("malformed root reset")).expect("reset JSON")["storage_cleared"],
        true
    );
    assert!(!paths.storage_root().exists());
}

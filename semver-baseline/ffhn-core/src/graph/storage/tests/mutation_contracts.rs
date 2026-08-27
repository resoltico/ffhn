use std::fs;

use super::super::*;

#[test]
fn regular_file_primitives_preserve_exact_bytes_hashes_removal_and_directory_sync() {
    let (_temporary, paths, source_id) = super::fixture();
    let graph = TrustedGraphRoot::open(paths).expect("graph");
    let source = graph.open_source(source_id).expect("source");
    let storage = source.open_storage().expect("storage");
    let path = source.paths().storage_dir().join("probe.json");

    assert_eq!(durability::take_directory_sync_count(), 0);
    atomic_write_text(&storage.dir, "probe.json", "{\"value\":7}", &path).expect("atomic write");
    assert_eq!(
        fs::read(&path).expect("persisted bytes"),
        b"{\"value\":7}\n"
    );
    assert_eq!(durability::take_directory_sync_count(), 1);
    assert_eq!(
        read_regular_file_bytes(&storage.dir, "probe.json", &path, "probe")
            .expect("bytes")
            .expect("present"),
        b"{\"value\":7}\n"
    );
    assert_eq!(
        read_json_regular::<serde_json::Value>(&storage.dir, "probe.json", &path, "probe")
            .expect("JSON")
            .expect("present"),
        serde_json::json!({"value": 7})
    );
    assert_eq!(
        hash_regular_file(&storage.dir, "probe.json", &path, "probe")
            .expect("hash")
            .expect("present"),
        crate::stable_json::sha256_hex(b"{\"value\":7}\n")
    );

    remove_regular_file(&storage.dir, "probe.json", &path, "probe").expect("remove");
    assert!(!path.exists());
    assert_eq!(durability::take_directory_sync_count(), 1);
}

#[test]
fn directory_and_optional_entry_primitives_distinguish_every_node_shape() {
    let (_temporary, paths, source_id) = super::fixture();
    let graph = TrustedGraphRoot::open(paths).expect("graph");
    let source = graph.open_source(source_id).expect("source");
    let storage = source.open_storage().expect("storage");
    let child_path = source.paths().storage_dir().join("child");
    fs::create_dir(&child_path).expect("child directory");

    require_real_directory(&child_path).expect("real directory");
    let child = open_real_child(&storage.dir, "child", &child_path, "child").expect("child");
    child.create("marker").expect("usable capability");
    assert!(child_path.join("marker").is_file());
    for error in [
        read_json_regular::<serde_json::Value>(&storage.dir, "child", &child_path, "probe")
            .expect_err("directory is not JSON"),
        read_regular_file_bytes(&storage.dir, "child", &child_path, "probe")
            .expect_err("directory has no regular-file bytes"),
        hash_regular_file(&storage.dir, "child", &child_path, "probe")
            .expect_err("directory has no regular-file hash"),
        remove_regular_file(&storage.dir, "child", &child_path, "probe")
            .expect_err("directory cannot be removed as a file"),
    ] {
        assert!(
            error
                .to_string()
                .contains("probe must be a non-symlink regular file")
        );
    }
    assert!(
        open_optional_real_child(&storage.dir, "child", &child_path, "child")
            .expect("optional child")
            .is_some()
    );
    assert_eq!(
        optional_fs_entry(Ok::<_, std::io::Error>(7_u8), &child_path).expect("entry"),
        Some(7)
    );
    assert_eq!(
        optional_fs_entry::<u8>(
            Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
            &child_path,
        )
        .expect("absent"),
        None
    );

    let file_path = source.paths().storage_dir().join("file");
    fs::write(&file_path, "file").expect("file");
    assert!(
        require_real_directory(&file_path)
            .expect_err("file is not a graph directory")
            .to_string()
            .contains("trusted graph-root component must be a non-symlink directory")
    );
    assert!(
        open_real_child(&storage.dir, "file", &file_path, "probe")
            .expect_err("file is not a child directory")
            .to_string()
            .contains("probe must be a non-symlink directory")
    );
    assert!(
        open_optional_real_child(&storage.dir, "file", &file_path, "probe")
            .expect_err("file is not an optional child directory")
            .to_string()
            .contains("probe must be a non-symlink directory")
    );
}

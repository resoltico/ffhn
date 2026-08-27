use std::fs;

use super::super::*;

#[test]
fn manifest_child_and_regular_byte_helpers_preserve_success_and_absence() {
    let (_temporary, source, _identity) = super::ready_source();
    let storage = source.open_storage().expect("storage");
    let child_path = source.paths().storage_dir().join("child");
    fs::create_dir(&child_path).expect("child");
    let child = open_existing_child(&storage.dir, "child", &child_path).expect("child capability");
    child.create("marker").expect("usable child capability");
    assert!(child_path.join("marker").is_file());
    assert!(
        read_regular_bytes(&storage.dir, "child", &child_path)
            .expect_err("directory is not a manifest file")
            .to_string()
            .contains("must name a non-symlink regular file")
    );

    let exact_path = source.paths().storage_dir().join("exact.bin");
    fs::write(&exact_path, b"exact bytes").expect("exact file");
    assert_eq!(
        read_regular_bytes(&storage.dir, "exact.bin", &exact_path)
            .expect("bytes")
            .expect("present"),
        b"exact bytes"
    );
    let absent_path = source.paths().storage_dir().join("absent.bin");
    assert_eq!(
        read_regular_bytes(&storage.dir, "absent.bin", &absent_path).expect("absent"),
        None
    );

    let file_parent_path = source.paths().storage_dir().join("file-parent");
    fs::write(&file_parent_path, "not a directory").expect("file parent");
    assert!(
        open_existing_child(&storage.dir, "file-parent", &file_parent_path)
            .expect_err("regular file is not a manifest parent")
            .to_string()
            .contains("must be a non-symlink directory")
    );
}

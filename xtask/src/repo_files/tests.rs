use super::*;

#[cfg(unix)]
#[test]
fn maintained_rust_source_entries_recurse_and_skip_tests_and_non_rust_files() {
    let repo = tempfile::tempdir().expect("tempdir");
    let repo_root = repo.path();

    fs::create_dir_all(repo_root.join("crates/ffhn-core/src/nested"))
        .expect("create nested rust source dir");
    fs::create_dir_all(repo_root.join("crates/ffhn-core/src/tests")).expect("create tests dir");
    fs::create_dir_all(repo_root.join("xtask/src")).expect("create xtask src");

    fs::write(
        repo_root.join("crates/ffhn-core/src/lib.rs"),
        "pub fn root() {}\n",
    )
    .expect("write root rust source");
    fs::write(
        repo_root.join("crates/ffhn-core/src/nested/keep.rs"),
        "pub fn nested() {}\n",
    )
    .expect("write nested rust source");
    fs::write(
        repo_root.join("crates/ffhn-core/src/tests/helper.rs"),
        "pub fn ignored() {}\n",
    )
    .expect("write ignored tests helper");
    fs::write(repo_root.join("crates/ffhn-core/src/tests.rs"), "ignored\n")
        .expect("write ignored tests.rs");
    fs::write(repo_root.join("xtask/src/note.txt"), "ignore").expect("write ignored txt");

    let entries =
        maintained_rust_source_entries(repo_root).expect("maintained rust source entries");
    let relative_paths = entries
        .into_iter()
        .map(|(_, relative_path)| relative_path)
        .collect::<Vec<_>>();

    assert_eq!(
        relative_paths,
        vec![
            "crates/ffhn-core/src/lib.rs".to_owned(),
            "crates/ffhn-core/src/nested/keep.rs".to_owned(),
        ]
    );
}

#[test]
fn source_shape_inventory_includes_unit_integration_and_fuzz_rust_files() {
    let repo = tempfile::tempdir().expect("tempdir");
    let repo_root = repo.path();

    for relative_path in [
        "crates/ffhn-core/src/tests/unit.rs",
        "crates/ffhn-cli/tests/cli.rs",
        "xtask/tests/cli.rs",
        "fuzz/fuzz_targets/target_documents.rs",
    ] {
        let path = repo_root.join(relative_path);
        fs::create_dir_all(path.parent().expect("parent")).expect("create source directory");
        fs::write(path, "fn scenario() {}\n").expect("write source");
    }
    fs::write(
        repo_root.join("fuzz/fuzz_targets/ignored.txt"),
        "not Rust source\n",
    )
    .expect("write ignored file");

    let paths = maintained_rust_source_entries_including_tests(repo_root)
        .expect("source-shape inventory")
        .into_iter()
        .map(|(_, relative_path)| relative_path)
        .collect::<Vec<_>>();

    assert_eq!(
        paths,
        vec![
            "crates/ffhn-cli/tests/cli.rs".to_owned(),
            "crates/ffhn-core/src/tests/unit.rs".to_owned(),
            "fuzz/fuzz_targets/target_documents.rs".to_owned(),
            "xtask/tests/cli.rs".to_owned(),
        ]
    );
}

#[test]
fn maintained_repo_owned_paths_skip_watchlist_runtime_artifacts_and_mac_metadata() {
    let repo = tempfile::tempdir().expect("tempdir");
    let repo_root = repo.path();

    fs::write(repo_root.join("AGENTS.md"), "# agents\n").expect("write AGENTS.md");
    fs::create_dir_all(repo_root.join("examples/file-example")).expect("create examples dir");
    fs::create_dir_all(repo_root.join("watchlist/demo/.ffhn")).expect("create state dir");
    fs::create_dir_all(repo_root.join("watchlist/.ffhn-locks")).expect("create lock dir");

    fs::write(repo_root.join("examples/.DS_Store"), "ignore").expect("write .DS_Store");
    fs::write(repo_root.join("examples/._release-notes.html"), "ignore")
        .expect("write AppleDouble metadata");
    fs::write(
        repo_root.join("examples/file-example/release-notes.html"),
        "<main>demo</main>\n",
    )
    .expect("write maintained example");
    fs::write(
        repo_root.join("watchlist/demo/target.toml"),
        "schema_name = \"ffhn.target\"\n",
    )
    .expect("write starter target");
    fs::write(repo_root.join("watchlist/demo/.ffhn/state.json"), "{}\n")
        .expect("write runtime state");
    fs::write(repo_root.join("watchlist/.ffhn-locks/demo.lock"), "lock\n").expect("write run lock");

    let paths = maintained_repo_owned_paths(repo_root).expect("maintained repo-owned paths");

    assert!(paths.contains(&repo_root.join("AGENTS.md")));
    assert!(paths.contains(&repo_root.join("examples/file-example/release-notes.html")));
    assert!(paths.contains(&repo_root.join("watchlist/demo/target.toml")));
    assert!(!paths.contains(&repo_root.join("examples/.DS_Store")));
    assert!(!paths.contains(&repo_root.join("examples/._release-notes.html")));
    assert!(!paths.contains(&repo_root.join("watchlist/demo/.ffhn/state.json")));
    assert!(!paths.contains(&repo_root.join("watchlist/.ffhn-locks/demo.lock")));
}

#[cfg(unix)]
#[test]
fn maintained_repo_owned_paths_skip_non_file_non_directory_entries() {
    use std::os::unix::fs::symlink;

    let repo = tempfile::tempdir().expect("tempdir");
    let repo_root = repo.path();

    fs::create_dir_all(repo_root.join("examples/file-example")).expect("create examples dir");
    symlink(
        repo_root.join("examples/missing-release-notes.html"),
        repo_root.join("examples/file-example/broken-link.html"),
    )
    .expect("create broken symlink");

    let paths = maintained_repo_owned_paths(repo_root).expect("maintained repo-owned paths");

    assert!(!paths.contains(&repo_root.join("examples/file-example/broken-link.html")));
}

#[test]
fn regular_file_collection_skips_retired_runtime_roots_and_metadata_files() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let root = temporary.path();
    fs::create_dir_all(root.join("keep/nested")).expect("kept directory");
    for directory in ["target", "dist", "lock", "snapshots"] {
        fs::create_dir_all(root.join(directory)).expect("retired runtime directory");
        fs::write(root.join(directory).join("ignored.txt"), "ignored\n").expect("ignored file");
    }
    fs::write(root.join("keep/nested/kept.txt"), "kept\n").expect("kept file");
    fs::write(root.join(".DS_Store"), "ignored\n").expect("metadata");
    fs::write(root.join("._kept.txt"), "ignored\n").expect("AppleDouble metadata");

    let mut paths = Vec::new();
    collect_regular_files(root, &mut paths).expect("collect regular files");
    assert_eq!(paths, vec![root.join("keep/nested/kept.txt")]);
    assert!(collect_regular_files(&root.join("missing"), &mut paths).is_err());
}

#[test]
fn repository_inventory_ignores_missing_roots_and_incomplete_watchlist_entries() {
    let repo = tempfile::tempdir().expect("temporary directory");
    let repo_root = repo.path();

    assert!(
        public_markdown_paths(repo_root)
            .expect("empty public markdown")
            .is_empty()
    );

    fs::create_dir_all(repo_root.join("watchlist/empty")).expect("watchlist directory");
    fs::write(repo_root.join("watchlist/not-a-target"), "not a directory").expect("watchlist file");
    assert!(
        watchlist_target_config_paths(repo_root)
            .expect("incomplete watchlist")
            .is_empty()
    );
}

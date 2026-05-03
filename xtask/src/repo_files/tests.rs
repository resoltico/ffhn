use super::*;

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
fn maintained_repo_owned_paths_skip_watchlist_runtime_artifacts_and_mac_metadata() {
    let repo = tempfile::tempdir().expect("tempdir");
    let repo_root = repo.path();

    fs::write(repo_root.join("AGENTS.md"), "# agents\n").expect("write AGENTS.md");
    fs::create_dir_all(repo_root.join("examples/file-example")).expect("create examples dir");
    fs::create_dir_all(repo_root.join("watchlist/demo/lock")).expect("create lock dir");
    fs::create_dir_all(repo_root.join("watchlist/demo/snapshots/current"))
        .expect("create snapshots dir");

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
    fs::write(repo_root.join("watchlist/demo/state.json"), "{}\n").expect("write runtime state");
    fs::write(repo_root.join("watchlist/demo/last_run.json"), "{}\n")
        .expect("write runtime last run");
    fs::write(repo_root.join("watchlist/demo/lock/run.lock"), "lock\n").expect("write run lock");
    fs::write(
        repo_root.join("watchlist/demo/snapshots/current/outer.html"),
        "<main>runtime</main>\n",
    )
    .expect("write runtime snapshot");

    let paths = maintained_repo_owned_paths(repo_root).expect("maintained repo-owned paths");

    assert!(paths.contains(&repo_root.join("AGENTS.md")));
    assert!(paths.contains(&repo_root.join("examples/file-example/release-notes.html")));
    assert!(paths.contains(&repo_root.join("watchlist/demo/target.toml")));
    assert!(!paths.contains(&repo_root.join("examples/.DS_Store")));
    assert!(!paths.contains(&repo_root.join("examples/._release-notes.html")));
    assert!(!paths.contains(&repo_root.join("watchlist/demo/state.json")));
    assert!(!paths.contains(&repo_root.join("watchlist/demo/last_run.json")));
    assert!(!paths.contains(&repo_root.join("watchlist/demo/lock/run.lock")));
    assert!(!paths.contains(&repo_root.join("watchlist/demo/snapshots/current/outer.html")));
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

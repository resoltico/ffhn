use super::*;

#[test]
fn workspace_version_from_manifest_extracts_workspace_package_version() {
    let version = workspace_version_from_manifest(
        "[workspace.package]\nversion = \"2.0.0\"\nedition = \"2024\"\n",
    )
    .expect("workspace version");

    assert_eq!(version, "2.0.0");
}

#[test]
fn workspace_version_from_manifest_requires_a_version_line() {
    let error = workspace_version_from_manifest("[workspace.package]\nedition = \"2024\"\n")
        .expect_err("missing version should fail");

    assert_eq!(
        error.to_string(),
        "workspace version not found in Cargo.toml"
    );
}

#[test]
fn workspace_version_from_manifest_ignores_dependency_version_tables() {
    let manifest = r#"
[workspace.dependencies.htmlcut-core]
version = "2.0.0"

[workspace.package]
version = "2.0.0"
"#;

    let version = workspace_version_from_manifest(manifest).expect("workspace version");

    assert_eq!(version, "2.0.0");
}

#[test]
fn refresh_semver_baseline_uses_the_requested_git_ref_instead_of_the_worktree() {
    let repo_root = tempdir().expect("tempdir");
    write_semver_fixture(
        repo_root.path(),
        "2.0.0",
        "pub const RELEASE_LINE: &str = \"tagged\";\n",
    );
    run_git(repo_root.path(), &["init", "-q"]);
    run_git(repo_root.path(), &["config", "user.name", "FFHN Tests"]);
    run_git(
        repo_root.path(),
        &["config", "user.email", "ffhn@example.invalid"],
    );
    run_git(repo_root.path(), &["add", "Cargo.toml", "crates/ffhn-core"]);
    run_git(
        repo_root.path(),
        &["commit", "-qm", "seed published snapshot"],
    );
    run_git(repo_root.path(), &["tag", "v2.0.0"]);

    write_semver_fixture(
        repo_root.path(),
        "9.9.9",
        "pub const RELEASE_LINE: &str = \"worktree\";\n",
    );

    refresh_semver_baseline(repo_root.path(), "v2.0.0").expect("refresh baseline");

    let baseline_manifest = fs::read_to_string(
        repo_root
            .path()
            .join("semver-baseline")
            .join("ffhn-core")
            .join("Cargo.toml"),
    )
    .expect("read baseline manifest");
    let baseline_lib = fs::read_to_string(
        repo_root
            .path()
            .join("semver-baseline")
            .join("ffhn-core")
            .join("src")
            .join("lib.rs"),
    )
    .expect("read baseline lib");

    assert!(baseline_manifest.contains("version = \"2.0.0\""));
    assert!(!baseline_manifest.contains("version = \"9.9.9\""));
    assert_eq!(baseline_lib, "pub const RELEASE_LINE: &str = \"tagged\";\n");
}

#[test]
fn refresh_semver_baseline_replaces_existing_baseline_artifacts() {
    let repo_root = tempdir().expect("tempdir");
    write_semver_fixture(
        repo_root.path(),
        "2.0.0",
        "pub const RELEASE_LINE: &str = \"published\";\n",
    );
    run_git(repo_root.path(), &["init", "-q"]);
    run_git(repo_root.path(), &["config", "user.name", "FFHN Tests"]);
    run_git(
        repo_root.path(),
        &["config", "user.email", "ffhn@example.invalid"],
    );
    run_git(repo_root.path(), &["add", "Cargo.toml", "crates/ffhn-core"]);
    run_git(
        repo_root.path(),
        &["commit", "-qm", "seed published snapshot"],
    );
    run_git(repo_root.path(), &["tag", "v2.0.0"]);

    let baseline_parent = repo_root.path().join("semver-baseline");
    let baseline_dir = baseline_parent.join("ffhn-core");
    fs::create_dir_all(baseline_dir.join("src")).expect("create stale baseline dir");
    fs::write(baseline_dir.join("src").join("lib.rs"), "stale\n").expect("write stale baseline");
    fs::create_dir_all(&baseline_parent).expect("create baseline parent");
    fs::write(baseline_parent.join("ffhn-core.tar.gz"), "stale archive")
        .expect("write stale archive");

    refresh_semver_baseline(repo_root.path(), "v2.0.0").expect("refresh baseline");

    let baseline_lib = fs::read_to_string(baseline_dir.join("src").join("lib.rs"))
        .expect("read refreshed baseline");
    assert_eq!(
        baseline_lib,
        "pub const RELEASE_LINE: &str = \"published\";\n"
    );
    assert!(!baseline_parent.join("ffhn-core.tar.gz").exists());
}

#[test]
fn refresh_semver_baseline_reports_missing_git_refs() {
    let repo_root = tempdir().expect("tempdir");
    write_semver_fixture(
        repo_root.path(),
        "2.0.0",
        "pub const RELEASE_LINE: &str = \"published\";\n",
    );
    run_git(repo_root.path(), &["init", "-q"]);
    run_git(repo_root.path(), &["config", "user.name", "FFHN Tests"]);
    run_git(
        repo_root.path(),
        &["config", "user.email", "ffhn@example.invalid"],
    );
    run_git(repo_root.path(), &["add", "Cargo.toml", "crates/ffhn-core"]);
    run_git(
        repo_root.path(),
        &["commit", "-qm", "seed published snapshot"],
    );

    let error = refresh_semver_baseline(repo_root.path(), "v9.9.9").expect_err("missing ref");
    assert!(
        error
            .to_string()
            .contains("failed to read Cargo.toml from git ref v9.9.9")
    );
}

#[test]
fn refresh_semver_baseline_rejects_non_utf8_workspace_manifests_from_git() {
    let repo_root = tempdir().expect("tempdir");
    fs::create_dir_all(
        repo_root
            .path()
            .join("crates")
            .join("ffhn-core")
            .join("src"),
    )
    .expect("create ffhn-core src");
    fs::write(
        repo_root.path().join("Cargo.toml"),
        b"[workspace.package]\nversion = \"2.0.0\"\n\xff\n",
    )
    .expect("write invalid Cargo.toml");
    fs::write(
        repo_root
            .path()
            .join("crates")
            .join("ffhn-core")
            .join("Cargo.toml"),
        "[package]\nname = \"ffhn-core\"\nversion.workspace = true\nedition = \"2024\"\n",
    )
    .expect("write ffhn-core manifest");
    fs::write(
        repo_root
            .path()
            .join("crates")
            .join("ffhn-core")
            .join("src")
            .join("lib.rs"),
        "pub const RELEASE_LINE: &str = \"published\";\n",
    )
    .expect("write ffhn-core lib");
    run_git(repo_root.path(), &["init", "-q"]);
    run_git(repo_root.path(), &["config", "user.name", "FFHN Tests"]);
    run_git(
        repo_root.path(),
        &["config", "user.email", "ffhn@example.invalid"],
    );
    run_git(repo_root.path(), &["add", "Cargo.toml", "crates/ffhn-core"]);
    run_git(
        repo_root.path(),
        &["commit", "-qm", "seed invalid manifest"],
    );
    run_git(repo_root.path(), &["tag", "v2.0.0"]);

    let error = refresh_semver_baseline(repo_root.path(), "v2.0.0")
        .expect_err("non-UTF-8 manifest should fail");
    assert!(
        error
            .to_string()
            .contains("git returned non-UTF-8 contents for Cargo.toml")
    );
}

#[test]
fn workspace_version_reads_from_repo_manifest() {
    let repo_root = tempdir().expect("tempdir");
    fs::write(
        repo_root.path().join("Cargo.toml"),
        "[workspace.package]\nversion = \"9.9.9\"\n",
    )
    .expect("write Cargo.toml");

    let version = workspace_version(repo_root.path()).expect("workspace version");

    assert_eq!(version, "9.9.9");
}

#[test]
fn semver_release_type_uses_major_until_the_current_version_has_a_release_tag() {
    let repo_root = tempdir().expect("tempdir");
    fs::write(
        repo_root.path().join("Cargo.toml"),
        "[workspace.package]\nversion = \"2.0.0\"\n",
    )
    .expect("write Cargo.toml");

    assert_eq!(semver_release_type_from_git_tag(false), "major");
    assert_eq!(semver_release_type_from_git_tag(true), "minor");
    assert!(!release_tag_exists(repo_root.path(), "2.0.0").expect("missing git repo"));
    assert_eq!(
        semver_release_type(repo_root.path()).expect("release type without tag"),
        "major"
    );

    let status = Command::new("git")
        .arg("-C")
        .arg(repo_root.path())
        .arg("init")
        .arg("-q")
        .status()
        .expect("git init");
    assert!(status.success());

    fs::write(repo_root.path().join("README.md"), "# ffhn\n").expect("write README");

    let status = Command::new("git")
        .arg("-C")
        .arg(repo_root.path())
        .args([
            "-c",
            "user.name=FFHN",
            "-c",
            "user.email=ffhn@example.com",
            "add",
            "README.md",
        ])
        .status()
        .expect("git add");
    assert!(status.success());

    let status = Command::new("git")
        .arg("-C")
        .arg(repo_root.path())
        .args([
            "-c",
            "user.name=FFHN",
            "-c",
            "user.email=ffhn@example.com",
            "commit",
            "-q",
            "-m",
            "init",
        ])
        .status()
        .expect("git commit");
    assert!(status.success());

    let status = Command::new("git")
        .arg("-C")
        .arg(repo_root.path())
        .arg("tag")
        .arg("-f")
        .arg("v2.0.0")
        .status()
        .expect("git tag");
    assert!(status.success());

    assert!(release_tag_exists(repo_root.path(), "2.0.0").expect("release tag"));
    assert!(!release_tag_exists(repo_root.path(), "2.0.1").expect("other tag"));
    assert_eq!(
        semver_release_type(repo_root.path()).expect("release type with tag"),
        "minor"
    );
}

#[test]
fn with_workspace_stub_appends_once() {
    let workspace_manifest = r#"
[workspace]
members = ["crates/ffhn-core"]

[workspace.package]
version = "2.0.0"
edition = "2024"

[workspace.dependencies]
serde = "1.0.228"

[workspace.lints.rust]
unsafe_code = "warn"
"#;
    let updated = with_workspace_stub("[package]\nname = \"ffhn-core\"\n", workspace_manifest)
        .expect("workspace stub");
    let unchanged = with_workspace_stub(
        "[package]\nname = \"ffhn-core\"\n\n[workspace]\n",
        workspace_manifest,
    )
    .expect("unchanged workspace stub");

    assert!(updated.contains("[workspace.package]"));
    assert!(updated.contains("version = \"2.0.0\""));
    assert!(updated.contains("edition = \"2024\""));
    assert!(updated.contains("[workspace.dependencies]"));
    assert!(updated.contains("serde = \"1.0.228\""));
    assert!(updated.contains("[workspace.lints.rust]"));
    assert!(updated.contains("unsafe_code = \"warn\""));
    assert_eq!(
        unchanged,
        "[package]\nname = \"ffhn-core\"\n\n[workspace]\n"
    );
}

#[test]
fn with_workspace_stub_skips_missing_workspace_inheritance_sections() {
    let workspace_manifest = r#"
[workspace]
members = ["crates/ffhn-core"]

[workspace.package]
version = "2.0.0"
edition = "2024"
"#;

    let updated = with_workspace_stub("[package]\nname = \"ffhn-core\"\n", workspace_manifest)
        .expect("workspace stub");

    assert!(updated.contains("[workspace.package]"));
    assert!(!updated.contains("[workspace.dependencies]"));
    assert!(!updated.contains("[workspace.lints"));
}

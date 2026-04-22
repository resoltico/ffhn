use super::*;

#[test]
fn public_target_examples_validate_against_the_current_target_contract() {
    let repo_root = repo_root();
    let workspace_version = workspace_version(&repo_root).expect("workspace version");
    let paths = public_target_example_paths(&repo_root).expect("target examples");

    assert!(!paths.is_empty(), "expected checked-in target examples");

    for path in paths {
        let document = fs::read_to_string(&path).expect("read example");
        let target: TargetDocument = toml::from_str(&document).expect("parse target document");
        assert_public_target_example_contract(&path, &target, &workspace_version);
    }
}

#[test]
fn public_target_example_helper_covers_non_watchlist_file_targets() {
    let workspace_version = "2.1.0";
    let path = Path::new("/tmp/examples/release_notes.toml");
    let document = r#"
schema_name = "ffhn.target"
schema_version = 1
target_id = "release_notes"
display_name = "Release Notes"
enabled = true

[target]
kind = "file"
file_path = "/tmp/release-notes.html"

[fetch]
engine = "file"
follow_redirects = false
max_bytes = 2000000

[selection]
kind = "css_selector"
selector = "main"
match = "single"
output = "outer_html"
whitespace = "normalize"
rewrite_urls = false

[compare]
basis = "canonical_text_sha256"
canonicalization = []
"#;
    let target: TargetDocument = toml::from_str(document).expect("parse file-target example");
    assert_public_target_example_contract(path, &target, workspace_version);
}

#[test]
fn file_target_example_materializer_emits_a_valid_target_document() {
    let repo_root = repo_root();
    let materializer =
        repo_root.join("examples/file-target-with-notifications/materialize-target.sh");
    let temp = tempfile::tempdir().expect("tempdir");
    let target_file = temp.path().join("release_notes/target.toml");

    let output = Command::new("sh")
        .arg(&materializer)
        .arg(&target_file)
        .output()
        .expect("run file-target materializer");

    assert!(output.status.success());

    let document = fs::read_to_string(&target_file).expect("read materialized target");
    let target: TargetDocument = toml::from_str(&document).expect("parse materialized target");
    target.validate().expect("validate materialized target");
    assert_eq!(target.target_id, "release_notes");
    let file_path = target
        .target
        .file_path
        .as_deref()
        .expect("materialized file path");
    let expected_path =
        repo_root.join("examples/file-target-with-notifications/release-notes.html");
    assert!(Path::new(file_path).is_absolute(), "{}", file_path);
    assert!(Path::new(file_path).is_file(), "{}", file_path);
    assert_eq!(Path::new(file_path), expected_path.as_path());
}

#[test]
fn file_target_example_materializer_requires_one_destination_argument() {
    let repo_root = repo_root();
    let materializer =
        repo_root.join("examples/file-target-with-notifications/materialize-target.sh");

    let output = Command::new("sh")
        .arg(&materializer)
        .output()
        .expect("run file-target materializer without args");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
}

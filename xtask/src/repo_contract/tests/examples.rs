use super::*;
use std::process::Command;

fn available_powershell_program() -> Option<&'static str> {
    ["pwsh", "powershell", "powershell.exe"]
        .into_iter()
        .find(|program| {
            Command::new(program)
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-Command",
                    "$PSVersionTable.PSVersion",
                ])
                .output()
                .is_ok_and(|output| output.status.success())
        })
}

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
fn markdown_embedded_target_examples_validate_against_the_current_target_contract() {
    let repo_root = repo_root();
    let workspace_version = workspace_version(&repo_root).expect("workspace version");
    let mut seen_embedded_target = false;

    for path in public_markdown_paths(&repo_root).expect("markdown paths") {
        let document = fs::read_to_string(&path).expect("read markdown");
        for (index, fence) in fenced_code_blocks(&document).into_iter().enumerate() {
            if fence.language.as_deref() != Some("toml") {
                continue;
            }
            let Ok(parsed) = toml::from_str::<toml::Value>(&fence.body) else {
                continue;
            };
            if parsed.get("schema_name").and_then(toml::Value::as_str) != Some("ffhn.target") {
                continue;
            }

            let target: TargetDocument = toml::from_str(&fence.body).unwrap_or_else(|error| {
                panic!(
                    "{} fenced TOML block #{} failed to parse as TargetDocument: {error}",
                    path.display(),
                    index + 1
                )
            });
            assert_public_target_example_contract(&path, &target, &workspace_version);
            seen_embedded_target = true;
        }
    }

    assert!(
        seen_embedded_target,
        "expected at least one public Markdown target example"
    );
}

#[cfg(unix)]
#[test]
fn public_target_example_helper_covers_non_watchlist_file_targets() {
    let workspace_version = workspace_version(&repo_root()).expect("workspace version");
    let path = Path::new("/tmp/examples/release_notes.toml");
    let document = r#"
schema_name = "ffhn.target"
schema_version = 3
target_id = "release_notes"
display_name = "Release Notes"
enabled = true

[target]
kind = "file"
file_path = "/tmp/release-notes.html"

[fetch]
engine = "file"
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
    assert_public_target_example_contract(path, &target, &workspace_version);
}

#[cfg(unix)]
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
    assert_eq!(target.target_id(), "release_notes");
    let file_path = target.file_path().expect("materialized file path");
    let expected_path =
        repo_root.join("examples/file-target-with-notifications/release-notes.html");
    assert!(Path::new(file_path).is_absolute(), "{}", file_path);
    assert!(Path::new(file_path).is_file(), "{}", file_path);
    assert_eq!(Path::new(file_path), expected_path.as_path());
    let hook = target
        .notifications()
        .next()
        .expect("materialized notification hook");
    assert_eq!(
        hook.args().last().expect("hook log path"),
        target_file
            .parent()
            .expect("target dir")
            .join("ffhn-release-notes-report.jsonl")
            .to_str()
            .expect("utf8 log path")
    );
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

#[test]
fn file_target_example_powershell_materializer_emits_a_valid_target_document() {
    let Some(program) = available_powershell_program() else {
        return;
    };

    let repo_root = repo_root();
    let materializer =
        repo_root.join("examples/file-target-with-notifications/materialize-target.ps1");
    let temp = tempfile::tempdir().expect("tempdir");
    let target_file = temp.path().join("release_notes/target.toml");

    let output = Command::new(program)
        .args(["-NoLogo", "-NoProfile", "-File"])
        .arg(&materializer)
        .arg(&target_file)
        .output()
        .expect("run powershell file-target materializer");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let document = fs::read_to_string(&target_file).expect("read materialized target");
    let target: TargetDocument = toml::from_str(&document).expect("parse materialized target");
    target.validate().expect("validate materialized target");
    assert_eq!(target.target_id(), "release_notes");
    let file_path = target.file_path().expect("materialized file path");
    let expected_path =
        repo_root.join("examples/file-target-with-notifications/release-notes.html");
    assert!(Path::new(file_path).is_absolute(), "{}", file_path);
    assert!(Path::new(file_path).is_file(), "{}", file_path);
    assert_eq!(Path::new(file_path), expected_path.as_path());
    let hook = target
        .notifications()
        .next()
        .expect("materialized notification hook");
    assert_eq!(
        hook.args().last().expect("hook log path"),
        target_file
            .parent()
            .expect("target dir")
            .join("ffhn-release-notes-report.jsonl")
            .to_str()
            .expect("utf8 log path")
    );
}

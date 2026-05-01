use super::*;

use serde_json::Value;

#[test]
fn maintained_release_shell_entrypoints_are_self_describing() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");

    for (script_name, expected_fragment) in [
        (
            "build-release-source-archives.sh",
            "Build the maintained FFHN source archives",
        ),
        (
            "build-release-artifact.sh",
            "Build one maintained standalone FFHN release artifact",
        ),
        (
            "build-release-checksums.sh",
            "Write the canonical SHA-256 checksum manifest",
        ),
        (
            "publish-github-release.sh",
            "Publish or converge the GitHub release object",
        ),
        (
            "release-targets.sh",
            "Inspect the canonical FFHN standalone release-target registry",
        ),
        (
            "smoke-release-artifact.sh",
            "Extract one maintained ./dist release archive",
        ),
        (
            "verify-github-release.sh",
            "Verify the published GitHub release object",
        ),
        (
            "workspace-package-field.sh",
            "Print one string field from [workspace.package]",
        ),
        (
            "workspace-version.sh",
            "Print the [workspace.package] version",
        ),
        ("qa-gate.sh", "Run FFHN's maintained local quality gate"),
    ] {
        let output = bash_command()
            .arg(release_script_argument(repo_root, script_name))
            .arg("--help")
            .current_dir(repo_root)
            .output()
            .expect("run script --help");

        assert!(
            output.status.success(),
            "{script_name} --help failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage:"));
        assert!(
            stdout.contains(expected_fragment),
            "{script_name} help missing expected fragment {expected_fragment:?}:\n{stdout}",
        );
    }
}

#[test]
fn root_check_entrypoint_is_self_describing() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let output = bash_command()
        .arg(repo_root.join("check.sh"))
        .arg("--help")
        .current_dir(repo_root)
        .output()
        .expect("run check.sh --help");

    assert!(
        output.status.success(),
        "check.sh --help failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: ./check.sh"));
    assert!(stdout.contains("Run FFHN's maintained local quality gate"));
}

#[test]
fn release_targets_cli_prints_canonical_registry_views() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");

    let triples_output = bash_command()
        .arg(release_script_argument(repo_root, "release-targets.sh"))
        .arg("triples")
        .current_dir(repo_root)
        .output()
        .expect("run release-targets triples");
    assert!(triples_output.status.success());
    let triples = String::from_utf8(triples_output.stdout).expect("utf8 triples");
    assert!(triples.lines().any(|line| line == "aarch64-apple-darwin"));
    assert!(triples.lines().any(|line| line == "x86_64-pc-windows-msvc"));

    let matrix_output = bash_command()
        .arg(release_script_argument(repo_root, "release-targets.sh"))
        .arg("matrix-json")
        .current_dir(repo_root)
        .output()
        .expect("run release-targets matrix-json");
    assert!(matrix_output.status.success());
    let matrix: Value = serde_json::from_slice(&matrix_output.stdout).expect("parse matrix json");
    let include = matrix["include"].as_array().expect("matrix include");
    assert!(
        include
            .iter()
            .any(|entry| entry["target_triple"] == "aarch64-apple-darwin")
    );
    assert!(
        include
            .iter()
            .any(|entry| entry["target_triple"] == "x86_64-unknown-linux-musl")
    );

    let assets_output = bash_command()
        .arg(release_script_argument(repo_root, "release-targets.sh"))
        .args(["assets", "--version", "9.9.9"])
        .current_dir(repo_root)
        .output()
        .expect("run release-targets assets");
    assert!(assets_output.status.success());
    let assets = String::from_utf8(assets_output.stdout).expect("utf8 assets");
    assert!(assets.lines().any(|line| line == "ffhn-source-9.9.9.zip"));
    assert!(
        assets
            .lines()
            .any(|line| line == "ffhn-9.9.9-x86_64-pc-windows-msvc.zip")
    );
    assert!(
        assets
            .lines()
            .any(|line| line == "ffhn-9.9.9-checksums.txt")
    );
}

#[cfg(unix)]
#[test]
fn release_targets_script_is_shipped_as_an_executable_entrypoint() {
    use std::os::unix::fs::PermissionsExt;

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let metadata = fs::metadata(repo_root.join("scripts").join("release-targets.sh"))
        .expect("release-targets metadata");

    assert_ne!(
        metadata.permissions().mode() & 0o111,
        0,
        "release-targets.sh should be directly runnable for local inspection",
    );
}

use assert_cmd::Command;

#[test]
fn xtask_binary_routes_through_main_and_reports_missing_git_refs() {
    let output = Command::cargo_bin("xtask")
        .expect("xtask binary")
        .args([
            "refresh-semver-baseline",
            "--git-ref",
            "definitely-missing-ref",
        ])
        .output()
        .expect("run xtask");

    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to read Cargo.toml from git ref definitely-missing-ref"));
}

#[test]
fn xtask_binary_help_exposes_the_targeted_semver_lane() {
    let output = Command::cargo_bin("xtask")
        .expect("xtask binary")
        .arg("--help")
        .output()
        .expect("run xtask --help");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Rust-native repository maintenance tasks for FFHN."));
    assert!(stdout.contains("audit"));
    assert!(stdout.contains("cargo xtask audit"));
    assert!(stdout.contains("semver-check"));
    assert!(stdout.contains("cargo xtask semver-check"));
}

#[test]
fn xtask_binary_audit_help_describes_the_retrying_lane() {
    let output = Command::cargo_bin("xtask")
        .expect("xtask binary")
        .args(["audit", "--help"])
        .output()
        .expect("run xtask audit --help");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Run the maintained RustSec audit lane"));
    assert!(stdout.contains("bounded transient advisory-database fetch retry policy"));
    assert!(stdout.contains("--file <LOCKFILE>"));
}

#[test]
fn xtask_binary_semver_help_describes_the_maintained_lane() {
    let output = Command::cargo_bin("xtask")
        .expect("xtask binary")
        .args(["semver-check", "--help"])
        .output()
        .expect("run xtask semver-check --help");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Run only the maintained ffhn-core semver gate."));
    assert!(stdout.contains("same baseline and release-type policy"));
}

#[test]
fn xtask_binary_refresh_semver_baseline_help_describes_git_ref_input() {
    let output = Command::cargo_bin("xtask")
        .expect("xtask binary")
        .args(["refresh-semver-baseline", "--help"])
        .output()
        .expect("run xtask refresh-semver-baseline --help");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Refresh the checked-in ffhn-core semver baseline"));
    assert!(stdout.contains("--git-ref <REF>"));
    assert!(stdout.contains("Published Git tag, branch, or commit"));
}

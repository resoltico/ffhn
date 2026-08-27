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
    assert!(stdout.contains("mutants"));
    assert!(stdout.contains("cargo xtask mutants"));
    assert!(stdout.contains("structure"));
    assert!(stdout.contains("cargo xtask structure check"));
}

#[test]
fn xtask_binary_mutants_help_exposes_isolated_scopes() {
    let output = Command::cargo_bin("xtask")
        .expect("xtask binary")
        .args(["mutants", "--help"])
        .output()
        .expect("run xtask mutants --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("isolated copied-workspace mode"));
    assert!(stdout.contains("--scope <SCOPE>"));
    assert!(stdout.contains("runtime"));
    assert!(stdout.contains("tooling"));
    assert!(!stdout.contains("--in-place"));
    assert!(stdout.contains("--in-diff <DIFF_FILE>"));
}

#[test]
fn xtask_binary_structure_help_explains_the_fail_closed_contract() {
    let output = Command::cargo_bin("xtask")
        .expect("xtask binary")
        .args(["structure", "--help"])
        .output()
        .expect("run xtask structure --help");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("check is fail-closed"));
    assert!(stdout.contains("check"));
    assert!(stdout.contains("report"));
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

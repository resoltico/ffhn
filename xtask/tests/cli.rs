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

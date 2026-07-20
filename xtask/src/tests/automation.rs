use std::fs;
use std::path::Path;

#[test]
fn dependency_freshness_uses_only_its_pinned_tool() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let workflow = fs::read_to_string(repo_root.join(".github/workflows/dependency-freshness.yml"))
        .expect("read dependency-freshness workflow");

    assert!(workflow.contains("install-stable-toolchain"));
    assert!(workflow.contains("install-dependency-freshness-tool"));
    assert!(!workflow.contains("install-qa-tools"));
    assert!(workflow.contains("cargo outdated --workspace --root-deps-only --exit-code 1"));
    assert!(
        workflow.contains(
            "cargo outdated --manifest-path fuzz/Cargo.toml --root-deps-only --exit-code 1"
        )
    );

    let bootstrap = fs::read_to_string(repo_root.join("scripts/bootstrap-rust-tools.sh"))
        .expect("read Rust bootstrap script");
    assert!(bootstrap.contains("install_dependency_freshness_tool()"));
    assert!(
        bootstrap.contains(
            "cargo install cargo-outdated --version \"${CARGO_OUTDATED_VERSION}\" --locked"
        )
    );
}

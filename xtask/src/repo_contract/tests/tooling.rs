use super::*;
use crate::tooling::rust_tooling;
use toml::Value;

#[test]
fn rust_tooling_manifest_owns_the_repo_toolchain_and_qa_versions() {
    let repo_root = repo_root();
    let tooling = rust_tooling(&repo_root).expect("rust tooling");

    let workspace_manifest = read_toml(repo_root.join("Cargo.toml"));
    assert_eq!(
        workspace_manifest["workspace"]["package"]["edition"]
            .as_str()
            .expect("workspace edition"),
        tooling.workspace_edition
    );
    assert_eq!(
        workspace_manifest["workspace"]["package"]["rust-version"]
            .as_str()
            .expect("workspace rust-version"),
        tooling.workspace_rust_version
    );

    let fuzz_manifest = read_toml(repo_root.join("fuzz/Cargo.toml"));
    assert_eq!(
        fuzz_manifest["package"]["edition"]
            .as_str()
            .expect("fuzz edition"),
        tooling.workspace_edition
    );
    assert_eq!(
        fuzz_manifest["package"]["rust-version"]
            .as_str()
            .expect("fuzz rust-version"),
        tooling.workspace_rust_version
    );

    let rust_toolchain = read_toml(repo_root.join("rust-toolchain.toml"));
    assert_eq!(
        rust_toolchain["toolchain"]["channel"]
            .as_str()
            .expect("toolchain channel"),
        tooling.stable_toolchain
    );
    let components = rust_toolchain["toolchain"]["components"]
        .as_array()
        .expect("toolchain components");
    for component in ["clippy", "rustfmt", "llvm-tools-preview"] {
        assert!(
            components
                .iter()
                .any(|entry| entry.as_str() == Some(component)),
            "rust-toolchain.toml must install {component}"
        );
    }

    let bootstrap = fs::read_to_string(repo_root.join("scripts/bootstrap-rust-tools.sh"))
        .expect("read bootstrap-rust-tools.sh");
    assert!(bootstrap.contains("tooling/rust-tooling.env"));
    assert!(bootstrap.contains("install-stable-toolchain"));
    assert!(bootstrap.contains("install-coverage-toolchain"));
    assert!(bootstrap.contains("install-cross-platform-qa-tools"));

    let ci = fs::read_to_string(repo_root.join(".github/workflows/ci.yml"))
        .expect("read .github/workflows/ci.yml");
    assert!(ci.contains("bash ./scripts/bootstrap-rust-tools.sh install-toolchains"));
    assert!(ci.contains("bash ./scripts/bootstrap-rust-tools.sh install-qa-tools"));
    assert!(ci.contains("bash ./scripts/bootstrap-rust-tools.sh install-stable-toolchain"));
    assert!(ci.contains("bash ./scripts/bootstrap-rust-tools.sh install-cross-platform-qa-tools"));
    assert!(!ci.contains("RUST_STABLE_VERSION:"));
    assert!(!ci.contains("taiki-e/install-action@"));

    let release = fs::read_to_string(repo_root.join(".github/workflows/release.yml"))
        .expect("read .github/workflows/release.yml");
    assert!(release.contains("bash ./scripts/bootstrap-rust-tools.sh install-stable-toolchain"));
    assert!(!release.contains("RUST_STABLE_VERSION:"));

    let dockerfile = fs::read_to_string(repo_root.join(".devcontainer/Dockerfile"))
        .expect("read .devcontainer/Dockerfile");
    assert!(
        dockerfile.contains("COPY tooling/rust-tooling.env /usr/local/share/ffhn/rust-tooling.env")
    );
    assert!(dockerfile.contains(
        "COPY scripts/bootstrap-rust-tools.sh /usr/local/share/ffhn/bootstrap-rust-tools.sh"
    ));
    assert!(dockerfile.contains("bash /usr/local/share/ffhn/bootstrap-rust-tools.sh install-all"));
    assert!(dockerfile.contains(
        "rustup target add x86_64-unknown-linux-musl --toolchain \"${RUST_STABLE_TOOLCHAIN}\""
    ));

    let devcontainer = fs::read_to_string(repo_root.join(".devcontainer/devcontainer.json"))
        .expect("read .devcontainer/devcontainer.json");
    assert!(devcontainer.contains("\"context\": \"..\""));

    let freshness =
        fs::read_to_string(repo_root.join(".github/workflows/dependency-freshness.yml"))
            .expect("read dependency-freshness workflow");
    assert!(freshness.contains("cargo outdated --workspace --root-deps-only --exit-code 1"));
    assert!(
        freshness.contains(
            "cargo outdated --manifest-path fuzz/Cargo.toml --root-deps-only --exit-code 1"
        )
    );
}

fn read_toml(path: impl Into<std::path::PathBuf>) -> Value {
    let path = path.into();
    let text = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("read {}: {error}", path.display());
    });
    toml::from_str(&text).unwrap_or_else(|error| {
        panic!("parse {}: {error}", path.display());
    })
}

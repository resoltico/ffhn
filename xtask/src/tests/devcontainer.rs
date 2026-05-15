use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn validate_devcontainer_shell_entrypoint_is_self_describing() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let script_path = crate::release::bash_source_argument_for_tests(
        &repo_root.join("scripts/validate-devcontainer.sh"),
    );

    let output = Command::new(crate::release::bash_program_for_tests())
        .arg(script_path)
        .arg("--help")
        .current_dir(repo_root)
        .output()
        .expect("run validate-devcontainer --help");

    assert!(
        output.status.success(),
        "validate-devcontainer --help failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("validate-devcontainer.sh"));
    assert!(stdout.contains("contributor devcontainer"));
}

#[test]
fn validate_devcontainer_help_stays_self_describing_inside_the_contributor_env() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let script_path = crate::release::bash_source_argument_for_tests(
        &repo_root.join("scripts/validate-devcontainer.sh"),
    );

    let output = Command::new(crate::release::bash_program_for_tests())
        .arg(script_path)
        .arg("--help")
        .env("FFHN_DEVCONTAINER", "1")
        .current_dir(repo_root)
        .output()
        .expect("run validate-devcontainer --help in contributor env");

    assert!(
        output.status.success(),
        "validate-devcontainer --help in contributor env failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("validate-devcontainer.sh"));
    assert!(stdout.contains("contributor devcontainer"));
}

#[test]
fn run_devcontainer_check_shell_entrypoint_is_self_describing() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let script_path = crate::release::bash_source_argument_for_tests(
        &repo_root.join("scripts/run-devcontainer-check.sh"),
    );

    let output = Command::new(crate::release::bash_program_for_tests())
        .arg(script_path)
        .arg("--help")
        .current_dir(repo_root)
        .output()
        .expect("run run-devcontainer-check --help");

    assert!(
        output.status.success(),
        "run-devcontainer-check --help failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("run-devcontainer-check.sh"));
    assert!(stdout.contains("./check.sh"));
}

#[test]
fn committed_devcontainer_contract_uses_canonical_paths_and_prepare_hook() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let devcontainer = fs::read_to_string(repo_root.join(".devcontainer/devcontainer.json"))
        .expect("read devcontainer.json");
    let json: Value = serde_json::from_str(&devcontainer).expect("parse devcontainer.json");

    assert_eq!(json["name"], "FFHN Contributor");
    assert_eq!(json["build"]["dockerfile"], "Dockerfile");
    assert_eq!(json["build"]["context"], "..");
    assert_eq!(json["workspaceFolder"], "/workspaces/ffhn");
    assert_eq!(
        json["workspaceMount"],
        "source=${localWorkspaceFolder},target=/workspaces/ffhn,type=bind,consistency=cached"
    );
    assert_eq!(
        json["postStartCommand"],
        "./scripts/devcontainer-prepare-user-home.sh"
    );
    assert_eq!(json["remoteUser"], "vscode");
    assert_eq!(json["updateRemoteUserUID"], true);
    assert_eq!(json["containerEnv"]["CARGO_HOME"], "/home/vscode/.cargo");
    assert_eq!(
        json["containerEnv"]["CARGO_TARGET_DIR"],
        "/home/vscode/.cache/ffhn-artifacts/target"
    );
    assert_eq!(
        json["containerEnv"]["CARGO_BUILD_BUILD_DIR"],
        "/home/vscode/.cache/ffhn-artifacts/build"
    );
    assert_eq!(json["containerEnv"]["FFHN_DEVCONTAINER"], "1");

    let mounts = json["mounts"].as_array().expect("mount array");
    assert!(mounts.iter().any(|mount| mount
        == "source=ffhn-cargo-registry,target=/home/vscode/.cargo/registry,type=volume"));
    assert!(
        mounts.iter().any(
            |mount| mount == "source=ffhn-cargo-git,target=/home/vscode/.cargo/git,type=volume"
        )
    );
    assert!(
        mounts
            .iter()
            .any(|mount| mount == "source=ffhn-user-cache,target=/home/vscode/.cache,type=volume")
    );
    assert_eq!(mounts.len(), 3);
}

#[test]
fn committed_devcontainer_dockerfile_pins_the_24_04_devcontainer_base_and_bakes_repo_tools() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let dockerfile = fs::read_to_string(repo_root.join(".devcontainer/Dockerfile"))
        .expect("read devcontainer Dockerfile");

    assert!(dockerfile.contains("FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04@sha256:"));
    assert!(
        dockerfile.contains("COPY tooling/rust-tooling.env /usr/local/share/ffhn/rust-tooling.env")
    );
    assert!(dockerfile.contains(
        "COPY scripts/bootstrap-rust-tools.sh /usr/local/share/ffhn/bootstrap-rust-tools.sh"
    ));
    assert!(dockerfile.contains("if getent passwd 1000 >/dev/null; then"));
    assert!(dockerfile.contains("usermod --login vscode"));
    assert!(dockerfile.contains("ENV RUSTUP_HOME=/usr/local/rustup"));
    assert!(
        dockerfile.contains("ENV FFHN_RUST_TOOLING_ENV=/usr/local/share/ffhn/rust-tooling.env")
    );
    assert!(dockerfile.contains("bash /usr/local/share/ffhn/bootstrap-rust-tools.sh install-all"));
    assert!(dockerfile.contains(
        "rustup target add x86_64-unknown-linux-musl --toolchain \"${RUST_STABLE_TOOLCHAIN}\""
    ));
    assert!(dockerfile.contains("USER vscode"));
}

#[test]
fn committed_devcontainer_cli_helper_installs_pinned_devcontainer_cli() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let dockerfile =
        fs::read_to_string(repo_root.join("scripts/devcontainer-cli-helper.Dockerfile"))
            .expect("read devcontainer cli helper dockerfile");

    assert!(dockerfile.contains("ARG BASE_IMAGE=ffhn-devcontainer:local"));
    assert!(dockerfile.contains("FROM ${BASE_IMAGE}"));
    assert!(
        dockerfile.contains("apt-get install --yes --no-install-recommends docker.io nodejs npm")
    );
    assert!(dockerfile.contains("@devcontainers/cli@0.86.0"));
    assert!(dockerfile.contains("USER root"));
}

#[test]
fn run_devcontainer_check_uses_the_committed_image_and_canonical_cache_volumes() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let script = fs::read_to_string(repo_root.join("scripts/run-devcontainer-check.sh"))
        .expect("read run-devcontainer-check.sh");

    assert!(script.contains("image_tag=\"ffhn-devcontainer:local\""));
    assert!(script.contains("ffhn-cargo-registry:/home/vscode/.cargo/registry"));
    assert!(script.contains("ffhn-cargo-git:/home/vscode/.cargo/git"));
    assert!(script.contains("ffhn-user-cache:/home/vscode/.cache"));
    assert!(script.contains("CARGO_TARGET_DIR=/home/vscode/.cache/ffhn-artifacts/target"));
    assert!(script.contains("CARGO_BUILD_BUILD_DIR=/home/vscode/.cache/ffhn-artifacts/build"));
    assert!(script.contains("FFHN_DEVCONTAINER_SKIP_BUILD"));
    assert!(script.contains("docker image inspect"));
    assert!(script.contains("ffhn_docker_build"));
    assert!(script.contains("./scripts/devcontainer-prepare-user-home.sh"));
    assert!(script.contains("git config --global --add safe.directory /workspaces/ffhn"));
    assert!(script.contains("./check.sh"));
}

#[test]
fn validate_devcontainer_repairs_workspace_volumes_and_exercises_client_path() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let script = fs::read_to_string(repo_root.join("scripts/validate-devcontainer.sh"))
        .expect("read validate-devcontainer.sh");

    assert!(script.contains("FFHN_DEVCONTAINER_VOLUME_MODE"));
    assert!(script.contains("CARGO_TARGET_DIR=/home/vscode/.cache/ffhn-artifacts/target"));
    assert!(script.contains("CARGO_BUILD_BUILD_DIR=/home/vscode/.cache/ffhn-artifacts/build"));
    assert!(script.contains("${HOME}/.cache"));
    assert!(script.contains("scripts/devcontainer-cli-helper.Dockerfile"));
    assert!(script.contains("ffhn_docker_build"));
    assert!(script.contains("--build-arg \"BASE_IMAGE=${image_tag}\""));
    assert!(script.contains("readonly FFHN_VALIDATE_REPO_ROOT"));
    assert!(script.contains("--volume \"${repo_root}:/workspaces/ffhn\""));
    assert!(!script.contains("--volume \"${repo_root}:/workspaces/ffhn:ro\""));
    assert!(script.contains("devcontainer up --remove-existing-container"));
    assert!(script.contains("devcontainer exec --workspace-folder"));
}

#[test]
fn common_shell_helper_defines_linear_docker_build_progress_by_default() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let script = fs::read_to_string(repo_root.join("scripts/common.sh")).expect("read common.sh");

    assert!(script.contains("ffhn_docker_build()"));
    assert!(script.contains("FFHN_DOCKER_BUILD_PROGRESS:-plain"));
    assert!(script.contains("docker build --progress \"${progress}\" \"$@\""));
}

#[test]
fn ci_workflow_runs_the_full_headless_devcontainer_gate() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let workflow =
        fs::read_to_string(repo_root.join(".github/workflows/ci.yml")).expect("read ci workflow");

    let contributor_job = workflow
        .split("contributor-devcontainer-gate:")
        .nth(1)
        .and_then(|section| section.split("cross-platform-rust-gate:").next())
        .expect("contributor-devcontainer-gate section");

    assert!(contributor_job.contains("./scripts/validate-devcontainer.sh"));
    assert!(contributor_job.contains("./scripts/run-devcontainer-check.sh"));
    assert!(contributor_job.contains("FFHN_DEVCONTAINER_VOLUME_MODE: shared-ci"));
    assert!(contributor_job.contains("FFHN_DEVCONTAINER_SKIP_BUILD: 1"));
}

#[test]
fn ci_workflow_gives_the_cross_platform_gate_enough_time_for_windows() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let workflow =
        fs::read_to_string(repo_root.join(".github/workflows/ci.yml")).expect("read ci workflow");

    let cross_platform_job = workflow
        .split("cross-platform-rust-gate:")
        .nth(1)
        .and_then(|section| section.split("release-target-smoke:").next())
        .expect("cross-platform-rust-gate section");

    assert!(cross_platform_job.contains("timeout-minutes: ${{ matrix.timeout_minutes }}"));
    assert!(cross_platform_job.contains("timeout_minutes: 150"));
    assert!(!cross_platform_job.contains("timeout_minutes: 60"));
    assert!(cross_platform_job.contains("- id: windows-x64"));
    assert!(cross_platform_job.contains("cargo nextest run --no-fail-fast"));
    assert!(cross_platform_job.contains("cargo xtask semver-check"));
}

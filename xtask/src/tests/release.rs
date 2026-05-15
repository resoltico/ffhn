use super::*;
use std::fs;
use std::path::Path;
use std::process::Command;

fn bash_command() -> Command {
    Command::new(crate::release::bash_program_for_tests())
}

fn seed_release_script_repo() -> tempfile::TempDir {
    let source_repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let temp = tempdir().expect("tempdir");
    let repo_root = temp.path();
    let scripts_dir = repo_root.join("scripts");
    fs::create_dir_all(&scripts_dir).expect("create scripts dir");

    for script_name in [
        "common.sh",
        "release-targets.sh",
        "workspace-package-field.sh",
        "build-release-source-archives.sh",
        "build-release-artifact.sh",
        "build-release-checksums.sh",
        "smoke-release-artifact.sh",
        "publish-github-release.sh",
        "verify-github-release.sh",
    ] {
        fs::copy(
            source_repo.join("scripts").join(script_name),
            scripts_dir.join(script_name),
        )
        .unwrap_or_else(|error| panic!("copy {script_name}: {error}"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            fs::set_permissions(
                scripts_dir.join(script_name),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap_or_else(|error| panic!("chmod {script_name}: {error}"));
        }
    }

    fs::write(
        repo_root.join("Cargo.toml"),
        r#"[workspace]
members = []
resolver = "3"

[workspace.package]
version = "9.9.9"
"#,
    )
    .expect("write Cargo.toml");
    fs::write(repo_root.join("README.md"), "# demo\n").expect("write README");
    fs::write(repo_root.join("LICENSE"), "MIT\n").expect("write LICENSE");
    fs::write(repo_root.join("NOTICE"), "notice\n").expect("write NOTICE");
    fs::write(repo_root.join("PATENTS.md"), "# patents\n").expect("write PATENTS.md");
    fs::write(repo_root.join("changelog.md"), "# changelog\n").expect("write changelog");

    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .unwrap_or_else(|error| panic!("run git {:?}: {error}", args));
        assert!(
            output.status.success(),
            "git {:?} failed:\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init"]);
    git(&["config", "user.email", "tests@example.com"]);
    git(&["config", "user.name", "FFHN Tests"]);
    git(&["add", "."]);
    git(&["commit", "-m", "init"]);

    temp
}

fn release_script_argument(repo_root: &Path, script_name: &str) -> String {
    crate::release::bash_source_argument_for_tests(&repo_root.join("scripts").join(script_name))
}

fn seed_release_dist_inventory(repo_root: &Path) {
    let dist = repo_root.join("dist");
    fs::create_dir_all(&dist).expect("create dist");

    for asset in [
        "ffhn-source-9.9.9.zip",
        "ffhn-source-9.9.9.tar.gz",
        "ffhn-9.9.9-aarch64-apple-darwin.tar.gz",
        "ffhn-9.9.9-x86_64-apple-darwin.tar.gz",
        "ffhn-9.9.9-x86_64-unknown-linux-musl.tar.gz",
        "ffhn-9.9.9-x86_64-pc-windows-msvc.zip",
    ] {
        fs::write(dist.join(asset), format!("dummy asset for {asset}\n"))
            .unwrap_or_else(|error| panic!("write {asset}: {error}"));
    }
}

#[test]
fn release_helpers_read_the_canonical_shell_registry() {
    let repo_root = tempdir().expect("tempdir");
    let scripts_dir = repo_root.path().join("scripts");
    fs::create_dir_all(&scripts_dir).expect("create scripts dir");
    fs::write(
        scripts_dir.join("release-targets.sh"),
        r#"#!/usr/bin/env bash
release_target_triples() {
    cat <<'EOF'
aarch64-apple-darwin
x86_64-pc-windows-msvc
EOF
}

release_matrix_json() {
    cat <<'EOF'
{"include":[{"id":"macos-arm64","runs_on":"macos-15","target_triple":"aarch64-apple-darwin","artifact_bundle_name":"standalone-macos-arm64","needs_musl_tools":false}]}
EOF
}

release_asset_names_for_version() {
    local release_version="$1"
    printf 'ffhn-source-%s.tar.gz\n' "${release_version}"
    printf 'ffhn-%s-checksums.txt\n' "${release_version}"
}

macos_deployment_target_for_target() {
    local requested_target="$1"
    case "${requested_target}" in
        aarch64-apple-darwin) printf '12.0\n' ;;
        *) printf '\n' ;;
    esac
}

case "${1:-}" in
    triples)
        release_target_triples
        ;;
    matrix-json)
        release_matrix_json
        ;;
    assets)
        [[ "${2:-}" == "--version" ]] || exit 64
        release_asset_names_for_version "${3:-}"
        ;;
    macos-deployment-target)
        [[ "${2:-}" == "--target" ]] || exit 64
        macos_deployment_target_for_target "${3:-}"
        ;;
esac
"#,
    )
    .expect("write release-targets.sh");

    assert_eq!(
        crate::release::release_target_triples(repo_root.path()).expect("target triples"),
        vec![
            "aarch64-apple-darwin".to_owned(),
            "x86_64-pc-windows-msvc".to_owned(),
        ]
    );
    assert_eq!(
        crate::release::release_asset_names(repo_root.path(), "9.9.9").expect("asset names"),
        vec![
            "ffhn-source-9.9.9.tar.gz".to_owned(),
            "ffhn-9.9.9-checksums.txt".to_owned(),
        ]
    );
    assert_eq!(
        crate::release::release_matrix(repo_root.path()).expect("release matrix"),
        vec![crate::release::ReleaseMatrixEntry {
            id: "macos-arm64".to_owned(),
            runs_on: "macos-15".to_owned(),
            target_triple: "aarch64-apple-darwin".to_owned(),
            artifact_bundle_name: "standalone-macos-arm64".to_owned(),
            needs_musl_tools: false,
        }]
    );
    assert_eq!(
        crate::release::macos_deployment_target(repo_root.path(), "aarch64-apple-darwin")
            .expect("macos deployment target"),
        Some("12.0".to_owned())
    );
    assert_eq!(
        crate::release::macos_deployment_target(repo_root.path(), "x86_64-pc-windows-msvc")
            .expect("windows deployment target"),
        None
    );
}

#[test]
fn release_shell_helpers_resolve_repo_root_workspace_version_and_workspace_fields() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let scripts_dir = repo_root.join("scripts");
    let version = workspace_version(repo_root).expect("workspace version");
    let description =
        workspace_package_field(repo_root, "description").expect("workspace description");
    let scripts_dir = crate::release::bash_source_argument_for_tests(&scripts_dir);
    let repo_root_for_bash = crate::release::bash_source_argument_for_tests(repo_root);
    let script = format!(
        r#"set -euo pipefail
script_dir="{scripts_dir}"
source "$script_dir/common.sh"

script_dir="$(ffhn_resolve_script_dir "$script_dir/common.sh")"
readonly script_dir
repo_root="{repo_root}"
readonly repo_root

resolved_root="$(ffhn_repo_root_from_script_dir "$script_dir")"
[[ "$resolved_root" == "$repo_root" ]]

resolved_version="$(ffhn_workspace_version "$script_dir" "$repo_root")"
[[ "$resolved_version" == "{version}" ]]

resolved_description="$(ffhn_workspace_description "$script_dir" "$repo_root")"
[[ "$resolved_description" == "{description}" ]]

resolved_field="$("$script_dir/workspace-package-field.sh" description "$repo_root/Cargo.toml")"
[[ "$resolved_field" == "{description}" ]]
"#,
        scripts_dir = scripts_dir,
        repo_root = repo_root_for_bash,
        version = version,
        description = description,
    );

    let output = bash_command()
        .arg("-c")
        .arg(script)
        .current_dir(repo_root)
        .output()
        .expect("run helper smoke");

    assert!(
        output.status.success(),
        "release shell helper smoke failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn release_shell_helpers_normalize_windows_temp_roots() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let temp = tempdir().expect("tempdir");
    let fake_bin = temp.path().join("bin");
    fs::create_dir_all(&fake_bin).expect("create fake bin");

    fs::write(
        fake_bin.join("cygpath"),
        "#!/usr/bin/env bash\nset -euo pipefail\n[[ \"$1\" == \"-u\" ]]\nprintf '/d/a/_temp\\n'\n",
    )
    .expect("write fake cygpath");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(fake_bin.join("cygpath"), fs::Permissions::from_mode(0o755))
            .expect("chmod fake cygpath");
    }

    let fake_bin_for_bash = crate::release::bash_source_argument_for_tests(&fake_bin);
    let repo_root_for_bash = crate::release::bash_source_argument_for_tests(repo_root);
    let output = bash_command()
        .arg("-c")
        .arg(format!(
            r#"set -euo pipefail
PATH="{fake_bin}:$PATH"
export OS=Windows_NT
export RUNNER_TEMP='D:\a\_temp'
source "{repo_root}/scripts/common.sh"
[[ "$(ffhn_temp_root)" == "/d/a/_temp" ]]
"#,
            fake_bin = fake_bin_for_bash,
            repo_root = repo_root_for_bash,
        ))
        .current_dir(repo_root)
        .output()
        .expect("run temp-root helper smoke");

    assert!(
        output.status.success(),
        "temp-root helper smoke failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn release_shell_helpers_resolve_the_active_cargo_target_root() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let scripts_dir = crate::release::bash_source_argument_for_tests(&repo_root.join("scripts"));
    let repo_root_for_bash = crate::release::bash_source_argument_for_tests(repo_root);
    let absolute_target_root = tempdir().expect("absolute target root");
    let absolute_target_root_for_bash =
        crate::release::bash_source_argument_for_tests(absolute_target_root.path());
    let script = format!(
        r#"set -euo pipefail
script_dir="{scripts_dir}"
repo_root="{repo_root}"
absolute_target_root="{absolute_target_root}"
source "$script_dir/common.sh"

expected_default_root="$(cd "$repo_root/.." && pwd)/.ffhn-artifacts/target"
expected_default_root="$(ffhn_normalize_bash_path "$expected_default_root")"

unset CARGO_TARGET_DIR
[[ "$(ffhn_cargo_target_dir "$repo_root")" == "$expected_default_root" ]]

export CARGO_TARGET_DIR="custom-target"
[[ "$(ffhn_cargo_target_dir "$repo_root")" == "$(ffhn_normalize_bash_path "$repo_root/custom-target")" ]]

export CARGO_TARGET_DIR="$absolute_target_root"
[[ "$(ffhn_cargo_target_dir "$repo_root")" == "$(ffhn_normalize_bash_path "$absolute_target_root")" ]]
"#,
        scripts_dir = scripts_dir,
        repo_root = repo_root_for_bash,
        absolute_target_root = absolute_target_root_for_bash,
    );

    let output = bash_command()
        .arg("-c")
        .arg(script)
        .current_dir(repo_root)
        .output()
        .expect("run cargo target helper smoke");

    assert!(
        output.status.success(),
        "cargo target helper smoke failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn release_shell_helpers_normalize_windows_source_paths() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let repo_root_for_bash = crate::release::bash_source_argument_for_tests(repo_root);

    let output = bash_command()
        .arg("-c")
        .arg(format!(
            r#"set -euo pipefail
source "{repo_root}/scripts/common.sh"
[[ "$(ffhn_normalize_bash_path 'D:\a\ffhn\scripts\release-targets.sh')" == '/d/a/ffhn/scripts/release-targets.sh' ]]
[[ "$(ffhn_normalize_bash_path 'scripts\release-targets.sh')" == 'scripts/release-targets.sh' ]]
"#,
            repo_root = repo_root_for_bash,
        ))
        .current_dir(repo_root)
        .output()
        .expect("run path-normalization smoke");

    assert!(
        output.status.success(),
        "path-normalization smoke failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

mod archives;
mod entrypoints;

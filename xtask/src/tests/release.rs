use super::*;

#[test]
fn release_shell_helpers_resolve_repo_root_and_workspace_version() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let scripts_dir = repo_root.join("scripts");
    let version = workspace_version(repo_root).expect("workspace version");
    let description =
        workspace_package_field(repo_root, "description").expect("workspace description");
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
"#,
        scripts_dir = scripts_dir.display(),
        repo_root = repo_root.display(),
        version = version,
        description = description,
    );

    let output = Command::new("bash")
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
fn release_shell_helpers_extract_arbitrary_workspace_package_fields() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let scripts_dir = repo_root.join("scripts");
    let version = workspace_version(repo_root).expect("workspace version");
    let description =
        workspace_package_field(repo_root, "description").expect("workspace description");
    let script = format!(
        r#"set -euo pipefail
script_dir="{scripts_dir}"

version="$("$script_dir/workspace-package-field.sh" version "{repo_root}/Cargo.toml")"
[[ "$version" == "{version}" ]]

description="$("$script_dir/workspace-package-field.sh" description "{repo_root}/Cargo.toml")"
[[ "$description" == "{description}" ]]
"#,
        scripts_dir = scripts_dir.display(),
        repo_root = repo_root.display(),
        version = version,
        description = description,
    );

    let output = Command::new("bash")
        .arg("-c")
        .arg(script)
        .current_dir(repo_root)
        .output()
        .expect("run workspace-package-field smoke");

    assert!(
        output.status.success(),
        "workspace-package-field smoke failed:\nstdout:\n{}\nstderr:\n{}",
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

    let output = Command::new("bash")
        .arg("-c")
        .arg(format!(
            r#"set -euo pipefail
PATH="{fake_bin}:$PATH"
export OS=Windows_NT
export RUNNER_TEMP='D:\a\_temp'
source "{repo_root}/scripts/common.sh"
[[ "$(ffhn_temp_root)" == "/d/a/_temp" ]]
"#,
            fake_bin = fake_bin.display(),
            repo_root = repo_root.display(),
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
fn windows_release_packager_uses_forward_slash_zip_entries() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let temp = tempdir().expect("tempdir");
    let fake_bin = temp.path().join("bin");
    fs::create_dir_all(&fake_bin).expect("create fake bin");
    let log_path = temp.path().join("powershell.log");

    fs::write(
        fake_bin.join("powershell.exe"),
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\n{{\n  printf 'SOURCE_PARENT_PATH=%s\\n' \"$SOURCE_PARENT_PATH\"\n  printf 'PACKAGE_ROOT_NAME=%s\\n' \"$PACKAGE_ROOT_NAME\"\n  printf 'ARCHIVE_OUTPUT_PATH=%s\\n' \"$ARCHIVE_OUTPUT_PATH\"\n  printf 'ARGS=%s\\n' \"$*\"\n}} > \"{}\"\n",
            log_path.display()
        ),
    )
    .expect("write fake powershell");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(
            fake_bin.join("powershell.exe"),
            fs::Permissions::from_mode(0o755),
        )
        .expect("chmod fake powershell");
    }

    let output = Command::new("bash")
        .arg("-c")
        .arg(format!(
            r#"set -euo pipefail
PATH="{fake_bin}:$PATH"
source "{repo_root}/scripts/build-release-artifact.sh"
create_zip_with_dotnet "/tmp/source-parent" "ffhn-9.9.9-x86_64-pc-windows-msvc" "/tmp/output.zip"
"#,
            fake_bin = fake_bin.display(),
            repo_root = repo_root.display(),
        ))
        .current_dir(repo_root)
        .output()
        .expect("run packager fallback smoke");

    assert!(
        output.status.success(),
        "packager fallback smoke failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let log = fs::read_to_string(&log_path).expect("read powershell log");
    assert!(log.contains("PACKAGE_ROOT_NAME=ffhn-9.9.9-x86_64-pc-windows-msvc"));
    assert!(log.contains("Add-Type -AssemblyName System.IO.Compression"));
    assert!(log.contains("Add-Type -AssemblyName System.IO.Compression.FileSystem"));
    assert!(log.contains("ZipArchiveMode]::Create"));
    assert!(log.contains(r#"-replace "\\", "/""#));
}

#[test]
fn windows_release_smoke_prefers_bash_native_unzip_before_powershell() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let temp = tempdir().expect("tempdir");
    let fake_bin = temp.path().join("bin");
    fs::create_dir_all(&fake_bin).expect("create fake bin");
    let log_path = temp.path().join("extractor.log");

    fs::write(
        fake_bin.join("unzip"),
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nprintf 'unzip\\n' > \"{}\"\n",
            log_path.display()
        ),
    )
    .expect("write fake unzip");
    fs::write(
        fake_bin.join("powershell.exe"),
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nprintf 'powershell\\n' > \"{}\"\n",
            log_path.display()
        ),
    )
    .expect("write fake powershell");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let executable = fs::Permissions::from_mode(0o755);
        fs::set_permissions(fake_bin.join("unzip"), executable.clone()).expect("chmod fake unzip");
        fs::set_permissions(fake_bin.join("powershell.exe"), executable)
            .expect("chmod fake powershell");
    }

    let output = Command::new("bash")
        .arg("-c")
        .arg(format!(
            r#"set -euo pipefail
PATH="{fake_bin}:$PATH"
export OS=Windows_NT
source "{repo_root}/scripts/smoke-release-artifact.sh"
extract_release_archive "/tmp/archive.zip" "zip" "/tmp/extract-root"
"#,
            fake_bin = fake_bin.display(),
            repo_root = repo_root.display(),
        ))
        .current_dir(repo_root)
        .output()
        .expect("run extractor selection smoke");

    assert!(
        output.status.success(),
        "extractor selection smoke failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&log_path).expect("read extractor log"),
        "unzip\n"
    );
}

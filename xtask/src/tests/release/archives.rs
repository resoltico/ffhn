use super::*;

#[test]
fn release_package_scripts_keep_the_packaged_legal_files_in_sync_with_the_storefront_readme() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let build_script = fs::read_to_string(repo_root.join("scripts/build-release-artifact.sh"))
        .expect("read build-release-artifact.sh");
    let smoke_script = fs::read_to_string(repo_root.join("scripts/smoke-release-artifact.sh"))
        .expect("read smoke-release-artifact.sh");

    assert!(build_script.contains("cp \"${repo_root}/NOTICE\" \"${package_dir}/NOTICE\""));
    assert!(build_script.contains("cp \"${repo_root}/PATENTS.md\" \"${package_dir}/PATENTS.md\""));
    assert!(smoke_script.contains("missing packaged NOTICE"));
    assert!(smoke_script.contains("missing packaged PATENTS.md"));
}

#[test]
fn build_release_artifact_honors_cargo_target_dir_override() {
    let repo = seed_release_script_repo();
    let repo_root = repo.path();
    let temp = tempdir().expect("tempdir");
    let fake_bin = temp.path().join("bin");
    fs::create_dir_all(&fake_bin).expect("create fake bin");
    let log_path = temp.path().join("cargo.log");
    let log_path_for_bash = crate::release::bash_source_argument_for_tests(&log_path);

    fs::write(
        fake_bin.join("cargo"),
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nprintf '%s\\n' \"$*\" > \"{log_path}\"\n\
target=''\nprofile=''\nwhile [ \"$#\" -gt 0 ]; do\n\
  if [ \"$1\" = \"--target\" ]; then\n    shift\n    target=\"$1\"\n\
  elif [ \"$1\" = \"--profile\" ]; then\n    shift\n    profile=\"$1\"\n\
  fi\n  shift\ndone\n\
mkdir -p \"$CARGO_TARGET_DIR/$target/$profile\"\n\
printf '#!/usr/bin/env bash\\nexit 0\\n' > \"$CARGO_TARGET_DIR/$target/$profile/ffhn\"\n\
chmod +x \"$CARGO_TARGET_DIR/$target/$profile/ffhn\"\n",
            log_path = log_path_for_bash
        ),
    )
    .expect("write fake cargo");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(fake_bin.join("cargo"), fs::Permissions::from_mode(0o755))
            .expect("chmod fake cargo");
    }

    let fake_bin_for_bash = crate::release::bash_source_argument_for_tests(&fake_bin);
    let repo_root_for_bash = crate::release::bash_source_argument_for_tests(repo_root);
    let output = bash_command()
        .arg("-c")
        .arg(format!(
            r#"set -euo pipefail
PATH="{fake_bin}:$PATH"
export CARGO_TARGET_DIR="custom-target"
"{repo_root}/scripts/build-release-artifact.sh" aarch64-apple-darwin
"#,
            fake_bin = fake_bin_for_bash,
            repo_root = repo_root_for_bash,
        ))
        .current_dir(repo_root)
        .output()
        .expect("run build-release-artifact with target override");

    assert!(
        output.status.success(),
        "build-release-artifact override smoke failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&log_path).expect("read cargo log"),
        "build --profile dist --locked -p ffhn-cli --bin ffhn --target aarch64-apple-darwin\n"
    );
    assert!(
        repo_root
            .join("custom-target/aarch64-apple-darwin/dist/ffhn")
            .is_file()
    );
    assert!(
        repo_root
            .join("dist/ffhn-9.9.9-aarch64-apple-darwin.tar.gz")
            .is_file()
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
    let log_path_for_bash = crate::release::bash_source_argument_for_tests(&log_path);

    fs::write(
        fake_bin.join("powershell.exe"),
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\n{{\n  printf 'SOURCE_PARENT_PATH=%s\\n' \"$SOURCE_PARENT_PATH\"\n  printf 'PACKAGE_ROOT_NAME=%s\\n' \"$PACKAGE_ROOT_NAME\"\n  printf 'ARCHIVE_OUTPUT_PATH=%s\\n' \"$ARCHIVE_OUTPUT_PATH\"\n  printf 'ARGS=%s\\n' \"$*\"\n}} > \"{}\"\n",
            log_path_for_bash
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

    let fake_bin_for_bash = crate::release::bash_source_argument_for_tests(&fake_bin);
    let repo_root_for_bash = crate::release::bash_source_argument_for_tests(repo_root);
    let output = bash_command()
        .arg("-c")
        .arg(format!(
            r#"set -euo pipefail
PATH="{fake_bin}:$PATH"
source "{repo_root}/scripts/build-release-artifact.sh"
create_zip_with_dotnet "/tmp/source-parent" "ffhn-9.9.9-x86_64-pc-windows-msvc" "/tmp/output.zip"
"#,
            fake_bin = fake_bin_for_bash,
            repo_root = repo_root_for_bash,
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
    let fake_archive = temp.path().join("archive.zip");
    let fake_extract_root = temp.path().join("extract-root");
    fs::create_dir_all(&fake_bin).expect("create fake bin");
    let log_path = temp.path().join("extractor.log");
    let log_path_for_bash = crate::release::bash_source_argument_for_tests(&log_path);

    fs::write(
        fake_bin.join("unzip"),
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nprintf 'unzip\\n' > \"{}\"\n",
            log_path_for_bash
        ),
    )
    .expect("write fake unzip");
    fs::write(
        fake_bin.join("powershell.exe"),
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nprintf 'powershell\\n' > \"{}\"\n",
            log_path_for_bash
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

    let fake_bin_for_bash = crate::release::bash_source_argument_for_tests(&fake_bin);
    let repo_root_for_bash = crate::release::bash_source_argument_for_tests(repo_root);
    let fake_archive_for_bash = crate::release::bash_source_argument_for_tests(&fake_archive);
    let fake_extract_root_for_bash =
        crate::release::bash_source_argument_for_tests(&fake_extract_root);
    let output = bash_command()
        .arg("-c")
        .arg(format!(
            r#"set -euo pipefail
PATH="{fake_bin}:$PATH"
export OS=Windows_NT
source "{repo_root}/scripts/smoke-release-artifact.sh"
extract_release_archive "{fake_archive}" "zip" "{fake_extract_root}"
"#,
            fake_bin = fake_bin_for_bash,
            repo_root = repo_root_for_bash,
            fake_archive = fake_archive_for_bash,
            fake_extract_root = fake_extract_root_for_bash,
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

#[test]
fn release_smoke_rejects_archive_metadata_sidecars() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let temp = tempdir().expect("tempdir");
    let extract_root = temp.path().join("extract-root");
    fs::create_dir_all(&extract_root).expect("create extract root");
    fs::write(extract_root.join("._README.md"), "metadata").expect("write sidecar");

    let repo_root_for_bash = crate::release::bash_source_argument_for_tests(repo_root);
    let extract_root_for_bash = crate::release::bash_source_argument_for_tests(&extract_root);
    let output = bash_command()
        .arg("-c")
        .arg(format!(
            r#"set -euo pipefail
source "{repo_root}/scripts/smoke-release-artifact.sh"
status=0
if (assert_no_archive_metadata_sidecars "{extract_root}"); then
    status=0
else
    status=$?
fi
printf 'status=%s\n' "$status"
exit 0
"#,
            repo_root = repo_root_for_bash,
            extract_root = extract_root_for_bash,
        ))
        .current_dir(repo_root)
        .output()
        .expect("run sidecar rejection smoke");

    assert!(
        output.status.success(),
        "sidecar rejection smoke wrapper failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("status=1"));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("release archive contains platform metadata sidecars")
    );
}

#[test]
fn build_release_source_archives_script_creates_the_maintained_source_bundle_pair() {
    let repo = seed_release_script_repo();
    let repo_root = repo.path();

    let output = bash_command()
        .arg(release_script_argument(
            repo_root,
            "build-release-source-archives.sh",
        ))
        .current_dir(repo_root)
        .output()
        .expect("run source-archive builder");

    assert!(
        output.status.success(),
        "source-archive builder failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(repo_root.join("dist/ffhn-source-9.9.9.zip").is_file());
    assert!(repo_root.join("dist/ffhn-source-9.9.9.tar.gz").is_file());
}

#[test]
fn maintained_release_entrypoints_refuse_dirty_tracked_checkouts() {
    let repo = seed_release_script_repo();
    let repo_root = repo.path();
    seed_release_dist_inventory(repo_root);
    fs::write(repo_root.join("README.md"), "# dirty\n").expect("dirty tracked README");

    for (script_name, extra_args, extra_env) in [
        (
            "build-release-source-archives.sh",
            Vec::<&str>::new(),
            Vec::<(&str, &str)>::new(),
        ),
        (
            "build-release-checksums.sh",
            Vec::<&str>::new(),
            Vec::<(&str, &str)>::new(),
        ),
        (
            "build-release-artifact.sh",
            vec!["aarch64-apple-darwin"],
            Vec::<(&str, &str)>::new(),
        ),
        (
            "publish-github-release.sh",
            Vec::<&str>::new(),
            vec![("GH_TOKEN", "test-token"), ("RELEASE_TAG", "v9.9.9")],
        ),
        (
            "verify-github-release.sh",
            Vec::<&str>::new(),
            vec![("GH_TOKEN", "test-token"), ("RELEASE_TAG", "v9.9.9")],
        ),
    ] {
        let mut command = bash_command();
        command
            .arg(release_script_argument(repo_root, script_name))
            .args(extra_args)
            .current_dir(repo_root);
        for (key, value) in extra_env {
            command.env(key, value);
        }
        let output = command
            .output()
            .unwrap_or_else(|error| panic!("run {script_name}: {error}"));

        assert!(
            !output.status.success(),
            "{script_name} should reject tracked checkout drift"
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
        assert!(stderr.contains("clean tracked checkout"));
        assert!(stderr.contains("commit or stash tracked changes"));
    }

    assert!(!repo_root.join("dist/ffhn-9.9.9-checksums.txt").exists());
}

#[test]
fn release_entrypoints_report_operator_preconditions_before_dirty_checkout_guard() {
    let repo = seed_release_script_repo();
    let repo_root = repo.path();
    fs::write(repo_root.join("README.md"), "# dirty\n").expect("dirty tracked README");

    let invalid_target = bash_command()
        .arg(release_script_argument(
            repo_root,
            "build-release-artifact.sh",
        ))
        .arg("not-a-target")
        .current_dir(repo_root)
        .output()
        .expect("run invalid target triple");
    assert!(!invalid_target.status.success());
    let invalid_target_stderr = String::from_utf8(invalid_target.stderr).expect("stderr utf8");
    assert!(invalid_target_stderr.contains("unsupported release target triple: not-a-target"));
    assert!(!invalid_target_stderr.contains("clean tracked checkout"));

    for script_name in ["publish-github-release.sh", "verify-github-release.sh"] {
        let output = bash_command()
            .arg(release_script_argument(repo_root, script_name))
            .arg("v9.9.9")
            .current_dir(repo_root)
            .output()
            .unwrap_or_else(|error| panic!("run {script_name}: {error}"));
        assert!(
            !output.status.success(),
            "{script_name} should fail without GH_TOKEN"
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
        assert!(stderr.contains("GH_TOKEN is required"));
        assert!(!stderr.contains("clean tracked checkout"));
    }

    let checksum_output = bash_command()
        .arg(release_script_argument(
            repo_root,
            "build-release-checksums.sh",
        ))
        .current_dir(repo_root)
        .output()
        .expect("run checksum builder on dirty checkout");
    assert!(!checksum_output.status.success());
    let checksum_stderr = String::from_utf8(checksum_output.stderr).expect("stderr utf8");
    assert!(checksum_stderr.contains("missing maintained release assets"));
    assert!(!checksum_stderr.contains("clean tracked checkout"));
}

#[test]
fn build_release_checksums_reports_missing_inventory_with_actionable_prerequisites() {
    let repo = seed_release_script_repo();
    let repo_root = repo.path();

    let output = bash_command()
        .arg(release_script_argument(
            repo_root,
            "build-release-checksums.sh",
        ))
        .current_dir(repo_root)
        .output()
        .expect("run checksum builder");

    assert!(
        !output.status.success(),
        "checksum builder should fail without assets"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
    assert!(stderr.contains("missing maintained release assets"));
    assert!(stderr.contains("ffhn-source-9.9.9.zip"));
    assert!(stderr.contains("ffhn-9.9.9-aarch64-apple-darwin.tar.gz"));
    assert!(stderr.contains("./scripts/build-release-source-archives.sh"));
    assert!(stderr.contains("./scripts/build-release-artifact.sh <target-triple>"));
}

#[test]
fn build_release_checksums_keeps_the_actionable_missing_inventory_error_under_system_bash() {
    let system_bash = Path::new("/bin/bash");
    if !system_bash.is_file() {
        return;
    }

    let repo = seed_release_script_repo();
    let repo_root = repo.path();

    let output = std::process::Command::new(system_bash)
        .arg(release_script_argument(
            repo_root,
            "build-release-checksums.sh",
        ))
        .current_dir(repo_root)
        .output()
        .expect("run checksum builder with system bash");

    assert!(
        !output.status.success(),
        "checksum builder should fail without assets"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
    assert!(stderr.contains("Populate ./dist with the full inventory first:"));
    assert!(stderr.contains("ffhn-source-9.9.9.zip"));
    assert!(stderr.contains("ffhn-9.9.9-aarch64-apple-darwin.tar.gz"));
}

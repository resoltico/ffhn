use std::fs;
use std::path::Path;
use std::process::Command;

use crate::model::{CommandSpec, DynResult};
use crate::plan::with_workspace_stub;

use super::command::{remove_file_if_exists, run_spec};

pub(crate) fn refresh_semver_baseline(repo_root: &Path, git_ref: &str) -> DynResult<()> {
    let workspace_manifest = git_file_contents(repo_root, git_ref, "Cargo.toml")?;
    let baseline_parent = repo_root.join("semver-baseline");
    let baseline_dir = baseline_parent.join("ffhn-core");
    let archive = baseline_parent.join("ffhn-core.tar.gz");

    if baseline_dir.exists() {
        fs::remove_dir_all(&baseline_dir)?;
    }
    remove_file_if_exists(&archive)?;
    fs::create_dir_all(&baseline_parent)?;

    run_spec(
        repo_root,
        &CommandSpec::new(
            "git",
            vec![
                "archive".to_owned(),
                "--format=tar.gz".to_owned(),
                "--prefix=ffhn-core/".to_owned(),
                "--output".to_owned(),
                archive.to_string_lossy().into_owned(),
                format!("{git_ref}:crates/ffhn-core"),
            ],
            false,
        ),
    )?;

    run_spec(
        repo_root,
        &CommandSpec::new(
            "tar",
            [
                "-xzf",
                archive.to_string_lossy().as_ref(),
                "-C",
                baseline_parent.to_string_lossy().as_ref(),
            ],
            false,
        ),
    )?;
    remove_file_if_exists(&archive)?;

    let baseline_manifest = baseline_dir.join("Cargo.toml");
    let cargo_toml = fs::read_to_string(&baseline_manifest)?;
    fs::write(
        &baseline_manifest,
        with_workspace_stub(&cargo_toml, &workspace_manifest)?,
    )?;
    Ok(())
}

fn git_file_contents(repo_root: &Path, git_ref: &str, relative_path: &str) -> DynResult<String> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .arg("show")
        .arg(format!("{git_ref}:{relative_path}"))
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(
            format!("failed to read {relative_path} from git ref {git_ref}: {stderr}").into(),
        );
    }

    String::from_utf8(output.stdout).map_err(|error| {
        format!("git returned non-UTF-8 contents for {relative_path}: {error}").into()
    })
}

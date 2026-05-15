use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::hygiene::prepare_artifact_layout;
use crate::model::{CommandSpec, DynResult};

const AMBIENT_NATIVE_TOOLCHAIN_ENV_VARS: [&str; 5] =
    ["CC", "CXX", "CLANG_BIN", "CPPFLAGS", "LDFLAGS"];

pub(crate) fn run_spec(repo_root: &Path, spec: &CommandSpec) -> DynResult<()> {
    let mut command = Command::new(&spec.program);
    command.current_dir(repo_root);
    command.args(&spec.args);
    command.stdin(Stdio::inherit());
    if spec.quiet_stdout {
        command.stdout(Stdio::null());
    } else {
        command.stdout(Stdio::inherit());
    }
    command.stderr(Stdio::inherit());
    for variable in AMBIENT_NATIVE_TOOLCHAIN_ENV_VARS {
        if !spec.env.contains_key(variable) {
            command.env_remove(variable);
        }
    }
    command.envs(&spec.env);
    apply_artifact_layout(&mut command, repo_root, spec)?;

    let status = command.status()?;
    if status.success() {
        return Ok(());
    }

    Err(format!("command failed with status {status}").into())
}

pub(crate) fn repo_root() -> DynResult<PathBuf> {
    #[cfg(test)]
    if let Some(repo_root) = env::var_os(TEST_REPO_ROOT_ENV) {
        return Ok(PathBuf::from(repo_root));
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask should live directly under the workspace root".into())
}

pub(crate) fn remove_dir_if_exists(path: &Path) -> DynResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }

    Ok(())
}

pub(crate) fn remove_file_if_exists(path: &Path) -> DynResult<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }

    Ok(())
}

fn apply_artifact_layout(
    command: &mut Command,
    repo_root: &Path,
    spec: &CommandSpec,
) -> DynResult<()> {
    let Some((target_root, build_root)) = prepare_artifact_layout(repo_root, spec.artifact_layout)?
    else {
        return Ok(());
    };

    if !spec.env.contains_key("CARGO_TARGET_DIR") {
        command.env("CARGO_TARGET_DIR", target_root);
    }
    if !spec.env.contains_key("CARGO_BUILD_BUILD_DIR") {
        command.env("CARGO_BUILD_BUILD_DIR", build_root);
    }

    Ok(())
}

#[cfg(test)]
pub(crate) const TEST_REPO_ROOT_ENV: &str = "FFHN_XTASK_TEST_REPO_ROOT";

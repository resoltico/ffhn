use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::model::{CommandSpec, DynResult};

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
    if spec.force_clang {
        command.env("CC", "clang");
    }
    command.envs(&spec.env);

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

#[cfg(test)]
pub(crate) const TEST_REPO_ROOT_ENV: &str = "FFHN_XTASK_TEST_REPO_ROOT";

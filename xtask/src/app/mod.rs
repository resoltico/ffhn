use std::ffi::OsString;

use clap::{Args, Parser, Subcommand};

use crate::model::DynResult;

mod check;
mod command;
mod semver;

#[cfg(test)]
pub(crate) use check::{run_check, run_coverage};
#[cfg(test)]
pub(crate) use command::{
    TEST_REPO_ROOT_ENV, remove_dir_if_exists, remove_file_if_exists, repo_root, run_spec,
};
#[cfg(test)]
pub(crate) use semver::refresh_semver_baseline;

const XTASK_NAME: &str = env!("CARGO_PKG_NAME");
const XTASK_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

#[derive(Parser)]
#[command(name = XTASK_NAME, version, about = XTASK_DESCRIPTION)]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Subcommand)]
enum Task {
    Check,
    Coverage,
    RefreshSemverBaseline(RefreshSemverBaselineArgs),
}

#[derive(Args)]
struct RefreshSemverBaselineArgs {
    #[arg(long, value_name = "REF")]
    git_ref: String,
}

/// Parses the xtask CLI and dispatches the selected maintenance action.
///
/// # Errors
///
/// Returns an error when the workspace root cannot be resolved or when the selected maintenance
/// action fails any required repository check.
pub fn run() -> DynResult<()> {
    run_from(std::env::args_os())
}

/// Parses one xtask CLI argument list and dispatches the selected maintenance action.
pub fn run_from<I, T>(args: I) -> DynResult<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    let repo_root = command::repo_root()?;

    match cli.command {
        Task::Check => check::run_check(&repo_root),
        Task::Coverage => check::run_coverage(&repo_root),
        Task::RefreshSemverBaseline(args) => {
            semver::refresh_semver_baseline(&repo_root, &args.git_ref)
        }
    }
}

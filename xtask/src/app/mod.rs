use std::ffi::OsString;

use clap::{Args, Parser, Subcommand};

use crate::model::DynResult;

mod check;
mod command;
mod hygiene;
mod semver;

#[cfg(all(test, unix))]
pub(crate) use check::{run_audit, run_check, run_coverage, run_semver_check};
#[cfg(test)]
pub(crate) use command::TEST_REPO_ROOT_ENV;
pub(crate) use command::remove_dir_if_exists;
#[cfg(all(test, unix))]
pub(crate) use command::run_spec;
#[cfg(test)]
pub(crate) use command::{remove_file_if_exists, repo_root};
pub(crate) use hygiene::HygieneTask;
#[cfg(test)]
pub(crate) use semver::refresh_semver_baseline;

const XTASK_NAME: &str = env!("CARGO_PKG_NAME");
const XTASK_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
const XTASK_AFTER_HELP: &str = "\
Examples:
  cargo xtask check
  cargo xtask audit
  cargo xtask audit --file fuzz/Cargo.lock
  cargo xtask semver-check
  cargo xtask coverage
  cargo xtask hygiene report
  cargo xtask hygiene clean --mode rebuildable
  cargo xtask refresh-semver-baseline --git-ref vX.Y.Z";

#[derive(Parser)]
#[command(
    name = XTASK_NAME,
    version,
    about = XTASK_DESCRIPTION,
    after_help = XTASK_AFTER_HELP
)]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Subcommand)]
enum Task {
    #[command(
        about = "Run the full maintainer quality gate.",
        long_about = "Run the full maintainer quality gate, including shell checks, formatting, dependency policy, tests, dist smoke, and the final curated 100% coverage pass."
    )]
    Check,
    #[command(
        about = "Run the maintained RustSec audit lane with transient advisory-fetch retries.",
        long_about = "Run the maintained RustSec audit lane with FFHN's bounded transient advisory-database fetch retry policy. By default this audits the workspace Cargo.lock; use --file to audit another maintained lockfile such as fuzz/Cargo.lock."
    )]
    Audit(AuditArgs),
    #[command(
        about = "Run only the maintained ffhn-core semver gate.",
        long_about = "Run only the maintained ffhn-core semver gate.\n\nThis lane uses the same baseline and release-type policy as cargo xtask check while skipping the rest of the maintainer suite."
    )]
    SemverCheck,
    #[command(
        about = "Run only the curated 100% coverage gate.",
        long_about = "Run the curated 100% line-and-branch coverage gate plus its prerequisite checks without rerunning the broader maintainer gate."
    )]
    Coverage,
    #[command(
        about = "Inspect or repair the repository artifact hygiene policy.",
        long_about = "Inspect or repair the maintained repository artifact hygiene policy, including managed Cargo artifact roots, repo-local scratch, and accidental legacy target trees."
    )]
    Hygiene {
        #[command(subcommand)]
        command: HygieneTask,
    },
    #[command(
        about = "Refresh the checked-in ffhn-core semver baseline.",
        long_about = "Refresh the checked-in ffhn-core semver baseline from one published Git tag, branch, or commit."
    )]
    RefreshSemverBaseline(RefreshSemverBaselineArgs),
}

#[derive(Args)]
struct RefreshSemverBaselineArgs {
    #[arg(
        long,
        value_name = "REF",
        help = "Published Git tag, branch, or commit to use as the semver baseline snapshot."
    )]
    git_ref: String,
}

#[derive(Args)]
struct AuditArgs {
    #[arg(
        long,
        value_name = "LOCKFILE",
        help = "Lockfile to audit. Defaults to the workspace Cargo.lock when omitted."
    )]
    file: Option<std::path::PathBuf>,
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
        Task::Audit(args) => check::run_audit(&repo_root, args.file.as_deref()),
        Task::SemverCheck => check::run_semver_check(&repo_root),
        Task::Coverage => check::run_coverage(&repo_root),
        Task::Hygiene { command } => hygiene::run_hygiene(&repo_root, command),
        Task::RefreshSemverBaseline(args) => {
            semver::refresh_semver_baseline(&repo_root, &args.git_ref)
        }
    }
}

use std::ffi::OsString;

use clap::{Args, Parser, Subcommand};

use crate::model::DynResult;

mod check;
mod command;
mod gate;
mod hygiene;
mod semver;
mod structure;

#[cfg(all(test, unix))]
pub(crate) use check::{run_audit, run_check, run_coverage, run_miri, run_semver_check};
#[cfg(test)]
pub(crate) use command::TEST_REPO_ROOT_ENV;
pub(crate) use command::remove_dir_if_exists;
#[cfg(all(test, unix))]
pub(crate) use command::run_spec;
#[cfg(test)]
pub(crate) use command::{remove_file_if_exists, repo_root};
pub(crate) use gate::{GateOutputFormat, GateOutputOptions, GateVerbosity};
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
  cargo xtask miri
  cargo xtask structure check
  cargo xtask structure report
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
    Check(GateOutputArgs),
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
        about = "Run only the maintained typed-observation strict-provenance Miri proof.",
        long_about = "Run only the maintained typed-observation strict-provenance Miri proof under the pinned nightly QA toolchain."
    )]
    Miri,
    #[command(
        about = "Verify or report FFHN's fail-closed Rust source-structure contract.",
        long_about = "Verify or report the repository-owned Rust source-structure contract. The check is fail-closed: every maintained Rust source file must have an ownership rule, stay within its declared budgets, and obey its internal dependency boundary."
    )]
    Structure {
        #[command(subcommand)]
        command: structure::StructureTask,
    },
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
struct GateOutputArgs {
    /// Render lifecycle events for people or newline-delimited JSON for automation.
    #[arg(long, value_enum, default_value_t = GateOutputFormat::Human)]
    format: GateOutputFormat,
    /// Show only actionable signal or stream every child-process byte.
    #[arg(long, value_enum, default_value_t = GateVerbosity::Concise)]
    verbosity: GateVerbosity,
    /// Retain raw successful-run logs under this caller-owned directory.
    #[arg(long, value_name = "DIRECTORY")]
    log_dir: Option<std::path::PathBuf>,
    /// Keep raw evidence after a successful run; failed-run evidence is always retained.
    #[arg(long)]
    retain_passing_logs: bool,
}

impl From<GateOutputArgs> for GateOutputOptions {
    fn from(args: GateOutputArgs) -> Self {
        Self {
            format: args.format,
            verbosity: args.verbosity,
            log_dir: args.log_dir,
            retain_passing_logs: args.retain_passing_logs,
        }
    }
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
        Task::Check(args) => check::run_check(&repo_root, args.into()),
        Task::Audit(args) => check::run_audit(&repo_root, args.file.as_deref()),
        Task::SemverCheck => check::run_semver_check(&repo_root),
        Task::Coverage => check::run_coverage(&repo_root),
        Task::Miri => check::run_miri(&repo_root),
        Task::Structure { command } => structure::run_structure(&repo_root, command),
        Task::Hygiene { command } => hygiene::run_hygiene(&repo_root, command),
        Task::RefreshSemverBaseline(args) => {
            semver::refresh_semver_baseline(&repo_root, &args.git_ref)
        }
    }
}

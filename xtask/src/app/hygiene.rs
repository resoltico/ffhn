use std::path::Path;

use crate::hygiene::{
    HygieneCleanMode, HygieneReportFormat, clean_hygiene, ensure_hygiene, hygiene_report,
    render_hygiene_report,
};
use crate::model::DynResult;

#[derive(clap::Subcommand)]
pub(crate) enum HygieneTask {
    #[command(
        about = "Report the current artifact inventory.",
        long_about = "Render the current repository artifact inventory, including managed Cargo caches, repo-local scratch, budgets, and policy violations."
    )]
    Report {
        /// Output format for the report.
        #[arg(long = "format", value_enum, default_value_t = HygieneReportFormat::Text)]
        format: HygieneReportFormat,
    },
    #[command(
        about = "Fail if the current artifact inventory violates policy.",
        long_about = "Fail if the current repository artifact inventory violates the maintained hygiene policy."
    )]
    Verify,
    #[command(
        about = "Delete disposable artifact roots.",
        long_about = "Delete disposable artifact roots. `safe` removes repo-local temporary workspace state, legacy repo-local target trees, and other disposable scratch; `rebuildable` also deletes the managed Cargo caches."
    )]
    Clean {
        /// Cleanup profile.
        #[arg(long = "mode", value_enum, default_value_t = HygieneCleanMode::Safe)]
        mode: HygieneCleanMode,
    },
}

pub(crate) fn run_hygiene(repo_root: &Path, command: HygieneTask) -> DynResult<()> {
    match command {
        HygieneTask::Report { format } => {
            let report = hygiene_report(repo_root)?;
            match format {
                HygieneReportFormat::Text => println!("{}", render_hygiene_report(&report)),
                HygieneReportFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
            }
            Ok(())
        }
        HygieneTask::Verify => ensure_hygiene(repo_root),
        HygieneTask::Clean { mode } => {
            let result = clean_hygiene(repo_root, mode)?;
            println!(
                "Removed {} artifact roots and reclaimed {} bytes.",
                result.removed_paths.len(),
                result.reclaimed_bytes
            );
            for path in result.removed_paths {
                println!("- {}", path.display());
            }
            Ok(())
        }
    }
}

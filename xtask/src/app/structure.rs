use std::path::Path;

use clap::Subcommand;

use crate::model::DynResult;

/// Selects whether to enforce or inspect the Rust source-structure contract.
#[derive(Subcommand)]
pub(crate) enum StructureTask {
    /// Fail when a maintained Rust source file has no role, exceeds a budget, or crosses a boundary.
    Check,
    /// Print the measured shape and resolved role of every maintained Rust source file.
    Report,
}

/// Runs the selected Rust source-structure action.
pub(crate) fn run_structure(repo_root: &Path, task: StructureTask) -> DynResult<()> {
    match task {
        StructureTask::Check => crate::structure::check(repo_root),
        StructureTask::Report => crate::structure::report(repo_root),
    }
}

//! Shared maintenance helpers behind `cargo xtask`.
#![deny(missing_docs)]

mod app;
mod coverage;
mod model;
mod plan;
#[cfg(test)]
mod repo_contract;
mod repo_files;
#[cfg(test)]
mod tests;

pub use app::{run, run_from};
pub use model::DynResult;

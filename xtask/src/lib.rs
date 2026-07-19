//! Shared maintenance helpers behind `cargo xtask`.
#![cfg_attr(not(test), forbid(unsafe_code))]
#![deny(missing_docs)]

mod app;
mod coverage;
mod hygiene;
mod miri;
mod model;
mod plan;
#[cfg(test)]
mod release;
#[cfg(test)]
mod repo_contract;
mod repo_files;
mod structure;
#[cfg(test)]
mod tests;
mod tooling;

pub use app::{run, run_from};
pub use model::DynResult;

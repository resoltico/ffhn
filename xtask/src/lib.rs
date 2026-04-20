//! Shared maintenance helpers behind `cargo xtask`.
#![deny(missing_docs)]

mod app;
mod coverage;
mod model;
mod plan;
#[cfg(test)]
mod repo_contract;
#[cfg(test)]
mod tests;

pub use app::run;
pub use model::DynResult;

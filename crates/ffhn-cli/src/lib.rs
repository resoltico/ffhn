//! Thin command-line adapter for the FFHN monitoring engine.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Exit code for structured failed FFHN runs.
pub const EXIT_CODE_RUN_FAILED: i32 = 1;
/// Exit code for CLI misuse.
pub const EXIT_CODE_USAGE: i32 = 2;
/// Exit code for fatal process-level failures before document emission.
pub const EXIT_CODE_FATAL: i32 = 3;
/// Exit code when an exclusive source or graph lease is already held.
pub const EXIT_CODE_BUSY: i32 = 4;

mod args;
mod error;
mod execute;
mod render;

pub use execute::run;

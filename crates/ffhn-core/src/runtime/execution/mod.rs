//! Runtime execution decomposed into single-target control flow, lifecycle staging/commit, and batch/status operations.

mod control;
mod lifecycle;
mod status;
#[cfg(test)]
mod tests;

pub(crate) use control::{reset, run_once, run_once_with_mode, validate_target};
#[cfg(test)]
pub(super) use control::{run_once_with_stager, run_once_with_stager_and_permanent_error_resolver};
#[cfg(test)]
pub(super) use lifecycle::*;
#[cfg(test)]
pub(in crate::runtime) use status::status_with_shared_lock_for_test;
pub(crate) use status::{run_batch, status};

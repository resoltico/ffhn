use std::thread;

use crate::model::{StatusReportParts, plain_detail};
use crate::{
    BatchRunReport, CoreError, DiagnosticKind, DiagnosticOperation, RunMode, StatusKind,
    StatusReport, TargetPaths,
};

use super::super::lock::lock_shared;
use super::super::report::{detail_from_error_for_operation, target_load_status_kind};
use super::super::storage::{load_state, load_target};
use super::control::run_once_with_mode;

/// Runs the supplied targets in bounded parallel groups.
pub(crate) fn run_batch(
    paths: Vec<TargetPaths>,
    mode: RunMode,
    jobs: usize,
) -> Result<BatchRunReport, CoreError> {
    if jobs == 0 {
        return Err(CoreError::contract("batch jobs must be positive"));
    }
    let requested_targets = paths
        .iter()
        .map(|path| path.target_id().to_owned())
        .collect::<Vec<_>>();
    let mut reports = Vec::with_capacity(paths.len());
    for group in paths.chunks(jobs) {
        let handles = group
            .iter()
            .cloned()
            .map(|path| thread::spawn(move || run_once_with_mode(&path, mode)))
            .collect::<Vec<_>>();
        for handle in handles {
            reports.push(
                handle
                    .join()
                    .map_err(|_| CoreError::internal("target-run worker panicked"))??,
            );
        }
    }
    Ok(BatchRunReport::new(mode, requested_targets, reports))
}

/// Returns one stable target status.
pub(crate) fn status(paths: &TargetPaths) -> Result<StatusReport, CoreError> {
    status_with_shared_lock_impl(paths, lock_shared)
}

/// Runs status through an injectable lock boundary used only by the concurrency contract test.
///
/// The production path always invokes [`status`], which supplies [`lock_shared`]. The test
/// boundary exposes only the lock acquisition so its retry observer can establish that a real
/// exclusive holder prevented status from reading persisted state.
#[cfg(test)]
pub(in crate::runtime) fn status_with_shared_lock_for_test<F>(
    paths: &TargetPaths,
    acquire_shared_lock: F,
) -> Result<StatusReport, CoreError>
where
    F: FnOnce(&TargetPaths) -> Result<super::super::lock::TargetLock, CoreError>,
{
    status_with_shared_lock_impl(paths, acquire_shared_lock)
}

/// Returns one stable target status after the caller supplies the shared lock operation.
fn status_with_shared_lock_impl<F>(
    paths: &TargetPaths,
    acquire_shared_lock: F,
) -> Result<StatusReport, CoreError>
where
    F: FnOnce(&TargetPaths) -> Result<super::super::lock::TargetLock, CoreError>,
{
    paths.require_watch_root_directory()?;
    let target = match load_target(paths) {
        Ok(target) => target,
        Err(error) => {
            return StatusReport::new(StatusReportParts {
                target_id: paths.target_id().to_owned(),
                kind: target_load_status_kind(&error),
                display_name: None,
                enabled: None,
                digest: None,
                observation: None,
                error: Some(detail_from_error_for_operation(
                    &error,
                    DiagnosticOperation::TargetLoad,
                    Some(paths.target_file()),
                )),
                lifecycle: None,
            });
        }
    };
    if let Err(error) = target.validate_without_projection() {
        return StatusReport::new(StatusReportParts {
            target_id: paths.target_id().to_owned(),
            kind: target_load_status_kind(&error),
            display_name: None,
            enabled: None,
            digest: None,
            observation: None,
            error: Some(detail_from_error_for_operation(
                &error,
                DiagnosticOperation::TargetValidation,
                Some(paths.target_file()),
            )),
            lifecycle: None,
        });
    }
    let digest = target.contract_digest_sha256()?;
    let _lock = match acquire_shared_lock(paths) {
        Ok(lock) => lock,
        Err(error) => {
            return StatusReport::new(StatusReportParts {
                target_id: paths.target_id().to_owned(),
                kind: StatusKind::InvalidState,
                display_name: Some(target.display_name().to_owned()),
                enabled: Some(target.enabled()),
                digest: Some(digest),
                observation: None,
                error: Some(detail_from_error_for_operation(
                    &error,
                    DiagnosticOperation::LockAcquire,
                    Some(paths.run_lock_file()),
                )),
                lifecycle: None,
            });
        }
    };
    match load_state(paths) {
        Ok(None) => match target.validate() {
            Ok(()) => StatusReport::new(StatusReportParts {
                target_id: paths.target_id().to_owned(),
                kind: StatusKind::Pending,
                display_name: Some(target.display_name().to_owned()),
                enabled: Some(target.enabled()),
                digest: Some(digest),
                observation: None,
                error: None,
                lifecycle: None,
            }),
            Err(error) => StatusReport::new(StatusReportParts {
                target_id: paths.target_id().to_owned(),
                kind: StatusKind::InvalidConfig,
                display_name: Some(target.display_name().to_owned()),
                enabled: Some(target.enabled()),
                digest: Some(digest),
                observation: None,
                error: Some(detail_from_error_for_operation(
                    &error,
                    DiagnosticOperation::TargetValidation,
                    Some(paths.target_file()),
                )),
                lifecycle: None,
            }),
        },
        Ok(Some(state)) if state.contract_digest_sha256() == digest => {
            if let Err(error) = state.validate_for_target(&target) {
                return StatusReport::new(StatusReportParts {
                    target_id: paths.target_id().to_owned(),
                    kind: StatusKind::InvalidState,
                    display_name: Some(target.display_name().to_owned()),
                    enabled: Some(target.enabled()),
                    digest: Some(digest),
                    observation: None,
                    error: Some(detail_from_error_for_operation(
                        &error,
                        DiagnosticOperation::StateLoad,
                        Some(paths.state_file()),
                    )),
                    lifecycle: None,
                });
            }
            let lifecycle = state.lifecycle_snapshot()?;
            if let Err(error) = target.validate() {
                return StatusReport::new(StatusReportParts {
                    target_id: paths.target_id().to_owned(),
                    kind: StatusKind::InvalidConfig,
                    display_name: Some(target.display_name().to_owned()),
                    enabled: Some(target.enabled()),
                    digest: Some(digest),
                    observation: None,
                    error: Some(detail_from_error_for_operation(
                        &error,
                        DiagnosticOperation::TargetValidation,
                        Some(paths.target_file()),
                    )),
                    lifecycle: Some(lifecycle),
                });
            }
            StatusReport::new(StatusReportParts {
                target_id: paths.target_id().to_owned(),
                kind: if state.accepted_observation().is_some() {
                    StatusKind::Ready
                } else {
                    StatusKind::Pending
                },
                display_name: Some(target.display_name().to_owned()),
                enabled: Some(target.enabled()),
                digest: Some(digest),
                observation: state.accepted_observation().cloned(),
                error: None,
                lifecycle: Some(lifecycle),
            })
        }
        Ok(Some(_)) => StatusReport::new(StatusReportParts {
            target_id: paths.target_id().to_owned(),
            kind: StatusKind::InvalidState,
            display_name: Some(target.display_name().to_owned()),
            enabled: Some(target.enabled()),
            digest: Some(digest),
            observation: None,
            error: Some(plain_detail(
                DiagnosticKind::Contract,
                DiagnosticOperation::StateLoad,
                "stored state belongs to a different measurement contract; reset is required",
                Some(paths.state_file().display().to_string()),
            )),
            lifecycle: None,
        }),
        Err(error) => StatusReport::new(StatusReportParts {
            target_id: paths.target_id().to_owned(),
            kind: StatusKind::InvalidState,
            display_name: Some(target.display_name().to_owned()),
            enabled: Some(target.enabled()),
            digest: Some(digest),
            observation: None,
            error: Some(detail_from_error_for_operation(
                &error,
                DiagnosticOperation::StateLoad,
                Some(paths.state_file()),
            )),
            lifecycle: None,
        }),
    }
}

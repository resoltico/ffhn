use crate::{CoreError, LastRunRecord, RunFailureCause, RunOutcome, TargetDocument, TargetPaths};

use super::super::domain::{PersistedBaselineState, PersistedState};
use super::super::state::{StateLoad, prior_valid_state};
use super::super::storage::write_json;

pub(crate) fn persist_state_only(
    paths: &TargetPaths,
    target: &TargetDocument,
    prior_state: &StateLoad,
    run_outcome: RunOutcome,
    failure_cause: Option<RunFailureCause>,
    run_started_at: &str,
) -> Result<(bool, Option<PersistedState>), CoreError> {
    #[rustfmt::skip]
    let state = build_state_update(target, prior_state, run_outcome, failure_cause, run_started_at)?;
    let Some(state) = state else {
        return Ok((false, None));
    };
    let state_document = state.to_document(target.target_id.clone())?;
    write_json(paths.state_file(), &state_document)?;
    Ok((true, Some(state)))
}

fn build_state_update(
    target: &TargetDocument,
    prior_state: &StateLoad,
    outcome: RunOutcome,
    cause: Option<RunFailureCause>,
    started_at: &str,
) -> Result<Option<PersistedState>, CoreError> {
    build_state_without_snapshot_changes(target, prior_state, outcome, cause, started_at)
}

fn build_state_without_snapshot_changes(
    target: &TargetDocument,
    prior_state: &StateLoad,
    run_outcome: RunOutcome,
    failure_cause: Option<RunFailureCause>,
    run_started_at: &str,
) -> Result<Option<PersistedState>, CoreError> {
    let last_run = build_last_run_record(run_started_at, run_outcome, failure_cause)?;
    match prior_valid_state(prior_state) {
        Some(prior) => Ok(Some(prior.state.clone().with_last_run(last_run))),
        None if run_outcome == RunOutcome::SkippedDisabled => {
            let state = PersistedState {
                monitoring_contract_digest_sha256: target.monitoring_contract_digest_sha256()?,
                baseline: PersistedBaselineState::Pending,
                last_run: Some(last_run),
            };
            Ok(Some(state))
        }
        None => Ok(None),
    }
}

fn build_last_run_record(
    run_started_at: &str,
    run_outcome: RunOutcome,
    failure_cause: Option<RunFailureCause>,
) -> Result<LastRunRecord, CoreError> {
    match run_outcome {
        RunOutcome::FailedTransient => {
            let cause = failure_cause.ok_or_else(|| {
                CoreError::internal("failed_transient persisted last_run requires a failure cause")
            })?;
            Ok(LastRunRecord::new(
                run_started_at.to_owned(),
                run_outcome,
                Some(cause),
            ))
        }
        RunOutcome::FailedPermanent => {
            let cause = failure_cause.ok_or_else(|| {
                CoreError::internal("failed_permanent persisted last_run requires a failure cause")
            })?;
            Ok(LastRunRecord::new(
                run_started_at.to_owned(),
                run_outcome,
                Some(cause),
            ))
        }
        RunOutcome::Initialized
        | RunOutcome::Changed
        | RunOutcome::Unchanged
        | RunOutcome::SkippedDisabled => Ok(LastRunRecord::new(
            run_started_at.to_owned(),
            run_outcome,
            None,
        )),
    }
}

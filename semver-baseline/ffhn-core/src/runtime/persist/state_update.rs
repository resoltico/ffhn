use crate::{
    CoreError, ReasonCode, RunOutcome, STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION, StateDocument,
    StatePhase, TargetDocument, TargetPaths,
};

use super::super::state::{StateLoad, prior_valid_state};
use super::super::storage::write_json;

pub(crate) fn persist_state_only(
    paths: &TargetPaths,
    target: &TargetDocument,
    prior_state: &StateLoad,
    run_outcome: RunOutcome,
    reason_code: ReasonCode,
    run_started_at: &str,
) -> Result<(bool, Option<StateDocument>), CoreError> {
    #[rustfmt::skip]
    let state = build_state_update(target, prior_state, run_outcome, reason_code, run_started_at)?;
    let Some(state) = state else {
        return Ok((false, None));
    };
    write_json(paths.state_file(), &state)?;
    Ok((true, Some(state)))
}

fn build_state_update(
    target: &TargetDocument,
    prior_state: &StateLoad,
    outcome: RunOutcome,
    reason: ReasonCode,
    started_at: &str,
) -> Result<Option<StateDocument>, CoreError> {
    build_state_without_snapshot_changes(target, prior_state, outcome, reason, started_at)
}

fn build_state_without_snapshot_changes(
    target: &TargetDocument,
    prior_state: &StateLoad,
    run_outcome: RunOutcome,
    reason_code: ReasonCode,
    run_started_at: &str,
) -> Result<Option<StateDocument>, CoreError> {
    match prior_valid_state(prior_state) {
        Some(prior) => {
            let mut state = prior.document.clone();
            state.last_run_at = Some(run_started_at.to_owned());
            state.last_run_outcome = Some(run_outcome);
            state.last_reason_code = Some(reason_code);
            state.validate()?;
            Ok(Some(state))
        }
        None if run_outcome == RunOutcome::SkippedDisabled => {
            let state = StateDocument {
                schema_name: STATE_SCHEMA_NAME.to_owned(),
                schema_version: STATE_SCHEMA_VERSION,
                target_id: target.target_id.clone(),
                state_phase: StatePhase::NeverSucceeded,
                last_run_at: Some(run_started_at.to_owned()),
                last_run_outcome: Some(run_outcome),
                last_reason_code: Some(reason_code),
                current_snapshot: None,
                snapshot_history: Vec::new(),
                extensions: None,
            };
            state.validate()?;
            Ok(Some(state))
        }
        None => Ok(None),
    }
}

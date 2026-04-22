use crate::stable_json::stable_json;
use crate::{
    CoreError, ExtractionRecord, ReasonCode, RunOutcome, RunReport, STATE_SCHEMA_NAME,
    STATE_SCHEMA_VERSION, StateDocument, StatePhase, TargetDocument, TargetPaths,
};

use super::super::state::{StateLoad, prior_valid_state};
use super::super::storage::write_json;
use super::snapshot_store::{
    archive_current_snapshot, clear_dir_if_exists, prune_history, write_new_current_snapshot,
};

pub(crate) struct SuccessfulPersistInput<'a> {
    pub(crate) target: &'a TargetDocument,
    pub(crate) prior_state: &'a StateLoad,
    pub(crate) run_started_at: &'a str,
    pub(crate) run_outcome: RunOutcome,
    pub(crate) canonical_text: &'a str,
    pub(crate) outer_html: &'a str,
    pub(crate) extraction_record: &'a ExtractionRecord,
}

pub(crate) fn persist_successful_run(
    paths: &TargetPaths,
    input: SuccessfulPersistInput<'_>,
) -> Result<Option<StateDocument>, CoreError> {
    let extraction_json = stable_json(input.extraction_record)?;
    let prior = prior_valid_state(input.prior_state);
    let history_limit = input.target.storage.history_limit;

    let (current_snapshot, snapshot_history) = match input.run_outcome {
        RunOutcome::Initialized => {
            clear_dir_if_exists(&paths.history_snapshots_dir())?;
            let current_reference = write_new_current_snapshot(
                paths,
                input.canonical_text,
                input.outer_html,
                &extraction_json,
            )?;
            (Some(current_reference), Vec::new())
        }
        RunOutcome::Changed => {
            let current_reference = write_new_current_snapshot(
                paths,
                input.canonical_text,
                input.outer_html,
                &extraction_json,
            )?;
            let mut snapshot_history = prior
                .map(|state| state.document.snapshot_history.clone())
                .unwrap_or_default();
            if let Some(previous_current) = prior.and_then(|state| state.current.as_ref()) {
                let archived = archive_current_snapshot(paths, previous_current)?;
                snapshot_history.insert(0, archived);
            }
            prune_history(paths, &mut snapshot_history, history_limit)?;
            (Some(current_reference), snapshot_history)
        }
        RunOutcome::Unchanged => (
            prior.and_then(|state| state.document.current_snapshot.clone()),
            prior
                .map(|state| state.document.snapshot_history.clone())
                .unwrap_or_default(),
        ),
        RunOutcome::FailedTransient | RunOutcome::FailedPermanent | RunOutcome::SkippedDisabled => {
            return Err(CoreError::htmlcut(
                "persist_successful_run only supports successful outcomes",
            ));
        }
    };

    let state = StateDocument {
        schema_name: STATE_SCHEMA_NAME.to_owned(),
        schema_version: STATE_SCHEMA_VERSION,
        target_id: input.target.target_id.clone(),
        state_phase: StatePhase::HasBaseline,
        last_run_at: Some(input.run_started_at.to_owned()),
        last_run_outcome: Some(input.run_outcome),
        last_reason_code: Some(ReasonCode::Ok),
        current_snapshot,
        snapshot_history,
        extensions: None,
    };
    state.validate()?;
    write_json(paths.state_file(), &state)?;
    Ok(Some(state))
}

pub(crate) fn write_last_run(paths: &TargetPaths, report: &RunReport) -> Result<(), CoreError> {
    write_json(paths.last_run_file(), report)
}

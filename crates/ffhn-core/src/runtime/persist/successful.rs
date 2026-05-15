use crate::stable_json::stable_json;
use crate::{
    CoreError, ExtractionRecord, LastRunRecord, LastRunSnapshot, RunOutcome, RunReport,
    TargetDocument, TargetPaths,
};

use super::super::domain::{PersistedBaselineState, PersistedState};
use super::super::state::{StateLoad, prior_valid_state};
use super::super::storage::write_json;
use super::snapshot_store::{stage_current_snapshot, stage_history_snapshot};
use super::transaction::SnapshotPersistPlan;

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
) -> Result<Option<PersistedState>, CoreError> {
    let extraction_json = stable_json(input.extraction_record)?;
    let prior = prior_valid_state(input.prior_state);
    let history_limit = input.target.storage.history_limit;

    let plan = match input.run_outcome {
        RunOutcome::Initialized => {
            let (staged_current_dir, current_reference) = stage_current_snapshot(
                paths,
                input.canonical_text,
                input.outer_html,
                &extraction_json,
            )?;
            SnapshotPersistPlan::with_staged_current(
                current_reference,
                Vec::new(),
                staged_current_dir,
                Vec::new(),
                Vec::new(),
                true,
            )
        }
        RunOutcome::Changed => prepare_changed_snapshot_plan(
            paths,
            prior,
            history_limit,
            input.canonical_text,
            input.outer_html,
            &extraction_json,
        )?,
        RunOutcome::Unchanged => SnapshotPersistPlan::unchanged(
            prior.and_then(|state| state.state.current_snapshot().cloned()),
            prior
                .map(|state| state.state.snapshot_history().to_vec())
                .unwrap_or_default(),
        ),
        RunOutcome::FailedTransient | RunOutcome::FailedPermanent | RunOutcome::SkippedDisabled => {
            return Err(CoreError::internal(
                "persist_successful_run only supports successful outcomes",
            ));
        }
    };

    let state = PersistedState {
        baseline: PersistedBaselineState::Ready {
            current_snapshot: plan
                .current_snapshot
                .clone()
                .expect("successful persistence requires a current snapshot"),
            snapshot_history: plan.snapshot_history.clone(),
        },
        last_run: Some(LastRunRecord::new(
            input.run_started_at.to_owned(),
            input.run_outcome,
            None,
        )),
    };
    let state_document = state.to_document(input.target.target_id.clone())?;
    plan.commit(paths, &state_document)?;
    Ok(Some(state))
}

pub(crate) fn write_last_run(paths: &TargetPaths, report: &RunReport) -> Result<(), CoreError> {
    let snapshot = LastRunSnapshot::new(report.clone())?;
    write_json(paths.last_run_file(), &snapshot)
}

fn prepare_changed_snapshot_plan(
    paths: &TargetPaths,
    prior: Option<&super::super::state::LoadedState>,
    history_limit: usize,
    canonical_text: &str,
    outer_html: &str,
    extraction_json: &str,
) -> Result<SnapshotPersistPlan, CoreError> {
    let (staged_current_dir, current_reference) =
        stage_current_snapshot(paths, canonical_text, outer_html, extraction_json)?;
    let staged_history_snapshots = prior
        .and_then(|state| state.current.as_ref())
        .map(|previous_current| stage_history_snapshot(paths, previous_current))
        .transpose()?;
    let (snapshot_history, pruned_snapshots) = rotate_snapshot_history(
        prior
            .map(|state| state.state.snapshot_history().to_vec())
            .unwrap_or_default(),
        staged_history_snapshots
            .as_ref()
            .map(|snapshot| snapshot.reference.clone()),
        history_limit,
    );
    Ok(SnapshotPersistPlan::with_staged_current(
        current_reference,
        snapshot_history,
        staged_current_dir,
        staged_history_snapshots.into_iter().collect(),
        pruned_snapshots,
        prior.is_none(),
    ))
}

fn rotate_snapshot_history(
    mut snapshot_history: Vec<crate::SnapshotReference>,
    archived_snapshot: Option<crate::SnapshotReference>,
    history_limit: usize,
) -> (Vec<crate::SnapshotReference>, Vec<crate::SnapshotReference>) {
    if let Some(archived_snapshot) = archived_snapshot {
        snapshot_history.insert(0, archived_snapshot);
    }
    let max_history_entries = history_limit.saturating_sub(1);
    let drain_from = max_history_entries.min(snapshot_history.len());
    let pruned_snapshots = snapshot_history.drain(drain_from..).collect::<Vec<_>>();
    (snapshot_history, pruned_snapshots)
}

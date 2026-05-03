use super::super::schema::{STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION};
use super::super::validate::{parse_timestamp, validate_identity, validate_timestamp};
use super::*;

impl StateDocument {
    /// Validates one state document.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the schema identity, last-run triplet, state phase, snapshot
    /// slots, snapshot ordering, or outcome/reason coupling violates FFHN's frozen state
    /// contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        let schema_name = &self.schema_name;
        let schema_version = self.schema_version;
        validate_identity(
            schema_name,
            STATE_SCHEMA_NAME,
            schema_version,
            STATE_SCHEMA_VERSION,
        )?;
        if let Some(last_run_at) = &self.last_run_at {
            validate_timestamp(last_run_at)?;
        }
        validate_last_run_triplet(
            self.last_run_at.is_some(),
            self.last_run_outcome,
            self.last_reason_code,
        )?;
        match self.state_phase {
            StatePhase::NeverSucceeded => {
                if self.current_snapshot.is_some() {
                    return Err(CoreError::contract(
                        "state_phase never_succeeded requires null snapshots",
                    ));
                }
                if !self.snapshot_history.is_empty() {
                    return Err(CoreError::contract(
                        "state_phase never_succeeded requires null snapshots",
                    ));
                }
            }
            StatePhase::HasBaseline => {
                if self.current_snapshot.is_none() {
                    return Err(CoreError::contract(
                        "state_phase has_baseline requires current_snapshot",
                    ));
                }
            }
        }
        if let Some(snapshot) = &self.current_snapshot {
            snapshot.validate()?;
            if snapshot.slot != SnapshotSlot::Current {
                return Err(CoreError::contract("current_snapshot.slot must be current"));
            }
        }
        for snapshot in &self.snapshot_history {
            snapshot.validate()?;
            if snapshot.slot != SnapshotSlot::History {
                return Err(CoreError::contract(
                    "snapshot_history entries must use slot = history",
                ));
            }
        }
        validate_snapshot_history_order(&self.snapshot_history)?;
        if let (Some(current_snapshot), Some(previous_snapshot)) =
            (&self.current_snapshot, self.snapshot_history.first())
        {
            let current_captured_at = parse_timestamp(&current_snapshot.captured_at)?;
            let previous_captured_at = parse_timestamp(&previous_snapshot.captured_at)?;
            if current_captured_at < previous_captured_at {
                return Err(CoreError::contract(
                    "current_snapshot must be at least as recent as snapshot_history[0]",
                ));
            }
        }
        Ok(())
    }
}

fn validate_last_run_triplet(
    has_last_run_at: bool,
    last_run_outcome: Option<RunOutcome>,
    last_reason_code: Option<ReasonCode>,
) -> Result<(), CoreError> {
    match (has_last_run_at, last_run_outcome, last_reason_code) {
        (false, None, None) => Ok(()),
        (true, Some(run_outcome), Some(reason_code)) => {
            validate_run_outcome_reason_pair(run_outcome, reason_code)
        }
        _ => Err(CoreError::contract(
            "state last-run fields must be all present or all absent",
        )),
    }
}

fn validate_run_outcome_reason_pair(
    run_outcome: RunOutcome,
    reason_code: ReasonCode,
) -> Result<(), CoreError> {
    match run_outcome {
        RunOutcome::Initialized | RunOutcome::Changed | RunOutcome::Unchanged => {
            if reason_code != ReasonCode::Ok {
                return Err(CoreError::contract(
                    "successful state outcomes require last_reason_code = ok",
                ));
            }
        }
        RunOutcome::SkippedDisabled => {
            if reason_code != ReasonCode::Disabled {
                return Err(CoreError::contract(
                    "skipped_disabled state outcomes require last_reason_code = disabled",
                ));
            }
        }
        RunOutcome::FailedTransient => {
            if reason_code.failure_class() != Some(crate::FailureClass::Transient) {
                return Err(CoreError::contract(
                    "failed_transient state outcomes require a transient last_reason_code",
                ));
            }
        }
        RunOutcome::FailedPermanent => {
            if reason_code.failure_class() != Some(crate::FailureClass::Permanent) {
                return Err(CoreError::contract(
                    "failed_permanent state outcomes require a permanent last_reason_code",
                ));
            }
        }
    }
    Ok(())
}

fn validate_snapshot_history_order(
    snapshot_history: &[SnapshotReference],
) -> Result<(), CoreError> {
    let mut previous_captured_at = None;
    for snapshot in snapshot_history {
        let captured_at = parse_timestamp(&snapshot.captured_at)?;
        if let Some(previous) = previous_captured_at
            && captured_at > previous
        {
            return Err(CoreError::contract(
                "snapshot_history must be ordered newest first",
            ));
        }
        previous_captured_at = Some(captured_at);
    }
    Ok(())
}

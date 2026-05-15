use super::super::schema::{STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION};
use super::super::validate::{parse_timestamp, validate_identity, validate_timestamp};
use super::*;

impl StateDocument {
    /// Validates one state document.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the schema identity, stored baseline, snapshot ordering, or
    /// last-run summary violates FFHN's frozen state contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_identity(
            &self.schema_name,
            STATE_SCHEMA_NAME,
            self.schema_version,
            STATE_SCHEMA_VERSION,
        )?;
        self.baseline.validate()?;
        self.last_run
            .as_ref()
            .map(LastRunRecord::validate)
            .transpose()?;
        validate_state_shape(self)?;
        Ok(())
    }
}

impl StoredBaseline {
    fn validate(&self) -> Result<(), CoreError> {
        match self {
            Self::Pending => Ok(()),
            Self::Ready {
                current_snapshot,
                snapshot_history,
            } => {
                current_snapshot.validate()?;
                if current_snapshot.slot != SnapshotSlot::Current {
                    return Err(CoreError::contract(
                        "baseline.ready current_snapshot.slot must be current",
                    ));
                }
                for snapshot in snapshot_history {
                    snapshot.validate()?;
                    if snapshot.slot != SnapshotSlot::History {
                        return Err(CoreError::contract(
                            "baseline.ready snapshot_history entries must use slot = history",
                        ));
                    }
                }
                validate_snapshot_history_order(snapshot_history)?;
                if let Some(previous_snapshot) = snapshot_history.first() {
                    let current_captured_at = parse_timestamp(&current_snapshot.captured_at)?;
                    let previous_captured_at = parse_timestamp(&previous_snapshot.captured_at)?;
                    if current_captured_at < previous_captured_at {
                        return Err(CoreError::contract(
                            "baseline.ready current_snapshot must be at least as recent as snapshot_history[0]",
                        ));
                    }
                }
                Ok(())
            }
        }
    }
}

impl LastRunRecord {
    fn validate(&self) -> Result<(), CoreError> {
        validate_timestamp(self.run_at())?;
        match (self.outcome(), self.failure_cause()) {
            (crate::RunOutcome::FailedTransient, Some(cause))
                if cause.failure_class() == crate::FailureClass::Transient => {}
            (crate::RunOutcome::FailedPermanent, Some(cause))
                if cause.failure_class() == crate::FailureClass::Permanent => {}
            (crate::RunOutcome::FailedTransient, _) => {
                return Err(CoreError::contract(
                    "failed_transient last_run entries require a transient cause",
                ));
            }
            (crate::RunOutcome::FailedPermanent, _) => {
                return Err(CoreError::contract(
                    "failed_permanent last_run entries require a permanent cause",
                ));
            }
            (
                crate::RunOutcome::Initialized
                | crate::RunOutcome::Changed
                | crate::RunOutcome::Unchanged
                | crate::RunOutcome::SkippedDisabled,
                Some(_),
            ) => {
                return Err(CoreError::contract(
                    "successful or skipped last_run entries must not carry a failure cause",
                ));
            }
            _ => {}
        }
        Ok(())
    }
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
                "baseline.ready snapshot_history must be ordered newest first",
            ));
        }
        previous_captured_at = Some(captured_at);
    }
    Ok(())
}

fn validate_state_shape(state: &StateDocument) -> Result<(), CoreError> {
    match (&state.baseline, state.last_run.as_ref()) {
        (StoredBaseline::Pending, Some(last_run))
            if matches!(
                last_run.outcome(),
                crate::RunOutcome::Initialized
                    | crate::RunOutcome::Changed
                    | crate::RunOutcome::Unchanged
            ) =>
        {
            Err(CoreError::contract(
                "baseline.pending cannot carry a successful last_run",
            ))
        }
        (StoredBaseline::Ready { .. }, None) => {
            Err(CoreError::contract("baseline.ready requires last_run"))
        }
        _ => Ok(()),
    }
}

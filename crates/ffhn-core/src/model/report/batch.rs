use super::checks::{validate_batch_request_contract, validate_batch_run_report_identity};
use super::*;

/// Aggregate outcome counts inside `ffhn.batch_run_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BatchOutcomeCounts {
    /// Initialized runs.
    pub initialized: usize,
    /// Changed runs.
    pub changed: usize,
    /// Unchanged runs.
    pub unchanged: usize,
    /// Transient failures.
    pub failed_transient: usize,
    /// Permanent failures.
    pub failed_permanent: usize,
    /// Disabled skips.
    pub skipped_disabled: usize,
    /// Fatal process-level per-target failures.
    pub fatal_error: usize,
}

/// One target result inside `ffhn.batch_run_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BatchRunEntry {
    /// Target id requested for the entry.
    pub target_id: String,
    /// Structured run report when FFHN could emit one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_report: Option<RunReport>,
    /// Fatal process-level error when FFHN could not emit a run report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fatal_error: Option<String>,
}

/// Aggregate batch report emitted by multi-target runs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BatchRunReport {
    /// Frozen schema identity.
    pub schema_name: String,
    /// Frozen schema version.
    pub schema_version: u32,
    /// Execution mode applied to every target.
    pub run_mode: RunMode,
    /// Watch-root path used for the batch.
    pub watch_root: String,
    /// Targets requested for the batch, in requested order.
    pub requested_targets: Vec<String>,
    /// Batch start time.
    pub run_started_at: String,
    /// Batch finish time.
    pub run_finished_at: String,
    /// Maximum concurrent target runs.
    pub max_concurrency: usize,
    /// Per-target results in stable output order.
    pub entries: Vec<BatchRunEntry>,
    /// Aggregate counts by outcome class.
    pub outcome_counts: BatchOutcomeCounts,
    /// Reserved extensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Extensions,
}

impl BatchRunReport {
    /// Validates one batch run report.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_batch_run_report_identity(&self.schema_name, self.schema_version)?;
        require_non_empty("batch.watch_root", &self.watch_root)?;
        validate_timestamp(&self.run_started_at)?;
        validate_timestamp(&self.run_finished_at)?;
        validate_batch_request_contract(&self.requested_targets, self.max_concurrency)?;
        if self.entries.len() != self.requested_targets.len() {
            return Err(CoreError::htmlcut(
                "batch entries must align with requested_targets",
            ));
        }
        let mut counts = BatchOutcomeCounts {
            initialized: 0,
            changed: 0,
            unchanged: 0,
            failed_transient: 0,
            failed_permanent: 0,
            skipped_disabled: 0,
            fatal_error: 0,
        };
        for entry in &self.entries {
            validate_target_id(&entry.target_id)?;
            match (&entry.run_report, &entry.fatal_error) {
                (Some(report), None) => {
                    report.validate()?;
                    if report.target_id != entry.target_id {
                        return Err(CoreError::htmlcut(
                            "batch entry target_id must match run_report.target_id",
                        ));
                    }
                    match report.run_outcome {
                        RunOutcome::Initialized => counts.initialized += 1,
                        RunOutcome::Changed => counts.changed += 1,
                        RunOutcome::Unchanged => counts.unchanged += 1,
                        RunOutcome::FailedTransient => counts.failed_transient += 1,
                        RunOutcome::FailedPermanent => counts.failed_permanent += 1,
                        RunOutcome::SkippedDisabled => counts.skipped_disabled += 1,
                    }
                }
                (None, Some(error)) => {
                    require_non_empty("batch.fatal_error", error)?;
                    counts.fatal_error += 1;
                }
                _ => {
                    return Err(CoreError::htmlcut(
                        "batch entries must carry exactly one of run_report or fatal_error",
                    ));
                }
            }
        }
        if counts != self.outcome_counts {
            return Err(CoreError::htmlcut(
                "batch outcome_counts do not match the batch entries",
            ));
        }
        Ok(())
    }
}

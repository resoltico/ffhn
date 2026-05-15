use super::checks::{validate_batch_request_contract, validate_batch_run_report_identity};
use super::*;
use crate::model::validate::validate_timestamp_not_before;

mod wire;

/// Aggregate outcome counts inside `ffhn.batch_run_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchOutcomeCounts {
    /// Initialized runs.
    pub(crate) initialized: usize,
    /// Changed runs.
    pub(crate) changed: usize,
    /// Unchanged runs.
    pub(crate) unchanged: usize,
    /// Transient failures.
    pub(crate) failed_transient: usize,
    /// Permanent failures.
    pub(crate) failed_permanent: usize,
    /// Disabled skips.
    pub(crate) skipped_disabled: usize,
    /// Runs that emitted a report but failed at least one persist step.
    pub(crate) persist_failure: usize,
    /// Target entries with at least one failed or timed-out notification delivery.
    pub(crate) notification_failure: usize,
    /// Fatal process-level per-target failures.
    pub(crate) fatal_error: usize,
}

impl BatchOutcomeCounts {
    /// Returns the initialized-run count.
    pub const fn initialized(&self) -> usize {
        self.initialized
    }

    /// Returns the changed-run count.
    pub const fn changed(&self) -> usize {
        self.changed
    }

    /// Returns the unchanged-run count.
    pub const fn unchanged(&self) -> usize {
        self.unchanged
    }

    /// Returns the transient-failure count.
    pub const fn failed_transient(&self) -> usize {
        self.failed_transient
    }

    /// Returns the permanent-failure count.
    pub const fn failed_permanent(&self) -> usize {
        self.failed_permanent
    }

    /// Returns the disabled-skip count.
    pub const fn skipped_disabled(&self) -> usize {
        self.skipped_disabled
    }

    /// Returns the persist-failure count.
    pub const fn persist_failure(&self) -> usize {
        self.persist_failure
    }

    /// Returns the notification-failure count.
    pub const fn notification_failure(&self) -> usize {
        self.notification_failure
    }

    /// Returns the fatal per-target error count.
    pub const fn fatal_error(&self) -> usize {
        self.fatal_error
    }
}

/// One target result inside `ffhn.batch_run_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchRunEntry {
    /// Target id requested for the entry.
    pub(crate) target_id: String,
    /// Structured run report when FFHN could emit one.
    pub(crate) run_report: Option<RunReport>,
    /// Fatal process-level error when FFHN could not emit a run report.
    pub(crate) fatal_error: Option<ProcessErrorDetail>,
}

/// Coherent read-only view over one batch entry payload.
#[derive(Clone, Copy, Debug)]
pub enum BatchRunEntryView<'a> {
    /// One structured per-target run report.
    RunReport(&'a RunReport),
    /// One fatal per-target process error.
    FatalError(&'a ProcessErrorDetail),
}

/// Aggregate batch report emitted by multi-target runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchRunReport {
    /// Frozen schema identity.
    pub(crate) schema_name: String,
    /// Frozen schema version.
    pub(crate) schema_version: u32,
    /// Execution mode applied to every target.
    pub(crate) run_mode: RunMode,
    /// Watch-root path used for the batch.
    pub(crate) watch_root: String,
    /// Targets requested for the batch, in requested order.
    pub(crate) requested_targets: Vec<String>,
    /// Batch start time.
    pub(crate) run_started_at: String,
    /// Batch finish time.
    pub(crate) run_finished_at: String,
    /// Maximum concurrent target runs.
    pub(crate) max_concurrency: usize,
    /// Per-target results in stable output order.
    pub(crate) entries: Vec<BatchRunEntry>,
    /// Aggregate counts by outcome class.
    pub(crate) outcome_counts: BatchOutcomeCounts,
    /// Reserved extensions.
    pub(crate) extensions: Extensions,
}

/// Inputs used to build one validated aggregate batch report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchRunReportInput {
    /// Execution mode applied to every target.
    run_mode: RunMode,
    /// Watch-root path used for the batch.
    watch_root: String,
    /// Targets requested for the batch, in requested order.
    requested_targets: Vec<String>,
    /// Batch start time.
    run_started_at: String,
    /// Batch finish time.
    run_finished_at: String,
    /// Maximum concurrent target runs.
    max_concurrency: usize,
    /// Per-target results in stable output order.
    entries: Vec<BatchRunEntry>,
    /// Reserved extensions.
    extensions: Extensions,
}

impl BatchRunEntry {
    /// Builds one batch entry from a structured run report.
    pub fn from_run_report(run_report: RunReport) -> Self {
        Self {
            target_id: run_report.target_id().to_owned(),
            run_report: Some(run_report),
            fatal_error: None,
        }
    }

    /// Builds one fatal batch entry when FFHN could not emit a structured run report.
    ///
    /// The `target_id` is intentionally stored verbatim because discovery-mode batch runs may need
    /// to report contract-invalid directory labels back to the caller.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the target id is empty or the fatal error payload is invalid.
    pub fn fatal(
        target_id: impl Into<String>,
        fatal_error: ProcessErrorDetail,
    ) -> Result<Self, CoreError> {
        let entry = Self {
            target_id: target_id.into(),
            run_report: None,
            fatal_error: Some(fatal_error),
        };
        entry.validate()?;
        Ok(entry)
    }

    /// Returns the requested target label for this entry.
    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    /// Returns the structured run report when FFHN emitted one.
    pub fn run_report(&self) -> Option<&RunReport> {
        self.run_report.as_ref()
    }

    /// Returns the fatal process-level error when FFHN could not emit a run report.
    pub fn fatal_error(&self) -> Option<&ProcessErrorDetail> {
        self.fatal_error.as_ref()
    }

    /// Returns one coherent payload view instead of independent optional projections.
    pub fn view(&self) -> BatchRunEntryView<'_> {
        match (&self.run_report, &self.fatal_error) {
            (Some(report), None) => BatchRunEntryView::RunReport(report),
            (None, Some(error)) => BatchRunEntryView::FatalError(error),
            _ => unreachable!("validated batch entries carry exactly one payload"),
        }
    }

    /// Validates one batch entry.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the target id is empty, when the entry does not carry exactly
    /// one of `run_report` or `fatal_error`, or when the attached payload violates its own
    /// contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty("batch.entry.target_id", &self.target_id)?;
        match (&self.run_report, &self.fatal_error) {
            (Some(report), None) => {
                report.validate()?;
                if report.target_id() != self.target_id {
                    return Err(CoreError::contract(
                        "batch entry target_id must match run_report.target_id",
                    ));
                }
            }
            (None, Some(error)) => {
                error.validate()?;
            }
            _ => {
                return Err(CoreError::contract(
                    "batch entries must carry exactly one of run_report or fatal_error",
                ));
            }
        }
        Ok(())
    }
}

impl BatchRunReportInput {
    /// Builds one validated input bag for aggregate batch-report construction.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the requested target list or max-concurrency value violates
    /// FFHN's aggregate batch-request contract.
    pub fn new(
        run_mode: RunMode,
        watch_root: String,
        requested_targets: Vec<String>,
        run_started_at: String,
        run_finished_at: String,
        max_concurrency: usize,
        entries: Vec<BatchRunEntry>,
    ) -> Result<Self, CoreError> {
        validate_batch_request_contract(&requested_targets, max_concurrency)?;
        Ok(Self {
            run_mode,
            watch_root,
            requested_targets,
            run_started_at,
            run_finished_at,
            max_concurrency,
            entries,
            extensions: None,
        })
    }

    /// Attaches reserved extensions to one validated batch report input.
    #[must_use]
    pub fn with_extensions(mut self, extensions: Extensions) -> Self {
        self.extensions = extensions;
        self
    }
}

impl BatchRunReport {
    /// Returns the frozen schema name.
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    /// Returns the frozen schema version.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the run mode applied to every target.
    pub fn run_mode(&self) -> RunMode {
        self.run_mode
    }

    /// Returns the watch-root path encoded in the report.
    pub fn watch_root(&self) -> &str {
        &self.watch_root
    }

    /// Returns the requested targets in stable output order.
    pub fn requested_targets(&self) -> &[String] {
        &self.requested_targets
    }

    /// Returns the batch start timestamp.
    pub fn run_started_at(&self) -> &str {
        &self.run_started_at
    }

    /// Returns the batch finish timestamp.
    pub fn run_finished_at(&self) -> &str {
        &self.run_finished_at
    }

    /// Returns the maximum concurrency used for the batch.
    pub fn max_concurrency(&self) -> usize {
        self.max_concurrency
    }

    /// Returns the per-target entries in stable output order.
    pub fn entries(&self) -> &[BatchRunEntry] {
        &self.entries
    }

    /// Returns the aggregate outcome counts.
    pub fn outcome_counts(&self) -> &BatchOutcomeCounts {
        &self.outcome_counts
    }

    /// Returns any reserved extensions.
    pub fn extensions(&self) -> Option<&std::collections::BTreeMap<String, serde_json::Value>> {
        self.extensions.as_ref()
    }

    /// Builds one aggregate batch report and derives its outcome counts from the entries.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the derived aggregate counts, timestamps, entry identities, or
    /// schema invariants do not match FFHN's frozen batch-report contract.
    pub fn new(input: BatchRunReportInput) -> Result<Self, CoreError> {
        let outcome_counts = compute_outcome_counts(&input.requested_targets, &input.entries)?;
        let report = Self {
            schema_name: BATCH_RUN_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: BATCH_RUN_REPORT_SCHEMA_VERSION,
            run_mode: input.run_mode,
            watch_root: input.watch_root,
            requested_targets: input.requested_targets,
            run_started_at: input.run_started_at,
            run_finished_at: input.run_finished_at,
            max_concurrency: input.max_concurrency,
            entries: input.entries,
            outcome_counts,
            extensions: input.extensions,
        };
        report.validate()?;
        Ok(report)
    }

    /// Validates one batch run report.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the schema identity, batch request contract, timestamps, entry
    /// payloads, or outcome counts do not match FFHN's frozen batch-report contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_batch_run_report_identity(&self.schema_name, self.schema_version)?;
        require_non_empty("batch.watch_root", &self.watch_root)?;
        validate_timestamp(&self.run_started_at)?;
        validate_timestamp(&self.run_finished_at)?;
        validate_timestamp_not_before(
            "batch.run_started_at",
            &self.run_started_at,
            "batch.run_finished_at",
            &self.run_finished_at,
        )?;
        validate_batch_request_contract(&self.requested_targets, self.max_concurrency)?;
        let counts = compute_outcome_counts(&self.requested_targets, &self.entries)?;
        if counts != self.outcome_counts {
            return Err(CoreError::contract(
                "batch outcome_counts do not match the batch entries",
            ));
        }
        Ok(())
    }
}

fn compute_outcome_counts(
    requested_targets: &[String],
    entries: &[BatchRunEntry],
) -> Result<BatchOutcomeCounts, CoreError> {
    if entries.len() != requested_targets.len() {
        return Err(CoreError::contract(
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
        persist_failure: 0,
        notification_failure: 0,
        fatal_error: 0,
    };

    for (index, entry) in entries.iter().enumerate() {
        entry.validate()?;
        if entry.target_id() != requested_targets[index] {
            return Err(CoreError::contract(
                "batch entries must preserve requested_targets order",
            ));
        }
        if let Some(report) = entry.run_report() {
            match report.run_outcome() {
                RunOutcome::Initialized => counts.initialized += 1,
                RunOutcome::Changed => counts.changed += 1,
                RunOutcome::Unchanged => counts.unchanged += 1,
                RunOutcome::FailedTransient => counts.failed_transient += 1,
                RunOutcome::FailedPermanent => counts.failed_permanent += 1,
                RunOutcome::SkippedDisabled => counts.skipped_disabled += 1,
            }
            if report.persist().has_failure() {
                counts.persist_failure += 1;
            }
            if report
                .notifications()
                .any(|delivery| delivery.status() != crate::NotificationDeliveryStatus::Delivered)
            {
                counts.notification_failure += 1;
            }
            continue;
        }

        counts.fatal_error += 1;
    }

    Ok(counts)
}

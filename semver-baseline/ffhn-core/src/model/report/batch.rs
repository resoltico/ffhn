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
    /// Live runs that emitted a run report but still failed a persist step.
    pub persist_error: usize,
    /// Fatal process-level per-target failures.
    pub fatal_error: usize,
}

/// One target result inside `ffhn.batch_run_report`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "RawBatchRunEntry")]
pub struct BatchRunEntry {
    /// Target id requested for the entry.
    pub(crate) target_id: String,
    /// Structured run report when FFHN could emit one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) run_report: Option<RunReport>,
    /// Fatal process-level error when FFHN could not emit a run report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fatal_error: Option<ProcessErrorDetail>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBatchRunEntry {
    target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_report: Option<RunReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fatal_error: Option<ProcessErrorDetail>,
}

/// Aggregate batch report emitted by multi-target runs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "RawBatchRunReport")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) extensions: Extensions,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBatchRunReport {
    schema_name: String,
    schema_version: u32,
    run_mode: RunMode,
    watch_root: String,
    requested_targets: Vec<String>,
    run_started_at: String,
    run_finished_at: String,
    max_concurrency: usize,
    entries: Vec<BatchRunEntry>,
    outcome_counts: BatchOutcomeCounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
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

    /// Validates one batch entry.
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

impl TryFrom<RawBatchRunEntry> for BatchRunEntry {
    type Error = CoreError;

    fn try_from(raw: RawBatchRunEntry) -> Result<Self, Self::Error> {
        let entry = Self {
            target_id: raw.target_id,
            run_report: raw.run_report,
            fatal_error: raw.fatal_error,
        };
        entry.validate()?;
        Ok(entry)
    }
}

impl BatchRunReportInput {
    /// Builds one validated input bag for aggregate batch-report construction.
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

    /// Builds one aggregate batch report and derives its outcome counts from the entries.
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
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_batch_run_report_identity(&self.schema_name, self.schema_version)?;
        require_non_empty("batch.watch_root", &self.watch_root)?;
        validate_timestamp(&self.run_started_at)?;
        validate_timestamp(&self.run_finished_at)?;
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

impl TryFrom<RawBatchRunReport> for BatchRunReport {
    type Error = CoreError;

    fn try_from(raw: RawBatchRunReport) -> Result<Self, Self::Error> {
        let report = Self {
            schema_name: raw.schema_name,
            schema_version: raw.schema_version,
            run_mode: raw.run_mode,
            watch_root: raw.watch_root,
            requested_targets: raw.requested_targets,
            run_started_at: raw.run_started_at,
            run_finished_at: raw.run_finished_at,
            max_concurrency: raw.max_concurrency,
            entries: raw.entries,
            outcome_counts: raw.outcome_counts,
            extensions: raw.extensions,
        };
        report.validate()?;
        Ok(report)
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
        persist_error: 0,
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
            match report.run_outcome {
                RunOutcome::Initialized => counts.initialized += 1,
                RunOutcome::Changed => counts.changed += 1,
                RunOutcome::Unchanged => counts.unchanged += 1,
                RunOutcome::FailedTransient => counts.failed_transient += 1,
                RunOutcome::FailedPermanent => counts.failed_permanent += 1,
                RunOutcome::SkippedDisabled => counts.skipped_disabled += 1,
            }
            if report.reason_code == ReasonCode::PersistError || report.persist.error.is_some() {
                counts.persist_error += 1;
            }
            continue;
        }

        counts.fatal_error += 1;
    }

    Ok(counts)
}

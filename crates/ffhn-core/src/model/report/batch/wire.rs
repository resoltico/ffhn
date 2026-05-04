use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    BatchOutcomeCounts, BatchRunEntry, BatchRunReport, Extensions, ProcessErrorDetail, RunMode,
    RunReport,
};
use crate::CoreError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBatchOutcomeCounts {
    initialized: usize,
    changed: usize,
    unchanged: usize,
    failed_transient: usize,
    failed_permanent: usize,
    skipped_disabled: usize,
    persist_error: usize,
    notification_failure: usize,
    fatal_error: usize,
}

impl From<RawBatchOutcomeCounts> for BatchOutcomeCounts {
    fn from(raw: RawBatchOutcomeCounts) -> Self {
        Self {
            initialized: raw.initialized,
            changed: raw.changed,
            unchanged: raw.unchanged,
            failed_transient: raw.failed_transient,
            failed_permanent: raw.failed_permanent,
            skipped_disabled: raw.skipped_disabled,
            persist_error: raw.persist_error,
            notification_failure: raw.notification_failure,
            fatal_error: raw.fatal_error,
        }
    }
}

impl From<&BatchOutcomeCounts> for RawBatchOutcomeCounts {
    fn from(counts: &BatchOutcomeCounts) -> Self {
        Self {
            initialized: counts.initialized,
            changed: counts.changed,
            unchanged: counts.unchanged,
            failed_transient: counts.failed_transient,
            failed_permanent: counts.failed_permanent,
            skipped_disabled: counts.skipped_disabled,
            persist_error: counts.persist_error,
            notification_failure: counts.notification_failure,
            fatal_error: counts.fatal_error,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBatchRunEntry {
    target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_report: Option<RunReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fatal_error: Option<ProcessErrorDetail>,
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

impl From<&BatchRunEntry> for RawBatchRunEntry {
    fn from(entry: &BatchRunEntry) -> Self {
        Self {
            target_id: entry.target_id.clone(),
            run_report: entry.run_report.clone(),
            fatal_error: entry.fatal_error.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
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
    entries: Vec<RawBatchRunEntry>,
    outcome_counts: RawBatchOutcomeCounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
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
            entries: raw
                .entries
                .into_iter()
                .map(BatchRunEntry::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            outcome_counts: raw.outcome_counts.into(),
            extensions: raw.extensions,
        };
        report.validate()?;
        Ok(report)
    }
}

impl From<&BatchRunReport> for RawBatchRunReport {
    fn from(report: &BatchRunReport) -> Self {
        Self {
            schema_name: report.schema_name.clone(),
            schema_version: report.schema_version,
            run_mode: report.run_mode,
            watch_root: report.watch_root.clone(),
            requested_targets: report.requested_targets.clone(),
            run_started_at: report.run_started_at.clone(),
            run_finished_at: report.run_finished_at.clone(),
            max_concurrency: report.max_concurrency,
            entries: report.entries.iter().map(RawBatchRunEntry::from).collect(),
            outcome_counts: RawBatchOutcomeCounts::from(&report.outcome_counts),
            extensions: report.extensions.clone(),
        }
    }
}

impl Serialize for BatchRunReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawBatchRunReport::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BatchRunReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawBatchRunReport::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

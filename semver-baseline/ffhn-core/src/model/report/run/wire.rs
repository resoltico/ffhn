use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    BaselinePhase, ChangeKind, CompareBasis, Extensions, FetchEngine, PersistWriteStatus,
    ProcessErrorDetail, RunChangeRegion, RunChangeSection, RunCompareSection, RunExtractionSection,
    RunFetchSection, RunMode, RunNotificationDelivery, RunPersistSection, RunReport, RunResult,
    SelectionKind, SelectionMatch,
};
use crate::CoreError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunFetchSection {
    engine: FetchEngine,
    final_url: Option<String>,
    http_status: Option<u16>,
    content_type: Option<String>,
    bytes_read: Option<usize>,
    duration_ms: u64,
}

impl From<RawRunFetchSection> for RunFetchSection {
    fn from(raw: RawRunFetchSection) -> Self {
        Self {
            engine: raw.engine,
            final_url: raw.final_url,
            http_status: raw.http_status,
            content_type: raw.content_type,
            bytes_read: raw.bytes_read,
            duration_ms: raw.duration_ms,
        }
    }
}

impl From<&RunFetchSection> for RawRunFetchSection {
    fn from(section: &RunFetchSection) -> Self {
        Self {
            engine: section.engine,
            final_url: section.final_url.clone(),
            http_status: section.http_status,
            content_type: section.content_type.clone(),
            bytes_read: section.bytes_read,
            duration_ms: section.duration_ms,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunExtractionSection {
    compare_source_sha256: String,
    outer_html_sha256: String,
    selection_kind: SelectionKind,
    selection_match: SelectionMatch,
    candidate_count: usize,
    selected_candidate_index: usize,
    warning_codes: Vec<String>,
    duration_ms: u64,
}

impl From<RawRunExtractionSection> for RunExtractionSection {
    fn from(raw: RawRunExtractionSection) -> Self {
        Self {
            compare_source_sha256: raw.compare_source_sha256,
            outer_html_sha256: raw.outer_html_sha256,
            selection_kind: raw.selection_kind,
            selection_match: raw.selection_match,
            candidate_count: raw.candidate_count,
            selected_candidate_index: raw.selected_candidate_index,
            warning_codes: raw.warning_codes,
            duration_ms: raw.duration_ms,
        }
    }
}

impl From<&RunExtractionSection> for RawRunExtractionSection {
    fn from(section: &RunExtractionSection) -> Self {
        Self {
            compare_source_sha256: section.compare_source_sha256.clone(),
            outer_html_sha256: section.outer_html_sha256.clone(),
            selection_kind: section.selection_kind,
            selection_match: section.selection_match,
            candidate_count: section.candidate_count,
            selected_candidate_index: section.selected_candidate_index,
            warning_codes: section.warning_codes.clone(),
            duration_ms: section.duration_ms,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunCompareSection {
    canonicalizers: Vec<String>,
    duration_ms: u64,
}

impl From<RawRunCompareSection> for RunCompareSection {
    fn from(raw: RawRunCompareSection) -> Self {
        Self {
            canonicalizers: raw.canonicalizers,
            duration_ms: raw.duration_ms,
        }
    }
}

impl From<&RunCompareSection> for RawRunCompareSection {
    fn from(section: &RunCompareSection) -> Self {
        Self {
            canonicalizers: section.canonicalizers.clone(),
            duration_ms: section.duration_ms,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum RawPersistWriteStatus {
    NotAttempted,
    Written,
    Failed { error: ProcessErrorDetail },
}

impl From<RawPersistWriteStatus> for PersistWriteStatus {
    fn from(raw: RawPersistWriteStatus) -> Self {
        match raw {
            RawPersistWriteStatus::NotAttempted => Self::NotAttempted,
            RawPersistWriteStatus::Written => Self::Written,
            RawPersistWriteStatus::Failed { error } => Self::Failed { error },
        }
    }
}

impl From<&PersistWriteStatus> for RawPersistWriteStatus {
    fn from(status: &PersistWriteStatus) -> Self {
        match status {
            PersistWriteStatus::NotAttempted => Self::NotAttempted,
            PersistWriteStatus::Written => Self::Written,
            PersistWriteStatus::Failed { error } => Self::Failed {
                error: error.clone(),
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunPersistSection {
    state_commit_duration_ms: u64,
    state_commit: RawPersistWriteStatus,
    last_run_write_duration_ms: u64,
    last_run_write: RawPersistWriteStatus,
}

impl From<RawRunPersistSection> for RunPersistSection {
    fn from(raw: RawRunPersistSection) -> Self {
        Self {
            state_commit: raw.state_commit.into(),
            state_commit_duration_ms: raw.state_commit_duration_ms,
            last_run_write: raw.last_run_write.into(),
            last_run_write_duration_ms: raw.last_run_write_duration_ms,
        }
    }
}

impl From<&RunPersistSection> for RawRunPersistSection {
    fn from(section: &RunPersistSection) -> Self {
        Self {
            state_commit_duration_ms: section.state_commit_duration_ms,
            state_commit: RawPersistWriteStatus::from(&section.state_commit),
            last_run_write: RawPersistWriteStatus::from(&section.last_run_write),
            last_run_write_duration_ms: section.last_run_write_duration_ms,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunChangeRegion {
    previous_start_line: usize,
    previous_line_count: usize,
    current_start_line: usize,
    current_line_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_excerpt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_excerpt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_excerpt_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_excerpt_sha256: Option<String>,
}

impl From<RawRunChangeRegion> for RunChangeRegion {
    fn from(raw: RawRunChangeRegion) -> Self {
        Self {
            previous_start_line: raw.previous_start_line,
            previous_line_count: raw.previous_line_count,
            current_start_line: raw.current_start_line,
            current_line_count: raw.current_line_count,
            previous_excerpt: raw.previous_excerpt,
            current_excerpt: raw.current_excerpt,
            previous_excerpt_sha256: raw.previous_excerpt_sha256,
            current_excerpt_sha256: raw.current_excerpt_sha256,
        }
    }
}

impl From<&RunChangeRegion> for RawRunChangeRegion {
    fn from(region: &RunChangeRegion) -> Self {
        Self {
            previous_start_line: region.previous_start_line,
            previous_line_count: region.previous_line_count,
            current_start_line: region.current_start_line,
            current_line_count: region.current_line_count,
            previous_excerpt: region.previous_excerpt.clone(),
            current_excerpt: region.current_excerpt.clone(),
            previous_excerpt_sha256: region.previous_excerpt_sha256.clone(),
            current_excerpt_sha256: region.current_excerpt_sha256.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunChangeSection {
    kind: ChangeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_compare_bytes: Option<usize>,
    current_compare_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_compare_line_count: Option<usize>,
    current_compare_line_count: usize,
    common_prefix_lines: usize,
    common_suffix_lines: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    changed_region: Option<RawRunChangeRegion>,
}

impl From<RawRunChangeSection> for RunChangeSection {
    fn from(raw: RawRunChangeSection) -> Self {
        Self {
            kind: raw.kind,
            previous_compare_bytes: raw.previous_compare_bytes,
            current_compare_bytes: raw.current_compare_bytes,
            previous_compare_line_count: raw.previous_compare_line_count,
            current_compare_line_count: raw.current_compare_line_count,
            common_prefix_lines: raw.common_prefix_lines,
            common_suffix_lines: raw.common_suffix_lines,
            changed_region: raw.changed_region.map(RunChangeRegion::from),
        }
    }
}

impl From<&RunChangeSection> for RawRunChangeSection {
    fn from(section: &RunChangeSection) -> Self {
        Self {
            kind: section.kind,
            previous_compare_bytes: section.previous_compare_bytes,
            current_compare_bytes: section.current_compare_bytes,
            previous_compare_line_count: section.previous_compare_line_count,
            current_compare_line_count: section.current_compare_line_count,
            common_prefix_lines: section.common_prefix_lines,
            common_suffix_lines: section.common_suffix_lines,
            changed_region: section
                .changed_region
                .as_ref()
                .map(RawRunChangeRegion::from),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunReport {
    schema_name: String,
    schema_version: u32,
    run_report_digest_sha256: String,
    target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    run_started_at: String,
    run_finished_at: String,
    run_mode: RunMode,
    result: RunResult,
    compare_basis: CompareBasis,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_compare_digest_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_compare_digest_sha256: Option<String>,
    baseline_phase_before_run: BaselinePhase,
    baseline_phase_after_run: BaselinePhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    fetch: Option<RawRunFetchSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extraction: Option<RawRunExtractionSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compare: Option<RawRunCompareSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    change: Option<RawRunChangeSection>,
    persist: RawRunPersistSection,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notifications: Vec<RunNotificationDelivery>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Extensions,
}

impl TryFrom<RawRunReport> for RunReport {
    type Error = CoreError;

    fn try_from(raw: RawRunReport) -> Result<Self, Self::Error> {
        let report = Self {
            schema_name: raw.schema_name,
            schema_version: raw.schema_version,
            run_report_digest_sha256: raw.run_report_digest_sha256,
            target_id: raw.target_id.try_into()?,
            display_name: raw.display_name,
            run_started_at: raw.run_started_at,
            run_finished_at: raw.run_finished_at,
            run_mode: raw.run_mode,
            result: raw.result,
            compare_basis: raw.compare_basis,
            previous_compare_digest_sha256: raw.previous_compare_digest_sha256,
            current_compare_digest_sha256: raw.current_compare_digest_sha256,
            baseline_phase_before_run: raw.baseline_phase_before_run,
            baseline_phase_after_run: raw.baseline_phase_after_run,
            fetch: raw.fetch.map(RunFetchSection::from),
            extraction: raw.extraction.map(RunExtractionSection::from),
            compare: raw.compare.map(RunCompareSection::from),
            change: raw.change.map(RunChangeSection::from),
            persist: raw.persist.into(),
            notifications: raw.notifications,
            extensions: raw.extensions,
        };
        report.validate()?;
        Ok(report)
    }
}

impl From<&RunReport> for RawRunReport {
    fn from(report: &RunReport) -> Self {
        Self {
            schema_name: report.schema_name.clone(),
            schema_version: report.schema_version,
            run_report_digest_sha256: report.run_report_digest_sha256.clone(),
            target_id: report.target_id.as_str().to_owned(),
            display_name: report.display_name.clone(),
            run_started_at: report.run_started_at.clone(),
            run_finished_at: report.run_finished_at.clone(),
            run_mode: report.run_mode,
            result: report.result.clone(),
            compare_basis: report.compare_basis,
            previous_compare_digest_sha256: report.previous_compare_digest_sha256.clone(),
            current_compare_digest_sha256: report.current_compare_digest_sha256.clone(),
            baseline_phase_before_run: report.baseline_phase_before_run,
            baseline_phase_after_run: report.baseline_phase_after_run,
            fetch: report.fetch.as_ref().map(RawRunFetchSection::from),
            extraction: report
                .extraction
                .as_ref()
                .map(RawRunExtractionSection::from),
            compare: report.compare.as_ref().map(RawRunCompareSection::from),
            change: report.change.as_ref().map(RawRunChangeSection::from),
            persist: RawRunPersistSection::from(&report.persist),
            notifications: report.notifications.clone(),
            extensions: report.extensions.clone(),
        }
    }
}

impl Serialize for RunReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawRunReport::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RunReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawRunReport::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

use super::*;
use crate::{
    LAST_RUN_SNAPSHOT_SCHEMA_NAME, LAST_RUN_SNAPSHOT_SCHEMA_VERSION, RUN_REPORT_SCHEMA_NAME,
    RUN_REPORT_SCHEMA_VERSION, STATUS_REPORT_SCHEMA_NAME, STATUS_REPORT_SCHEMA_VERSION, TargetId,
};

pub(super) use super::checks::{validate_notification_delivery, validate_run_change_section};

pub(super) const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

pub(super) fn target_id(value: &str) -> TargetId {
    TargetId::new(value).expect("target id")
}

pub(super) fn valid_process_error() -> ProcessErrorDetail {
    ProcessErrorDetail {
        kind: ProcessErrorKind::Io,
        message: "permission denied".to_owned(),
        path: Some("/tmp/watch/demo/last_run.json".to_owned()),
    }
}

pub(super) fn delivered_notification() -> RunNotificationDelivery {
    RunNotificationDelivery::delivered("notify", 1, 0)
}

pub(super) fn failed_notification(
    exit_code: Option<i32>,
    error: impl Into<String>,
) -> RunNotificationDelivery {
    RunNotificationDelivery::failed("notify", 1, exit_code, error)
}

pub(super) fn persist_section(
    duration_ms: u64,
    state_commit: PersistWriteStatus,
    last_run_write: PersistWriteStatus,
) -> RunPersistSection {
    RunPersistSection::from_writes(
        if state_commit.is_not_attempted() {
            0
        } else {
            duration_ms
        },
        state_commit,
        if last_run_write.is_not_attempted() {
            0
        } else {
            duration_ms
        },
        last_run_write,
    )
}

pub(super) fn valid_run_report() -> RunReport {
    RunReport {
        schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: RUN_REPORT_SCHEMA_VERSION,
        run_report_digest_sha256: String::new(),
        target_id: target_id("demo"),
        display_name: Some("Demo".to_owned()),
        run_started_at: "2026-04-05T10:15:30Z".to_owned(),
        run_finished_at: "2026-04-05T10:15:31Z".to_owned(),
        run_mode: RunMode::Live,
        result: RunResult::Changed,
        compare_basis: CompareBasis::CanonicalTextSha256,
        previous_compare_digest_sha256: Some(DIGEST.to_owned()),
        current_compare_digest_sha256: Some(DIGEST.to_owned()),
        baseline_phase_before_run: BaselinePhase::HasBaseline,
        baseline_phase_after_run: BaselinePhase::HasBaseline,
        fetch: Some(RunFetchSection {
            engine: FetchEngine::Http,
            final_url: Some("https://example.com/final".to_owned()),
            http_status: Some(200),
            content_type: Some("text/html".to_owned()),
            bytes_read: Some(42),
            duration_ms: 12,
        }),
        extraction: Some(RunExtractionSection {
            comparison_input_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            selection_kind: SelectionKind::CssSelector,
            selection_match: SelectionMatch::Single,
            output_kind: OutputKind::OuterHtml,
            candidate_count: 1,
            selected_candidate_index: 1,
            warning_codes: Vec::new(),
            duration_ms: 8,
        }),
        compare: Some(RunCompareSection {
            canonicalizers: vec!["trim".to_owned()],
            duration_ms: 3,
        }),
        change: Some(RunChangeSection {
            kind: ChangeKind::Changed,
            previous_text_bytes: Some(6),
            current_text_bytes: 7,
            previous_line_count: Some(1),
            current_line_count: 1,
            common_prefix_lines: 0,
            common_suffix_lines: 0,
            changed_region: Some(RunChangeRegion {
                previous_start_line: 1,
                previous_line_count: 1,
                current_start_line: 1,
                current_line_count: 1,
                previous_excerpt: Some("Before".to_owned()),
                current_excerpt: Some("Changed".to_owned()),
                previous_excerpt_sha256: Some(DIGEST.to_owned()),
                current_excerpt_sha256: Some(DIGEST.to_owned()),
            }),
        }),
        persist: persist_section(2, PersistWriteStatus::Written, PersistWriteStatus::Written),
        notifications: Vec::new(),
        extensions: None,
    }
    .with_digest()
    .expect("digest")
}

pub(super) fn valid_batch_report() -> BatchRunReport {
    BatchRunReport {
        schema_name: BATCH_RUN_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: BATCH_RUN_REPORT_SCHEMA_VERSION,
        run_mode: RunMode::Live,
        watch_root: "watchlist".to_owned(),
        requested_targets: vec!["demo".to_owned(), "fatal_target".to_owned()],
        run_started_at: "2026-04-05T10:15:30Z".to_owned(),
        run_finished_at: "2026-04-05T10:15:31Z".to_owned(),
        max_concurrency: 2,
        entries: vec![
            BatchRunEntry {
                target_id: "demo".to_owned(),
                run_report: Some(valid_run_report()),
                fatal_error: None,
            },
            BatchRunEntry {
                target_id: "fatal_target".to_owned(),
                run_report: None,
                fatal_error: Some(valid_process_error()),
            },
        ],
        outcome_counts: BatchOutcomeCounts {
            initialized: 0,
            changed: 1,
            unchanged: 0,
            failed_transient: 0,
            failed_permanent: 0,
            skipped_disabled: 0,
            persist_failure: 0,
            notification_failure: 0,
            fatal_error: 1,
        },
        extensions: None,
    }
}

mod batch;
mod helpers;
mod last_run;
mod notification;
mod run;
mod status;

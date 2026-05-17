use std::io::{self, Write};

use ffhn_core::{
    BatchRunEntry, BatchRunEntryView, BatchRunReport, ProcessErrorDetail, RunBodyView, RunMode,
    RunReport, StatusReport,
};
use serde::Serialize;

use crate::args::OutputFormat;

pub fn render_run_report(
    stdout: &mut (impl Write + ?Sized),
    report: &RunReport,
    format: OutputFormat,
) -> io::Result<()> {
    match format {
        OutputFormat::Json => render_json_document(stdout, report),
        OutputFormat::JsonPretty => render_pretty_json_document(stdout, report),
        OutputFormat::Summary => render_run_summary(stdout, report),
    }
}

pub fn render_status_report(
    stdout: &mut (impl Write + ?Sized),
    report: &StatusReport,
    format: OutputFormat,
) -> io::Result<()> {
    match format {
        OutputFormat::Json => render_json_document(stdout, report),
        OutputFormat::JsonPretty => render_pretty_json_document(stdout, report),
        OutputFormat::Summary => render_status_summary(stdout, report),
    }
}

pub fn render_batch_report(
    stdout: &mut (impl Write + ?Sized),
    report: &BatchRunReport,
    format: OutputFormat,
) -> io::Result<()> {
    match format {
        OutputFormat::Json => render_json_document(stdout, report),
        OutputFormat::JsonPretty => render_pretty_json_document(stdout, report),
        OutputFormat::Summary => render_batch_summary(stdout, report),
    }
}

fn render_json_document(
    stdout: &mut (impl Write + ?Sized),
    value: &impl Serialize,
) -> io::Result<()> {
    serde_json::to_writer(&mut *stdout, value).map_err(io::Error::other)?;
    writeln!(stdout)
}

fn render_pretty_json_document(
    stdout: &mut (impl Write + ?Sized),
    value: &impl Serialize,
) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *stdout, value).map_err(io::Error::other)?;
    writeln!(stdout)
}

fn render_run_summary(stdout: &mut (impl Write + ?Sized), report: &RunReport) -> io::Result<()> {
    writeln!(stdout, "Run report")?;
    render_target_heading(
        stdout,
        report.target_id(),
        report.display_name(),
        report.run_mode(),
    )?;
    writeln!(stdout, "Outcome: {}", run_outcome_label(report))?;
    writeln!(
        stdout,
        "Baseline phase: {} -> {}",
        report.baseline_phase_before_run().as_str(),
        report.baseline_phase_after_run().as_str()
    )?;
    writeln!(stdout, "Started: {}", report.run_started_at())?;
    writeln!(stdout, "Finished: {}", report.run_finished_at())?;

    if let Some(detail) = report.error_detail() {
        render_error_detail(stdout, detail)?;
    }

    match report.body() {
        RunBodyView::Reportable(body) => {
            let fetch = body.fetch();
            let extraction = body.extraction();
            let compare = body.compare();
            let change = body.change();
            writeln!(
                stdout,
                "Fetch: engine={}, bytes={}, duration={} ms",
                fetch.engine().as_str(),
                fetch.bytes_read().unwrap_or(0),
                fetch.duration_ms()
            )?;
            if let Some(url) = fetch.final_url() {
                writeln!(stdout, "Final URL: {url}")?;
            }
            if let Some(status) = fetch.http_status() {
                writeln!(stdout, "HTTP status: {status}")?;
            }
            writeln!(
                stdout,
                "Extraction: kind={}, match={}, candidates={}, selected={}, duration={} ms",
                extraction.selection_kind().as_str(),
                extraction.selection_match().as_str(),
                extraction.candidate_count(),
                extraction.selected_candidate_index(),
                extraction.duration_ms()
            )?;
            writeln!(
                stdout,
                "Compare: basis={}, current_digest={}, duration={} ms",
                report.compare_basis().as_str(),
                body.current_compare_digest_sha256(),
                compare.duration_ms()
            )?;
            writeln!(stdout, "Change: kind={}", change.kind().as_str())?;
        }
        RunBodyView::None => {}
        _ => {
            let fetch = report
                .fetch()
                .expect("partial run bodies that are not reportable carry fetch");
            write!(
                stdout,
                "Progress: fetch completed; fetch_engine={}, bytes={}, duration={} ms",
                fetch.engine().as_str(),
                fetch.bytes_read().unwrap_or(0),
                fetch.duration_ms()
            )?;
            if let Some(extraction) = report.extraction() {
                write!(
                    stdout,
                    "; extraction_candidates={}",
                    extraction.candidate_count()
                )?;
            }
            if let Some(compare) = report.compare() {
                write!(stdout, "; compare_duration={} ms", compare.duration_ms())?;
            }
            writeln!(stdout)?;
            if let Some(url) = fetch.final_url() {
                writeln!(stdout, "Final URL: {url}")?;
            }
            if let Some(status) = fetch.http_status() {
                writeln!(stdout, "HTTP status: {status}")?;
            }
        }
    }

    writeln!(
        stdout,
        "Persist: state_commit={} ({} ms), last_run_write={} ({} ms)",
        report.persist().state_commit().as_str(),
        report.persist().state_commit_duration_ms(),
        report.persist().last_run_write().as_str(),
        report.persist().last_run_write_duration_ms()
    )?;
    render_notifications_summary(stdout, report.notifications())?;
    Ok(())
}

fn render_status_summary(
    stdout: &mut (impl Write + ?Sized),
    report: &StatusReport,
) -> io::Result<()> {
    writeln!(stdout, "Status report")?;
    writeln!(stdout, "Target: {}", report.target_id())?;
    if let Some(display_name) = report.display_name() {
        writeln!(stdout, "Display name: {display_name}")?;
    }
    if let Some(enabled) = report.enabled() {
        writeln!(stdout, "Enabled: {enabled}")?;
    }
    writeln!(stdout, "Status: {}", report.status().kind_str())?;
    if let Some(phase) = report.baseline_phase() {
        writeln!(stdout, "Baseline phase: {}", phase.as_str())?;
    }
    if let Some(current) = report.current_snapshot() {
        writeln!(
            stdout,
            "Current snapshot: compare={}, outer={}, captured_at={}",
            current.compare_digest_sha256(),
            current.outer_html_sha256(),
            current.captured_at()
        )?;
    }
    if !report.snapshot_history().is_empty() {
        writeln!(
            stdout,
            "Snapshot history: {}",
            report.snapshot_history().len()
        )?;
    }
    if let Some(detail) = report.error_detail() {
        render_error_detail(stdout, detail)?;
    }
    Ok(())
}

fn render_batch_summary(
    stdout: &mut (impl Write + ?Sized),
    report: &BatchRunReport,
) -> io::Result<()> {
    let counts = report.outcome_counts();
    writeln!(stdout, "Batch run report")?;
    writeln!(stdout, "Mode: {}", report.run_mode().as_str())?;
    writeln!(stdout, "Watch root: {}", report.watch_root())?;
    writeln!(
        stdout,
        "Requested targets: {}",
        report.requested_targets().len()
    )?;
    writeln!(stdout, "Started: {}", report.run_started_at())?;
    writeln!(stdout, "Finished: {}", report.run_finished_at())?;
    writeln!(stdout, "Max concurrency: {}", report.max_concurrency())?;
    writeln!(
        stdout,
        "Outcome counts: initialized={}, changed={}, unchanged={}, failed_transient={}, failed_permanent={}, skipped_disabled={}, persist_failure={}, notification_failure={}, fatal_error={}",
        counts.initialized(),
        counts.changed(),
        counts.unchanged(),
        counts.failed_transient(),
        counts.failed_permanent(),
        counts.skipped_disabled(),
        counts.persist_failure(),
        counts.notification_failure(),
        counts.fatal_error()
    )?;
    writeln!(stdout, "Entries:")?;
    for entry in report.entries() {
        render_batch_entry(stdout, entry)?;
    }
    Ok(())
}

fn render_batch_entry(stdout: &mut (impl Write + ?Sized), entry: &BatchRunEntry) -> io::Result<()> {
    match entry.view() {
        BatchRunEntryView::RunReport(report) => {
            let display_name = report
                .display_name()
                .map_or_else(String::new, |value| format!(" ({value})"));
            writeln!(
                stdout,
                "- {}{}: {}",
                entry.target_id(),
                display_name,
                run_outcome_label(report)
            )
        }
        BatchRunEntryView::FatalError(error) => {
            write!(stdout, "- {}: fatal_error", entry.target_id())?;
            if let Some(path) = error.path() {
                write!(stdout, " [{}]", path)?;
            }
            writeln!(stdout, " - {}", error.message())
        }
    }
}

fn render_target_heading(
    stdout: &mut (impl Write + ?Sized),
    target_id: &str,
    display_name: Option<&str>,
    run_mode: RunMode,
) -> io::Result<()> {
    writeln!(stdout, "Target: {target_id}")?;
    if let Some(display_name) = display_name {
        writeln!(stdout, "Display name: {display_name}")?;
    }
    writeln!(stdout, "Mode: {}", run_mode.as_str())
}

fn render_notifications_summary<'a>(
    stdout: &mut (impl Write + ?Sized),
    notifications: impl Iterator<Item = ffhn_core::RunNotificationDeliveryView<'a>>,
) -> io::Result<()> {
    let notifications = notifications.collect::<Vec<_>>();
    if notifications.is_empty() {
        return writeln!(stdout, "Notifications: none");
    }

    writeln!(stdout, "Notifications: {}", notifications.len())?;
    for notification in notifications {
        write!(
            stdout,
            "- {}: {} ({} ms",
            notification.route_name(),
            notification.status().as_str(),
            notification.duration_ms()
        )?;
        if let Some(exit_code) = notification.exit_code() {
            write!(stdout, ", exit_code={exit_code}")?;
        }
        writeln!(stdout, ")")?;
        if let Some(error) = notification.error() {
            writeln!(stdout, "  error: {error}")?;
        }
    }
    Ok(())
}

fn render_error_detail(
    stdout: &mut (impl Write + ?Sized),
    detail: &ProcessErrorDetail,
) -> io::Result<()> {
    writeln!(
        stdout,
        "Error: {} - {}",
        detail.kind().as_str(),
        detail.message()
    )?;
    if let Some(path) = detail.path() {
        writeln!(stdout, "Error path: {path}")?;
    }
    Ok(())
}

fn run_outcome_label(report: &RunReport) -> String {
    let mut label = report.run_outcome().as_str().to_owned();
    if let Some(cause) = report.failure_cause() {
        label.push_str(" (");
        label.push_str(cause.as_str());
        label.push(')');
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use serde_json::json;
    use sha2::{Digest, Sha256};

    fn parse_run_report_value(value: Value) -> RunReport {
        serde_json::from_value(value).expect("run report json")
    }

    fn parse_run_report_object(value: Value) -> RunReport {
        parse_run_report_value(with_valid_run_report_digest(
            value.as_object().cloned().expect("run report object"),
        ))
    }

    fn parse_status_report(value: serde_json::Value) -> StatusReport {
        serde_json::from_value(value).expect("status report json")
    }

    fn parse_batch_report(value: serde_json::Value) -> BatchRunReport {
        serde_json::from_value(value).expect("batch report json")
    }

    fn changed_run_report() -> RunReport {
        parse_run_report_object(json!({
            "schema_name": "ffhn.run_report",
            "schema_version": 4,
            "target_id": "payload_target",
            "display_name": "Payload Target",
            "run_started_at": "2026-05-14T15:24:54.420145Z",
            "run_finished_at": "2026-05-14T15:24:54.426475Z",
            "run_mode": "live",
            "result": { "kind": "changed" },
            "compare_basis": "text",
            "previous_compare_digest_sha256": "ee8abdc0db68120327dc6674b651c44f17e6df9769b233f7528fe1f52479f3a5",
            "current_compare_digest_sha256": "079522f2e24f6e98341baf703990cf8a79bce55eaaab8c70bd73e2f840c1c424",
            "baseline_phase_before_run": "has_baseline",
            "baseline_phase_after_run": "has_baseline",
            "fetch": {
                "engine": "file",
                "final_url": "file:///Users/erst/Tools/ffhn/tmp/seed-refresh/source.html",
                "http_status": null,
                "content_type": null,
                "bytes_read": 55,
                "duration_ms": 0
            },
            "extraction": {
                "compare_source_sha256": "079522f2e24f6e98341baf703990cf8a79bce55eaaab8c70bd73e2f840c1c424",
                "outer_html_sha256": "c028a2183b5d2f7b48e7415a5616ec4b72c9ef78f8c79afbc92ddf68db706541",
                "selection_kind": "css_selector",
                "selection_match": "single",
                "candidate_count": 1,
                "selected_candidate_index": 1,
                "warning_codes": [],
                "duration_ms": 1
            },
            "compare": {
                "canonicalizers": ["trim"],
                "duration_ms": 0
            },
            "change": {
                "kind": "changed",
                "previous_compare_bytes": 12,
                "current_compare_bytes": 15,
                "previous_compare_line_count": 1,
                "current_compare_line_count": 1,
                "common_prefix_lines": 0,
                "common_suffix_lines": 0,
                "changed_region": {
                    "previous_start_line": 1,
                    "previous_line_count": 1,
                    "current_start_line": 1,
                    "current_line_count": 1,
                    "previous_excerpt": "Payload Seed",
                    "current_excerpt": "Payload Changed",
                    "previous_excerpt_sha256": "ee8abdc0db68120327dc6674b651c44f17e6df9769b233f7528fe1f52479f3a5",
                    "current_excerpt_sha256": "079522f2e24f6e98341baf703990cf8a79bce55eaaab8c70bd73e2f840c1c424"
                }
            },
            "persist": {
                "state_commit_duration_ms": 1,
                "state_commit": { "status": "written" },
                "last_run_write_duration_ms": 0,
                "last_run_write": { "status": "written" }
            }
        }))
    }

    fn dry_run_initialized_run_report(target_id: &str) -> RunReport {
        parse_run_report_object(json!({
            "schema_name": "ffhn.run_report",
            "schema_version": 4,
            "target_id": target_id,
            "display_name": target_id,
            "run_started_at": "2026-05-14T15:15:43.155502Z",
            "run_finished_at": "2026-05-14T15:15:43.158166Z",
            "run_mode": "dry_run",
            "result": { "kind": "initialized" },
            "compare_basis": "text",
            "current_compare_digest_sha256": "703390318bd55aef50b7823d2b90a846debff99e6e3d401a24a921b733912a6d",
            "baseline_phase_before_run": "never_succeeded",
            "baseline_phase_after_run": "never_succeeded",
            "fetch": {
                "engine": "file",
                "final_url": "file:///private/var/folders/45/5nz735_d4_zgwrvy4fb4xz340000gn/T/tmp.195RIPoa8f/src/source_changed.html",
                "http_status": null,
                "content_type": null,
                "bytes_read": 43,
                "duration_ms": 0
            },
            "extraction": {
                "compare_source_sha256": "703390318bd55aef50b7823d2b90a846debff99e6e3d401a24a921b733912a6d",
                "outer_html_sha256": "fc3fa5f17bbf31ef2b6824e3d356850b7f822812d72512729db4794dba83dfda",
                "selection_kind": "css_selector",
                "selection_match": "single",
                "candidate_count": 1,
                "selected_candidate_index": 1,
                "warning_codes": [],
                "duration_ms": 1
            },
            "compare": {
                "canonicalizers": [],
                "duration_ms": 0
            },
            "change": {
                "kind": "initialized",
                "current_compare_bytes": 4,
                "current_compare_line_count": 1,
                "common_prefix_lines": 0,
                "common_suffix_lines": 0,
                "changed_region": {
                    "previous_start_line": 1,
                    "previous_line_count": 0,
                    "current_start_line": 1,
                    "current_line_count": 1,
                    "current_excerpt": "Beta",
                    "current_excerpt_sha256": "703390318bd55aef50b7823d2b90a846debff99e6e3d401a24a921b733912a6d"
                }
            },
            "persist": {
                "state_commit_duration_ms": 0,
                "state_commit": { "status": "not_attempted" },
                "last_run_write_duration_ms": 0,
                "last_run_write": { "status": "not_attempted" }
            }
        }))
    }

    fn dry_run_batch_report() -> BatchRunReport {
        let demo_a = serde_json::to_value(dry_run_initialized_run_report("demo-a"))
            .expect("demo-a run report");
        let demo_b = serde_json::to_value(dry_run_initialized_run_report("demo-b"))
            .expect("demo-b run report");
        parse_batch_report(json!({
            "schema_name": "ffhn.batch_run_report",
            "schema_version": 4,
            "run_mode": "dry_run",
            "watch_root": "/var/folders/45/5nz735_d4_zgwrvy4fb4xz340000gn/T/tmp.195RIPoa8f/watch-batch",
            "requested_targets": ["demo-a", "demo-b"],
            "run_started_at": "2026-05-14T15:15:43.154874Z",
            "run_finished_at": "2026-05-14T15:15:43.158661Z",
            "max_concurrency": 1,
            "entries": [
                { "target_id": "demo-a", "run_report": demo_a },
                { "target_id": "demo-b", "run_report": demo_b }
            ],
            "outcome_counts": {
                "initialized": 2,
                "changed": 0,
                "unchanged": 0,
                "failed_transient": 0,
                "failed_permanent": 0,
                "skipped_disabled": 0,
                "persist_failure": 0,
                "notification_failure": 0,
                "fatal_error": 0
            }
        }))
    }

    fn with_valid_run_report_digest(mut map: serde_json::Map<String, Value>) -> Value {
        map.remove("run_report_digest_sha256");
        let stable = stable_json_value(&Value::Object(map.clone()));
        let digest = Sha256::digest(stable.as_bytes());
        map.insert(
            "run_report_digest_sha256".to_owned(),
            Value::String(digest.iter().map(|byte| format!("{byte:02x}")).collect()),
        );
        Value::Object(map)
    }

    fn stable_json_value(value: &Value) -> String {
        match value {
            Value::Null => "null".to_owned(),
            Value::Bool(value) => {
                if *value {
                    "true".to_owned()
                } else {
                    "false".to_owned()
                }
            }
            Value::Number(value) => value.to_string(),
            Value::String(value) => serde_json::to_string(value).expect("stable string"),
            Value::Array(values) => {
                let joined = values
                    .iter()
                    .map(stable_json_value)
                    .collect::<Vec<_>>()
                    .join(",");
                format!("[{joined}]")
            }
            Value::Object(map) => {
                let mut entries = map.iter().collect::<Vec<_>>();
                entries.sort_unstable_by_key(|(key, _)| *key);
                let joined = entries
                    .into_iter()
                    .map(|(key, value)| {
                        format!(
                            "{}:{}",
                            serde_json::to_string(key).expect("stable key"),
                            stable_json_value(value)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{{{joined}}}")
            }
        }
    }

    #[test]
    fn run_summary_renders_reportable_failure_partial_and_disabled_shapes() {
        let changed_report = changed_run_report();
        let fetch_failure_report = parse_run_report_object(json!({
            "schema_name":"ffhn.run_report",
            "schema_version":4,
            "target_id":"demo",
            "display_name":"Demo",
            "run_started_at":"2026-05-14T15:35:59.759681Z",
            "run_finished_at":"2026-05-14T15:35:59.764345Z",
            "run_mode":"live",
            "result":{"kind":"failed_transient","cause":"fetch_http_server_error","error_detail":{"kind":"contract","message":"HTTP request returned status 500","path":"http://127.0.0.1:53237/"}},"compare_basis":"text","baseline_phase_before_run":"never_succeeded","baseline_phase_after_run":"never_succeeded","fetch":{"engine":"http","final_url":"http://127.0.0.1:53237/","http_status":500,"content_type":"text/html","bytes_read":null,"duration_ms":2},"persist":{"state_commit_duration_ms":0,"state_commit":{"status":"not_attempted"},"last_run_write_duration_ms":0,"last_run_write":{"status":"written"}}
        }));
        let persist_failure_report = parse_run_report_object(json!({
            "schema_name":"ffhn.run_report",
            "schema_version":4,
            "target_id":"demo",
            "display_name":"Demo",
            "run_started_at":"2026-05-14T15:41:43.353707Z",
            "run_finished_at":"2026-05-14T15:41:43.360033Z",
            "run_mode":"live",
            "result":{"kind":"failed_transient","cause":"persist_error","error_detail":{"kind":"io","message":"Is a directory (os error 21)","path":"/Users/erst/Tools/ffhn/tmp/render-fixture-capture-current/watch/demo/state.json"}},"compare_basis":"text","current_compare_digest_sha256":"185f8db32271fe25f561a6fc938b2e264306ec304eda518007d1764826381969","baseline_phase_before_run":"never_succeeded","baseline_phase_after_run":"never_succeeded","fetch":{"engine":"http","final_url":"http://127.0.0.1:53349/","http_status":200,"content_type":"text/html; charset=utf-8","bytes_read":31,"duration_ms":3},"extraction":{"compare_source_sha256":"185f8db32271fe25f561a6fc938b2e264306ec304eda518007d1764826381969","outer_html_sha256":"116fc675d792391f14e2f8877730c76018d070210bb279064c2ed69416c3a149","selection_kind":"css_selector","selection_match":"single","candidate_count":1,"selected_candidate_index":1,"warning_codes":[],"duration_ms":0},"compare":{"canonicalizers":["trim"],"duration_ms":0},"change":{"kind":"initialized","current_compare_bytes":5,"current_compare_line_count":1,"common_prefix_lines":0,"common_suffix_lines":0,"changed_region":{"previous_start_line":1,"previous_line_count":0,"current_start_line":1,"current_line_count":1,"current_excerpt":"Hello","current_excerpt_sha256":"185f8db32271fe25f561a6fc938b2e264306ec304eda518007d1764826381969"}},"persist":{"state_commit_duration_ms":0,"state_commit":{"status":"failed","error":{"kind":"io","message":"Is a directory (os error 21)","path":"/Users/erst/Tools/ffhn/tmp/render-fixture-capture-current/watch/demo/state.json"}},"last_run_write_duration_ms":0,"last_run_write":{"status":"written"}}
        }));
        let disabled_report = parse_run_report_object(json!({
            "schema_name":"ffhn.run_report",
            "schema_version":4,
            "target_id":"demo",
            "display_name":"Demo",
            "run_started_at":"2026-05-14T15:35:59.777966Z",
            "run_finished_at":"2026-05-14T15:35:59.77834Z",
            "run_mode":"live",
            "result":{"kind":"skipped_disabled"},"compare_basis":"text","baseline_phase_before_run":"never_succeeded","baseline_phase_after_run":"never_succeeded","persist":{"state_commit_duration_ms":0,"state_commit":{"status":"written"},"last_run_write_duration_ms":0,"last_run_write":{"status":"written"}}
        }));
        let reportable_without_final_url = parse_run_report_value(with_valid_run_report_digest(
            json!({
                "schema_name": "ffhn.run_report",
                "schema_version": 4,
                "target_id": "payload_target",
                "display_name": "Payload Target",
                "run_started_at": "2026-05-14T15:24:54.420145Z",
                "run_finished_at": "2026-05-14T15:24:54.426475Z",
                "run_mode": "live",
                "result": { "kind": "changed" },
                "compare_basis": "text",
                "previous_compare_digest_sha256": "ee8abdc0db68120327dc6674b651c44f17e6df9769b233f7528fe1f52479f3a5",
                "current_compare_digest_sha256": "079522f2e24f6e98341baf703990cf8a79bce55eaaab8c70bd73e2f840c1c424",
                "baseline_phase_before_run": "has_baseline",
                "baseline_phase_after_run": "has_baseline",
                "fetch": {
                    "engine": "file",
                    "final_url": null,
                    "http_status": null,
                    "content_type": null,
                    "bytes_read": 55,
                    "duration_ms": 0
                },
                "extraction": {
                    "compare_source_sha256": "079522f2e24f6e98341baf703990cf8a79bce55eaaab8c70bd73e2f840c1c424",
                    "outer_html_sha256": "c028a2183b5d2f7b48e7415a5616ec4b72c9ef78f8c79afbc92ddf68db706541",
                    "selection_kind": "css_selector",
                    "selection_match": "single",                    "candidate_count": 1,
                    "selected_candidate_index": 1,
                    "warning_codes": [],
                    "duration_ms": 1
                },
                "compare": {
                    "canonicalizers": ["trim"],
                    "duration_ms": 0
                },
                "change": {
                    "kind": "changed",
                    "previous_compare_bytes": 12,
                    "current_compare_bytes": 15,
                    "previous_compare_line_count": 1,
                    "current_compare_line_count": 1,
                    "common_prefix_lines": 0,
                    "common_suffix_lines": 0,
                    "changed_region": {
                        "previous_start_line": 1,
                        "previous_line_count": 1,
                        "current_start_line": 1,
                        "current_line_count": 1,
                        "previous_excerpt": "Payload Seed",
                        "current_excerpt": "Payload Changed",
                        "previous_excerpt_sha256": "ee8abdc0db68120327dc6674b651c44f17e6df9769b233f7528fe1f52479f3a5",
                        "current_excerpt_sha256": "079522f2e24f6e98341baf703990cf8a79bce55eaaab8c70bd73e2f840c1c424"
                    }
                },
                "persist": {
                    "state_commit_duration_ms": 1,
                    "state_commit": { "status": "written" },
                    "last_run_write_duration_ms": 0,
                    "last_run_write": { "status": "written" }
                }
            })
            .as_object()
            .cloned()
            .expect("reportable without final url object"),
        ));
        let partial_extraction_report = parse_run_report_value(with_valid_run_report_digest(
            json!({
                "schema_name": "ffhn.run_report",
                "schema_version": 4,
                "target_id": "payload_target",
                "display_name": "Payload Target",
                "run_started_at": "2026-05-14T15:24:54.420145Z",
                "run_finished_at": "2026-05-14T15:24:54.426475Z",
                "run_mode": "live",
                "result": {
                    "kind": "failed_permanent",
                    "cause": "compare_error",
                    "error_detail": {
                        "kind": "htmlcut_interop",
                        "message": "compare stage failed"
                    }
                },
                "compare_basis": "text",
                "previous_compare_digest_sha256": "ee8abdc0db68120327dc6674b651c44f17e6df9769b233f7528fe1f52479f3a5",
                "baseline_phase_before_run": "has_baseline",
                "baseline_phase_after_run": "has_baseline",
                "fetch": {
                    "engine": "file",
                    "final_url": null,
                    "http_status": null,
                    "content_type": null,
                    "bytes_read": 55,
                    "duration_ms": 0
                },
                "extraction": {
                    "compare_source_sha256": "079522f2e24f6e98341baf703990cf8a79bce55eaaab8c70bd73e2f840c1c424",
                    "outer_html_sha256": "c028a2183b5d2f7b48e7415a5616ec4b72c9ef78f8c79afbc92ddf68db706541",
                    "selection_kind": "css_selector",
                    "selection_match": "single",                    "candidate_count": 1,
                    "selected_candidate_index": 1,
                    "warning_codes": [],
                    "duration_ms": 1
                },
                "persist": {
                    "state_commit_duration_ms": 0,
                    "state_commit": {
                        "status": "written"
                    },
                    "last_run_write_duration_ms": 0,
                    "last_run_write": {
                        "status": "written"
                    }
                }
            })
            .as_object()
            .cloned()
            .expect("partial extraction report object"),
        ));
        let no_display_name_report = parse_run_report_value(with_valid_run_report_digest(
            json!({
                "schema_name": "ffhn.run_report",
                "schema_version": 4,
                "target_id": "demo",
                "run_started_at": "2026-05-14T15:35:59.759681Z",
                "run_finished_at": "2026-05-14T15:35:59.764345Z",
                "run_mode": "live",
                "result": {
                    "kind": "failed_permanent",
                    "cause": "config_invalid",
                    "error_detail": {
                        "kind": "contract",
                        "message": "target contract validation failed"
                    }
                },
                "compare_basis": "text",
                "baseline_phase_before_run": "never_succeeded",
                "baseline_phase_after_run": "never_succeeded",
                "persist": {
                    "state_commit_duration_ms": 0,
                    "state_commit": { "status": "written" },
                    "last_run_write_duration_ms": 0,
                    "last_run_write": { "status": "written" }
                }
            })
            .as_object()
            .cloned()
            .expect("no display name report object"),
        ));
        let partial_compare_report = parse_run_report_value(with_valid_run_report_digest(
            json!({
                "schema_name": "ffhn.run_report",
                "schema_version": 4,
                "target_id": "demo",
                "display_name": "Demo",
                "run_started_at": "2026-05-14T15:41:43.353707Z",
                "run_finished_at": "2026-05-14T15:41:43.360033Z",
                "run_mode": "live",
                "result": {
                    "kind": "failed_transient",
                    "cause": "persist_error",
                    "error_detail": {
                        "kind": "io",
                        "message": "Is a directory (os error 21)",
                        "path": "/Users/erst/Tools/ffhn/tmp/render-fixture-capture-current/watch/demo/state.json"
                    }
                },
                "compare_basis": "text",
                "current_compare_digest_sha256": "185f8db32271fe25f561a6fc938b2e264306ec304eda518007d1764826381969",
                "baseline_phase_before_run": "never_succeeded",
                "baseline_phase_after_run": "never_succeeded",
                "fetch": {
                    "engine": "http",
                    "final_url": null,
                    "http_status": 200,
                    "content_type": "text/html; charset=utf-8",
                    "bytes_read": 31,
                    "duration_ms": 3
                },
                "extraction": {
                    "compare_source_sha256": "185f8db32271fe25f561a6fc938b2e264306ec304eda518007d1764826381969",
                    "outer_html_sha256": "116fc675d792391f14e2f8877730c76018d070210bb279064c2ed69416c3a149",
                    "selection_kind": "css_selector",
                    "selection_match": "single",                    "candidate_count": 1,
                    "selected_candidate_index": 1,
                    "warning_codes": [],
                    "duration_ms": 0
                },
                "compare": {
                    "canonicalizers": ["trim"],
                    "duration_ms": 0
                },
                "persist": {
                    "state_commit_duration_ms": 0,
                    "state_commit": {
                        "status": "failed",
                        "error": {
                            "kind": "io",
                            "message": "Is a directory (os error 21)",
                            "path": "/Users/erst/Tools/ffhn/tmp/render-fixture-capture-current/watch/demo/state.json"
                        }
                    },
                    "last_run_write_duration_ms": 0,
                    "last_run_write": {
                        "status": "written"
                    }
                },
                "notifications": [
                    {
                        "route_name": "notify-success",
                        "duration_ms": 4,
                        "outcome": {
                            "status": "delivered",
                            "exit_code": 0
                        }
                    },
                    {
                        "route_name": "notify-timeout",
                        "duration_ms": 7,
                        "outcome": {
                            "status": "timed_out",
                            "error": "route timed out"
                        }
                    }
                ]
            })
            .as_object()
            .cloned()
            .expect("partial compare report object"),
        ));

        let mut stdout = Vec::new();
        render_run_report(&mut stdout, &changed_report, OutputFormat::Summary).expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("Run report"));
        assert!(rendered.contains("Display name: Payload Target"));
        assert!(rendered.contains("Outcome: changed"));
        assert!(
            rendered
                .contains("Final URL: file:///Users/erst/Tools/ffhn/tmp/seed-refresh/source.html")
        );
        assert!(rendered.contains("Change: kind=changed"));
        assert!(rendered.contains("Notifications: none"));

        let mut stdout = Vec::new();
        render_run_report(
            &mut stdout,
            &reportable_without_final_url,
            OutputFormat::Summary,
        )
        .expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("Outcome: changed"));
        assert!(!rendered.contains("Final URL:"));

        let mut stdout = Vec::new();
        render_run_report(&mut stdout, &fetch_failure_report, OutputFormat::Summary)
            .expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("Outcome: failed_transient (fetch_http_server_error)"));
        assert!(rendered.contains("Error: contract - HTTP request returned status 500"));
        assert!(rendered.contains("Error path: http://127.0.0.1:53237/"));
        assert!(rendered.contains("HTTP status: 500"));

        let mut stdout = Vec::new();
        render_run_report(&mut stdout, &persist_failure_report, OutputFormat::Summary)
            .expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("Outcome: failed_transient (persist_error)"));
        assert!(rendered.contains("Fetch: engine=http, bytes=31, duration=3 ms"));
        assert!(
            rendered
                .contains("Extraction: kind=css_selector, match=single, candidates=1, selected=1")
        );
        assert!(rendered.contains("Compare: basis=text"));
        assert!(rendered.contains("Change: kind=initialized"));
        assert!(rendered.contains("Persist: state_commit=failed"));

        let mut stdout = Vec::new();
        render_run_report(
            &mut stdout,
            &partial_extraction_report,
            OutputFormat::Summary,
        )
        .expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("Outcome: failed_permanent (compare_error)"));
        assert!(rendered.contains("Progress: fetch completed; fetch_engine=file, bytes=55, duration=0 ms; extraction_candidates=1"));
        assert!(!rendered.contains("compare_duration="));

        let mut stdout = Vec::new();
        render_run_report(&mut stdout, &partial_compare_report, OutputFormat::Summary)
            .expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("Outcome: failed_transient (persist_error)"));
        assert!(rendered.contains("compare_duration=0 ms"));
        assert!(!rendered.contains("Final URL:"));
        assert!(rendered.contains("Notifications: 2"));
        assert!(rendered.contains("- notify-success: delivered (4 ms, exit_code=0)"));
        assert!(rendered.contains("- notify-timeout: timed_out (7 ms)"));
        assert!(rendered.contains("  error: route timed out"));

        let mut stdout = Vec::new();
        render_run_report(&mut stdout, &no_display_name_report, OutputFormat::Summary)
            .expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("Target: demo"));
        assert!(!rendered.contains("Display name:"));

        let mut stdout = Vec::new();
        render_run_report(&mut stdout, &disabled_report, OutputFormat::Summary).expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("Outcome: skipped_disabled"));
        assert!(!rendered.contains("Fetch: engine="));
        assert!(!rendered.contains("Progress: fetch completed"));
    }

    #[test]
    fn status_summary_renders_ready_and_unavailable_contracts() {
        let ready_report = parse_status_report(json!({
            "schema_name": "ffhn.status_report",
            "schema_version": 5,
            "target_id": "demo",
            "display_name": "Demo",
            "enabled": true,
            "status": {
                "kind": "ready",
                "current_snapshot": {
                    "compare_digest_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "outer_html_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "captured_at": "2026-05-14T15:00:00Z"
                },
                "snapshot_history": [{
                    "compare_digest_sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                    "outer_html_sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                    "captured_at": "2026-05-14T14:59:00Z"
                }]
            }
        }));
        let unavailable_report = parse_status_report(json!({
            "schema_name": "ffhn.status_report",
            "schema_version": 5,
            "target_id": "demo",
            "status": {
                "kind": "unavailable_target",
                "error_detail": {
                    "kind": "io",
                    "message": "No such file or directory (os error 2)",
                    "path": "/tmp/ffhn/watch/demo/target.toml"
                }
            }
        }));

        let mut stdout = Vec::new();
        render_status_report(&mut stdout, &ready_report, OutputFormat::Summary).expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("Status report"));
        assert!(rendered.contains("Display name: Demo"));
        assert!(rendered.contains("Enabled: true"));
        assert!(rendered.contains("Status: ready"));
        assert!(rendered.contains("Baseline phase: has_baseline"));
        assert!(rendered.contains("Current snapshot: compare="));
        assert!(rendered.contains("Snapshot history: 1"));

        let mut stdout = Vec::new();
        render_status_report(&mut stdout, &unavailable_report, OutputFormat::Summary)
            .expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("Status: unavailable_target"));
        assert!(rendered.contains("Error: io - No such file or directory"));
        assert!(rendered.contains("/tmp/ffhn/watch/demo/target.toml"));
    }

    #[test]
    fn json_and_pretty_renderers_emit_machine_documents_with_trailing_newlines() {
        let run_report = changed_run_report();
        let status_report = parse_status_report(json!({
            "schema_name": "ffhn.status_report",
            "schema_version": 5,
            "target_id": "demo",
            "display_name": "Demo",
            "enabled": true,
            "status": { "kind": "pending" }
        }));
        let batch_report = dry_run_batch_report();

        let mut stdout = Vec::new();
        render_run_report(&mut stdout, &run_report, OutputFormat::JsonPretty).expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("\n  \"schema_name\": \"ffhn.run_report\""));
        assert!(rendered.ends_with("}\n"));

        let mut stdout = Vec::new();
        render_status_report(&mut stdout, &status_report, OutputFormat::Json).expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("\"schema_name\":\"ffhn.status_report\""));
        assert!(rendered.ends_with("}\n"));

        let mut stdout = Vec::new();
        render_batch_report(&mut stdout, &batch_report, OutputFormat::JsonPretty).expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("\n  \"schema_name\": \"ffhn.batch_run_report\""));
        assert!(rendered.ends_with("}\n"));
    }

    #[test]
    fn stable_json_helper_covers_scalar_array_and_object_variants() {
        let rendered = stable_json_value(&json!({
            "null_value": null,
            "bool_true": true,
            "bool_false": false,
            "number": 7,
            "string": "demo",
            "array": [1, false, {"b": 2, "a": 1}]
        }));

        assert_eq!(
            rendered,
            r#"{"array":[1,false,{"a":1,"b":2}],"bool_false":false,"bool_true":true,"null_value":null,"number":7,"string":"demo"}"#
        );
    }

    #[test]
    fn batch_summary_renders_success_entries_and_fatal_entries() {
        let dry_run_batch = dry_run_batch_report();
        let changed_run = serde_json::to_value(changed_run_report()).expect("run report json");
        let mixed_batch = parse_batch_report(json!({
            "schema_name": "ffhn.batch_run_report",
            "schema_version": 4,
            "run_mode": "live",
            "watch_root": "/tmp/ffhn/watch-root",
            "requested_targets": ["payload_target", "fatal_target"],
            "run_started_at": "2026-05-14T15:00:00Z",
            "run_finished_at": "2026-05-14T15:00:01Z",
            "max_concurrency": 2,
            "entries": [
                {
                    "target_id": "payload_target",
                    "run_report": changed_run
                },
                {
                    "target_id": "fatal_target",
                    "fatal_error": {
                        "kind": "contract",
                        "message": "target contract validation failed",
                        "path": "/tmp/ffhn/watch-root/fatal_target/target.toml"
                    }
                }
            ],
            "outcome_counts": {
                "initialized": 0,
                "changed": 1,
                "unchanged": 0,
                "failed_transient": 0,
                "failed_permanent": 0,
                "skipped_disabled": 0,
                "persist_failure": 0,
                "notification_failure": 0,
                "fatal_error": 1
            }
        }));
        let fatal_without_path = parse_batch_report(json!({
            "schema_name": "ffhn.batch_run_report",
            "schema_version": 4,
            "run_mode": "live",
            "watch_root": "/tmp/ffhn/watch-root",
            "requested_targets": ["fatal_target"],
            "run_started_at": "2026-05-14T15:00:00Z",
            "run_finished_at": "2026-05-14T15:00:01Z",
            "max_concurrency": 1,
            "entries": [
                {
                    "target_id": "fatal_target",
                    "fatal_error": {
                        "kind": "internal",
                        "message": "worker crashed"
                    }
                }
            ],
            "outcome_counts": {
                "initialized": 0,
                "changed": 0,
                "unchanged": 0,
                "failed_transient": 0,
                "failed_permanent": 0,
                "skipped_disabled": 0,
                "persist_failure": 0,
                "notification_failure": 0,
                "fatal_error": 1
            }
        }));

        let mut stdout = Vec::new();
        render_batch_report(&mut stdout, &dry_run_batch, OutputFormat::Summary).expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("Batch run report"));
        assert!(rendered.contains("Mode: dry_run"));
        assert!(rendered.contains("Outcome counts: initialized=2"));
        assert!(rendered.contains("demo-a"));
        assert!(rendered.contains("initialized"));

        let mut stdout = Vec::new();
        render_batch_report(&mut stdout, &mixed_batch, OutputFormat::Summary).expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("- payload_target (Payload Target): changed"));
        assert!(rendered.contains("- fatal_target: fatal_error [/tmp/ffhn/watch-root/fatal_target/target.toml] - target contract validation failed"));

        let mut stdout = Vec::new();
        render_batch_report(&mut stdout, &fatal_without_path, OutputFormat::Summary)
            .expect("render");
        let rendered = String::from_utf8(stdout).expect("utf8");
        assert!(rendered.contains("- fatal_target: fatal_error - worker crashed"));
    }
}

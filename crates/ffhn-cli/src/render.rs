use std::io::{self, Write};

use serde::Serialize;

use ffhn_core::{
    BatchRunReport, DeliveryOutcome, LifecycleFacet, LifecycleSnapshot, OutboxOverflow,
    PolicyEvaluation, ResetReport, RunReport, StatusReport,
};

use crate::args::OutputFormat;

mod diagnostic;

/// Renders a single run report.
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
/// Renders a batch run report.
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
/// Renders a status report.
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
/// Renders a reset report.
pub fn render_reset_report(
    stdout: &mut (impl Write + ?Sized),
    report: &ResetReport,
    format: OutputFormat,
) -> io::Result<()> {
    match format {
        OutputFormat::Json => render_json_document(stdout, report),
        OutputFormat::JsonPretty => render_pretty_json_document(stdout, report),
        OutputFormat::Summary => render_reset_summary(stdout, report),
    }
}

fn render_json_document(
    stdout: &mut (impl Write + ?Sized),
    document: &impl Serialize,
) -> io::Result<()> {
    serde_json::to_writer(&mut *stdout, document).map_err(io::Error::other)?;
    writeln!(stdout)
}

fn render_pretty_json_document(
    stdout: &mut (impl Write + ?Sized),
    document: &impl Serialize,
) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *stdout, document).map_err(io::Error::other)?;
    writeln!(stdout)
}

fn render_run_summary(stdout: &mut (impl Write + ?Sized), report: &RunReport) -> io::Result<()> {
    writeln!(stdout, "Run report")?;
    writeln!(stdout, "Target: {}", report.target_id())?;
    if let Some(display_name) = report.display_name() {
        writeln!(stdout, "Display name: {display_name}")?;
    }
    writeln!(stdout, "Mode: {}", report.run_mode().as_str())?;
    writeln!(stdout, "Outcome: {}", report.outcome().as_str())?;
    writeln!(stdout, "Started: {}", report.run_started_at())?;
    writeln!(stdout, "Finished: {}", report.run_finished_at())?;
    if let Some(observation) = report.observation() {
        writeln!(stdout, "Observation: {}", observation.canonical_value())?;
    }
    if let Some(previous) = report.previous_canonical_value() {
        writeln!(stdout, "Previous canonical value: {previous}")?;
    }
    if let Some(digest) = report.contract_digest_sha256() {
        writeln!(stdout, "Contract digest: {digest}")?;
    }
    if let Some(detail) = report.error_detail() {
        diagnostic::render_error_detail(stdout, detail)?;
    }
    render_policy_evaluation(stdout, report.policy_evaluation())?;
    render_run_lifecycle(stdout, report.lifecycle())?;
    writeln!(stdout, "State persisted: {}", report.state_persisted())?;
    render_delivery_evidence(
        stdout,
        report.delivery_outcomes(),
        report.outbox_overflow(),
        report.outbox_error_detail(),
    )
}

fn render_batch_summary(
    stdout: &mut (impl Write + ?Sized),
    report: &BatchRunReport,
) -> io::Result<()> {
    writeln!(stdout, "Batch run report")?;
    writeln!(stdout, "Mode: {}", report.run_mode().as_str())?;
    writeln!(
        stdout,
        "Requested targets: {}",
        report.requested_targets().join(", ")
    )?;
    writeln!(stdout, "Reports: {}", report.reports().len())?;
    for run in report.reports() {
        writeln!(stdout)?;
        render_run_summary(stdout, run)?;
    }
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
    writeln!(stdout, "Status: {}", report.kind().as_str())?;
    if let Some(observation) = report.accepted_observation() {
        writeln!(
            stdout,
            "Accepted observation: {}",
            observation.canonical_value()
        )?;
    }
    if let Some(digest) = report.contract_digest_sha256() {
        writeln!(stdout, "Contract digest: {digest}")?;
    }
    if let Some(detail) = report.error_detail() {
        diagnostic::render_error_detail(stdout, detail)?;
    }
    match report.lifecycle() {
        Some(lifecycle) => render_lifecycle_snapshot(stdout, "Lifecycle", lifecycle),
        None => writeln!(stdout, "Lifecycle: unavailable"),
    }
}

fn render_reset_summary(
    stdout: &mut (impl Write + ?Sized),
    report: &ResetReport,
) -> io::Result<()> {
    writeln!(stdout, "Reset report")?;
    writeln!(stdout, "Target: {}", report.target_id())?;
    writeln!(stdout, "Storage cleared: {}", report.storage_cleared())?;
    render_delivery_evidence(
        stdout,
        report.delivery_outcomes(),
        report.outbox_overflow(),
        report.outbox_error_detail(),
    )
}

fn render_policy_evaluation(
    stdout: &mut (impl Write + ?Sized),
    policy: &PolicyEvaluation,
) -> io::Result<()> {
    match policy.condition_results() {
        Some(results) => {
            writeln!(stdout, "Policy: evaluated ({} conditions)", results.len())?;
            if results.is_empty() {
                writeln!(stdout, "Policy conditions: none configured")?;
            }
            for result in results {
                write!(
                    stdout,
                    "- Condition {}: outcome={}, triggered={}, active={} -> {}",
                    result.condition_id(),
                    result.outcome().as_str(),
                    result.triggered(),
                    result.active_before(),
                    result.active_after(),
                )?;
                if let Some(reference) = result.reference() {
                    write!(stdout, ", reference={}", reference.reference().as_str())?;
                    match reference.canonical_value() {
                        Some(value) => write!(stdout, "={value}")?,
                        None => write!(stdout, " (unavailable)")?,
                    }
                }
                writeln!(stdout)?;
            }
        }
        None => writeln!(stdout, "Policy: not evaluated")?,
    }
    let events = policy.event_eligibilities();
    if events.is_empty() {
        return writeln!(stdout, "Policy events: none eligible");
    }
    writeln!(stdout, "Policy events: {} eligible", events.len())?;
    for event in events {
        write!(
            stdout,
            "- Event: kind={}, route_family={}",
            event.event_kind().as_str(),
            event.route_family().as_str(),
        )?;
        if let Some(condition_id) = event.condition_id() {
            write!(stdout, ", condition={condition_id}")?;
        }
        if let Some(reason) = event.reason_class() {
            write!(stdout, ", source_reason={}", reason.as_str())?;
        }
        if let Some(code) = event.error_code() {
            write!(stdout, ", permanent_error={}", code.as_str())?;
        }
        if let Some(code) = event.integration_fault_code() {
            write!(stdout, ", integration_fault={}", code.as_str())?;
        }
        writeln!(stdout)?;
    }
    Ok(())
}

fn render_run_lifecycle(
    stdout: &mut (impl Write + ?Sized),
    lifecycle: &LifecycleFacet,
) -> io::Result<()> {
    match lifecycle.before() {
        Some(snapshot) => render_lifecycle_snapshot(stdout, "Lifecycle before", snapshot)?,
        None => writeln!(stdout, "Lifecycle before: unavailable")?,
    }
    match lifecycle.after() {
        Some(snapshot) => render_lifecycle_snapshot(stdout, "Lifecycle after", snapshot),
        None => writeln!(stdout, "Lifecycle after: not staged"),
    }
}

fn render_lifecycle_snapshot(
    stdout: &mut (impl Write + ?Sized),
    heading: &str,
    snapshot: &LifecycleSnapshot,
) -> io::Result<()> {
    let source_health = snapshot.source_health();
    writeln!(
        stdout,
        "{heading}: source_health.state={}, reason_class={}, consecutive_unresolved={}",
        source_health.state().as_str(),
        source_health
            .reason_class()
            .map_or("none", |reason| reason.as_str()),
        source_health.consecutive_unresolved(),
    )?;
    if let Some(first_unresolved_at) = source_health.first_unresolved_at() {
        writeln!(
            stdout,
            "{heading}: source_health.first_unresolved_at={first_unresolved_at}"
        )?;
    }
    if let Some(details) = source_health.last_details() {
        writeln!(stdout, "{heading}: source_health.last_details:")?;
        diagnostic::render_error_detail(stdout, details)?;
    }
    match snapshot.permanent_error_episode() {
        Some(episode) => writeln!(
            stdout,
            "{heading}: permanent_error_episode.code={}, first_seen_at={}",
            episode.error_code().as_str(),
            episode.first_seen_at(),
        )?,
        None => writeln!(stdout, "{heading}: permanent_error_episode=none")?,
    }
    match snapshot.integration_fault_episode() {
        Some(episode) => writeln!(
            stdout,
            "{heading}: integration_fault_episode.code={}, first_seen_at={}",
            episode.code().as_str(),
            episode.first_seen_at(),
        ),
        None => writeln!(stdout, "{heading}: integration_fault_episode=none"),
    }
}

fn render_delivery_evidence(
    stdout: &mut (impl Write + ?Sized),
    outcomes: &[DeliveryOutcome],
    overflow: &[OutboxOverflow],
    outbox_error_detail: Option<&ffhn_core::DiagnosticDetail>,
) -> io::Result<()> {
    if outcomes.is_empty() {
        writeln!(stdout, "Delivery outcomes: none")?;
    } else {
        writeln!(stdout, "Delivery outcomes: {}", outcomes.len())?;
        for outcome in outcomes {
            write!(
                stdout,
                "- Delivery: event_id={}, route={}, kind={}, status={}, attempts={}",
                outcome.event_id(),
                outcome.route_id(),
                outcome.event_kind().as_str(),
                outcome.status().as_str(),
                outcome.attempt_count(),
            )?;
            if let Some(condition_id) = outcome.condition_id() {
                write!(stdout, ", condition={condition_id}")?;
            }
            writeln!(stdout)?;
            if let Some(detail) = outcome.error_detail() {
                writeln!(stdout, "  Delivery failure:")?;
                diagnostic::render_error_detail(stdout, detail)?;
            }
            if let Some(detail) = outcome.outbox_error_detail() {
                writeln!(stdout, "  Outbox persistence failure:")?;
                diagnostic::render_error_detail(stdout, detail)?;
            }
            if let Some(detail) = outcome.delivery_observability_detail() {
                writeln!(stdout, "  Delivery observability:")?;
                diagnostic::render_error_detail(stdout, detail)?;
            }
        }
    }
    if overflow.is_empty() {
        writeln!(stdout, "Outbox overflow: none")?;
    } else {
        writeln!(stdout, "Outbox overflow: {}", overflow.len())?;
        for item in overflow {
            write!(
                stdout,
                "- Overflow: event_id={}, route={}, kind={}",
                item.event_id(),
                item.route_id(),
                item.event_kind().as_str(),
            )?;
            if let Some(condition_id) = item.condition_id() {
                write!(stdout, ", condition={condition_id}")?;
            }
            writeln!(stdout)?;
        }
    }
    if let Some(detail) = outbox_error_detail {
        writeln!(stdout, "Outbox error:")?;
        diagnostic::render_error_detail(stdout, detail)?;
    }
    Ok(())
}

#[cfg(test)]
use diagnostic::{operation_label, render_error_detail};

#[cfg(test)]
mod tests;

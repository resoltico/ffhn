use std::io::{self, Write};

use serde::Serialize;

use ffhn_core::{BatchRunReport, ResetReport, RunReport, StatusReport};

use crate::args::OutputFormat;

/// Renders a single run report.
pub fn render_run_report(
    stdout: &mut (impl Write + ?Sized),
    report: &RunReport,
    format: OutputFormat,
) -> io::Result<()> {
    render(stdout, report, format)
}
/// Renders a batch run report.
pub fn render_batch_report(
    stdout: &mut (impl Write + ?Sized),
    report: &BatchRunReport,
    format: OutputFormat,
) -> io::Result<()> {
    render(stdout, report, format)
}
/// Renders a status report.
pub fn render_status_report(
    stdout: &mut (impl Write + ?Sized),
    report: &StatusReport,
    format: OutputFormat,
) -> io::Result<()> {
    render(stdout, report, format)
}
/// Renders a reset report.
pub fn render_reset_report(
    stdout: &mut (impl Write + ?Sized),
    report: &ResetReport,
    format: OutputFormat,
) -> io::Result<()> {
    render(stdout, report, format)
}

fn render(
    stdout: &mut (impl Write + ?Sized),
    document: &impl Serialize,
    format: OutputFormat,
) -> io::Result<()> {
    match format {
        OutputFormat::Json => {
            serde_json::to_writer(&mut *stdout, document).map_err(io::Error::other)?
        }
        OutputFormat::JsonPretty | OutputFormat::Summary => {
            serde_json::to_writer_pretty(&mut *stdout, document).map_err(io::Error::other)?
        }
    }
    writeln!(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("writer failed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn run_report() -> RunReport {
        serde_json::from_str(
            r#"{"schema_name":"ffhn.run_report","schema_version":7,"target_id":"demo","run_mode":"live","outcome":"initialized","run_started_at":"2026-01-01T00:00:00Z","run_finished_at":"2026-01-01T00:00:01Z","state_persisted":true,"delivery_outcomes":[],"outbox_overflow":[]}"#,
        )
        .expect("run report")
    }

    #[test]
    fn renders_each_v2_document_in_every_output_mode() {
        let run = run_report();
        let batch: BatchRunReport = serde_json::from_value(serde_json::json!({
            "schema_name": "ffhn.batch_run_report",
            "schema_version": 7,
            "run_mode": "live",
            "requested_targets": ["demo"],
            "reports": [run],
        }))
        .expect("batch");
        let status: StatusReport = serde_json::from_str(
            r#"{"schema_name":"ffhn.status_report","schema_version":6,"target_id":"demo","kind":"pending"}"#,
        )
        .expect("status");
        let reset: ResetReport = serde_json::from_str(
            r#"{"schema_name":"ffhn.reset_report","schema_version":3,"target_id":"demo","storage_cleared":false,"delivery_outcomes":[],"outbox_overflow":[]}"#,
        )
        .expect("reset");
        let mut output = Vec::new();
        render_run_report(&mut output, &run_report(), OutputFormat::Json).expect("run JSON");
        render_batch_report(&mut output, &batch, OutputFormat::JsonPretty).expect("batch pretty");
        render_status_report(&mut output, &status, OutputFormat::Summary).expect("status summary");
        render_reset_report(&mut output, &reset, OutputFormat::Json).expect("reset JSON");
        assert!(
            String::from_utf8(output)
                .expect("UTF-8")
                .contains("ffhn.reset_report")
        );
        let mut failing = FailingWriter;
        assert!(render_run_report(&mut failing, &run_report(), OutputFormat::Json).is_err());
        assert!(failing.flush().is_ok());
    }
}

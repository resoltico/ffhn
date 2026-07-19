//! Human rendering of the closed diagnostic evidence vocabulary.

use std::io::{self, Write};

use ffhn_core::{
    DiagnosticDetail, DiagnosticOperation, FetchFailureDetails, HtmlcutBoundaryEvidence,
    HtmlcutDiagnosticDetails,
};

/// Renders every typed diagnostic fact while deliberately excluding retained stderr text.
pub(super) fn render_error_detail(
    stdout: &mut (impl Write + ?Sized),
    detail: &DiagnosticDetail,
) -> io::Result<()> {
    let io_class = detail
        .io_error_class()
        .map(|class| format!(" [{}]", class.as_str()))
        .unwrap_or_default();
    write!(
        stdout,
        "Error [{}] during {}{}: ",
        detail.kind().as_str(),
        operation_label(detail.operation()),
        io_class,
    )?;
    render_text(stdout, detail.message())?;
    writeln!(stdout)?;
    if let Some(truncation) = detail.message_truncation() {
        writeln!(
            stdout,
            "Diagnostic message truncation: original_len_bytes={}, original_sha256={}",
            truncation.original_len_bytes(),
            truncation.original_sha256(),
        )?;
    }
    if let Some(path) = detail.path() {
        write!(stdout, "Error path: ")?;
        render_text(stdout, path)?;
        writeln!(stdout)?;
    }
    if let Some(fetch_failure) = detail.fetch_failure() {
        render_fetch_failure(stdout, fetch_failure)?;
    }
    if let Some(htmlcut_failure) = detail.htmlcut_failure() {
        render_htmlcut_failure(stdout, htmlcut_failure)?;
    }
    if let Some(code) = detail.integration_fault_code() {
        writeln!(stdout, "Integration fault: {}", code.as_str())?;
    }
    render_delivery_process_facts(stdout, detail)
}

/// Renders strings on one summary line without losing control characters to formatting.
fn render_text(stdout: &mut (impl Write + ?Sized), value: &str) -> io::Result<()> {
    if value.chars().any(char::is_control) {
        serde_json::to_writer(&mut *stdout, value).map_err(io::Error::other)
    } else {
        write!(stdout, "{value}")
    }
}

fn render_fetch_failure(
    stdout: &mut (impl Write + ?Sized),
    failure: &FetchFailureDetails,
) -> io::Result<()> {
    match failure {
        FetchFailureDetails::HttpStatus { status } => {
            writeln!(stdout, "Fetch failure: http_status (status={status})")
        }
        FetchFailureDetails::HttpContentLengthExceeded {
            configured_max_bytes,
            content_length,
        } => writeln!(
            stdout,
            "Fetch failure: http_content_length_exceeded (configured_max_bytes={configured_max_bytes}, content_length={content_length})"
        ),
        FetchFailureDetails::BodyBytesExceeded {
            configured_max_bytes,
            observed_bytes,
        } => writeln!(
            stdout,
            "Fetch failure: body_bytes_exceeded (configured_max_bytes={configured_max_bytes}, observed_bytes={observed_bytes})"
        ),
        FetchFailureDetails::InvalidUtf8 => writeln!(stdout, "Fetch failure: invalid_utf8"),
    }
}

fn render_htmlcut_failure(
    stdout: &mut (impl Write + ?Sized),
    failure: &ffhn_core::HtmlcutFailureDetails,
) -> io::Result<()> {
    writeln!(
        stdout,
        "HTMLCut error class: {}",
        failure.error_class().as_str()
    )?;
    if let Some(core_diagnostic_code) = failure.core_diagnostic_code() {
        writeln!(
            stdout,
            "HTMLCut core diagnostic code: {}",
            core_diagnostic_code.as_str()
        )?;
    }
    if let Some(candidate_count) = failure.candidate_count() {
        writeln!(stdout, "HTMLCut candidate count: {candidate_count}")?;
    }
    if let Some(boundary_evidence) = failure.boundary_evidence() {
        match boundary_evidence {
            HtmlcutBoundaryEvidence::SelectedMatchCount {
                selected_match_count,
            } => writeln!(
                stdout,
                "HTMLCut boundary evidence: selected_match_count={selected_match_count}"
            )?,
            HtmlcutBoundaryEvidence::RequestedCssAttribute { attribute } => {
                write!(
                    stdout,
                    "HTMLCut boundary evidence: requested_css_attribute="
                )?;
                render_text(stdout, attribute)?;
                writeln!(stdout)?;
            }
        }
    }
    writeln!(
        stdout,
        "HTMLCut plan digest: {}",
        failure.plan_digest_sha256()
    )?;
    if let Some(selector_parse) = failure.selector_parse() {
        render_selector_parse(stdout, "HTMLCut selector parse", selector_parse)?;
    }
    writeln!(
        stdout,
        "HTMLCut diagnostics: {}",
        failure.diagnostics().len()
    )?;
    for diagnostic in failure.diagnostics() {
        write!(
            stdout,
            "HTMLCut diagnostic: level={}, code={}, message=",
            diagnostic.level(),
            diagnostic.code(),
        )?;
        render_text(stdout, diagnostic.message())?;
        writeln!(stdout)?;
        if let Some(details) = diagnostic.details() {
            render_htmlcut_diagnostic_details(stdout, details)?;
        }
    }
    Ok(())
}

fn render_htmlcut_diagnostic_details(
    stdout: &mut (impl Write + ?Sized),
    details: &HtmlcutDiagnosticDetails,
) -> io::Result<()> {
    match details {
        HtmlcutDiagnosticDetails::SelectorParse { selector_parse } => {
            render_selector_parse(stdout, "HTMLCut diagnostic selector parse", selector_parse)
        }
        HtmlcutDiagnosticDetails::CandidateSelection {
            candidate_count,
            requested_index,
            selected_index,
        } => writeln!(
            stdout,
            "HTMLCut candidate selection: candidate_count={candidate_count}, requested_index={}, selected_index={}",
            optional_number(*requested_index),
            optional_number(*selected_index),
        ),
        HtmlcutDiagnosticDetails::EffectiveBaseUrlUnresolved {
            document_base_href,
            rewrite_requested,
        } => {
            write!(stdout, "HTMLCut effective-base URL: document_base_href=")?;
            render_optional_text(stdout, document_base_href.as_deref())?;
            writeln!(stdout, ", rewrite_requested={rewrite_requested}")
        }
        HtmlcutDiagnosticDetails::SliceSplitsMarkup { affected_matches } => {
            for affected_match in affected_matches {
                let selected_range = affected_match.selected_range();
                writeln!(
                    stdout,
                    "HTMLCut markup split: match_index={}, candidate_index={}, selected_range=[{}, {})",
                    affected_match.match_index(),
                    affected_match.candidate_index(),
                    selected_range.start(),
                    selected_range.end(),
                )?;
            }
            Ok(())
        }
        HtmlcutDiagnosticDetails::SlicePattern {
            from,
            to,
            offset,
            pattern,
            flags,
        } => {
            write!(stdout, "HTMLCut slice pattern: from=")?;
            render_optional_text(stdout, from.as_deref())?;
            write!(stdout, ", to=")?;
            render_optional_text(stdout, to.as_deref())?;
            write!(stdout, ", offset={}", optional_number(*offset))?;
            write!(stdout, ", pattern=")?;
            render_optional_text(stdout, pattern.as_deref())?;
            write!(stdout, ", flags=")?;
            render_optional_text(stdout, flags.as_deref())?;
            writeln!(stdout)
        }
        HtmlcutDiagnosticDetails::UnsupportedValueType {
            strategy,
            value,
            path,
        } => {
            write!(stdout, "HTMLCut unsupported value type: strategy=")?;
            render_text(stdout, strategy)?;
            write!(stdout, ", value=")?;
            render_text(stdout, value)?;
            write!(stdout, ", path=")?;
            render_optional_text(stdout, path.as_deref())?;
            writeln!(stdout)
        }
        HtmlcutDiagnosticDetails::MissingAttribute {
            attribute,
            path,
            selected_range,
            hint,
        } => {
            write!(stdout, "HTMLCut missing attribute: attribute=")?;
            render_text(stdout, attribute)?;
            write!(stdout, ", path=")?;
            render_optional_text(stdout, path.as_deref())?;
            write!(stdout, ", selected_range=")?;
            match selected_range {
                Some(selected_range) => write!(
                    stdout,
                    "[{}, {})",
                    selected_range.start(),
                    selected_range.end(),
                )?,
                None => write!(stdout, "none")?,
            }
            write!(stdout, ", hint=")?;
            render_optional_text(stdout, hint.as_deref())?;
            writeln!(stdout)
        }
    }
}

fn render_selector_parse(
    stdout: &mut (impl Write + ?Sized),
    label: &str,
    selector_parse: &ffhn_core::HtmlcutSelectorParse,
) -> io::Result<()> {
    writeln!(
        stdout,
        "{label}: line={}, column_utf16={}, parse_error_class={}",
        selector_parse.line(),
        selector_parse.column_utf16(),
        selector_parse.parse_error_class().as_str(),
    )
}

fn render_optional_text(stdout: &mut (impl Write + ?Sized), value: Option<&str>) -> io::Result<()> {
    match value {
        Some(value) => serde_json::to_writer(&mut *stdout, value).map_err(io::Error::other),
        None => write!(stdout, "none"),
    }
}

const fn optional_number(value: Option<usize>) -> NumericSummary {
    NumericSummary(value)
}

struct NumericSummary(Option<usize>);

impl std::fmt::Display for NumericSummary {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(value) => write!(formatter, "{value}"),
            None => formatter.write_str("none"),
        }
    }
}

/// Renders closed process facts without exposing retained stderr text in human summaries.
fn render_delivery_process_facts(
    stdout: &mut (impl Write + ?Sized),
    detail: &DiagnosticDetail,
) -> io::Result<()> {
    if let Some((attempt, primary)) = detail.delivery_failure_facts() {
        match attempt.terminal() {
            ffhn_core::TerminalOutcome::NotStarted { io } => {
                writeln!(stdout, "Delivery terminal: not_started ({})", io.as_str())?;
            }
            ffhn_core::TerminalOutcome::StdinUnavailable => {
                writeln!(stdout, "Delivery terminal: stdin_unavailable")?;
            }
            ffhn_core::TerminalOutcome::Exited { exit_code } => match exit_code {
                Some(exit_code) => {
                    writeln!(stdout, "Delivery terminal: exited (exit_code={exit_code})")?;
                }
                None => {
                    writeln!(stdout, "Delivery terminal: exited (exit_code=unavailable)")?;
                }
            },
            ffhn_core::TerminalOutcome::TimedOut { timeout_ms } => {
                writeln!(
                    stdout,
                    "Delivery terminal: timed_out (timeout_ms={timeout_ms})"
                )?;
            }
            ffhn_core::TerminalOutcome::WaitFailed { io } => {
                writeln!(stdout, "Delivery terminal: wait_failed ({})", io.as_str())?;
            }
        }
        match attempt.writer() {
            ffhn_core::WriterOutcome::Completed => writeln!(stdout, "Delivery writer: completed")?,
            ffhn_core::WriterOutcome::IoFailed { io } => {
                writeln!(stdout, "Delivery writer: io_failed ({})", io.as_str())?;
            }
            ffhn_core::WriterOutcome::Panicked => writeln!(stdout, "Delivery writer: panicked")?,
            ffhn_core::WriterOutcome::NotAttempted => {
                writeln!(stdout, "Delivery writer: not_attempted")?;
            }
        }
        render_stderr_outcome(stdout, attempt.stderr())?;
        writeln!(stdout, "Delivery primary: {}", primary.as_str())?;
    } else if let Some(problem) = detail.stderr_capture_problem() {
        match problem {
            ffhn_core::StderrCaptureProblem::ReaderUnavailable => {
                writeln!(stdout, "Delivery stderr: reader_unavailable")?;
            }
            ffhn_core::StderrCaptureProblem::ReaderPanicked => {
                writeln!(stdout, "Delivery stderr: reader_panicked")?;
            }
            ffhn_core::StderrCaptureProblem::ReadFailed { io, partial } => {
                writeln!(stdout, "Delivery stderr: read_failed ({})", io.as_str())?;
                render_stderr_capture_metadata(stdout, partial)?;
            }
        }
    }
    Ok(())
}

fn render_stderr_outcome(
    stdout: &mut (impl Write + ?Sized),
    outcome: &ffhn_core::StderrOutcome,
) -> io::Result<()> {
    match outcome {
        ffhn_core::StderrOutcome::Absent => writeln!(stdout, "Delivery stderr: absent"),
        ffhn_core::StderrOutcome::Captured { capture } => {
            writeln!(stdout, "Delivery stderr: captured")?;
            render_stderr_capture_metadata(stdout, capture)
        }
        ffhn_core::StderrOutcome::ReadFailed { io, partial } => {
            writeln!(stdout, "Delivery stderr: read_failed ({})", io.as_str())?;
            render_stderr_capture_metadata(stdout, partial)
        }
        ffhn_core::StderrOutcome::ReaderUnavailable => {
            writeln!(stdout, "Delivery stderr: reader_unavailable")
        }
        ffhn_core::StderrOutcome::ReaderPanicked => {
            writeln!(stdout, "Delivery stderr: reader_panicked")
        }
    }
}

fn render_stderr_capture_metadata(
    stdout: &mut (impl Write + ?Sized),
    capture: &ffhn_core::StderrCapture,
) -> io::Result<()> {
    writeln!(
        stdout,
        "Delivery stderr metadata: retained_encoding={}, original_len_bytes={}, truncated={}",
        match capture.encoding() {
            ffhn_core::StderrEncoding::Utf8 => "utf8",
            ffhn_core::StderrEncoding::Utf8IncompleteAtRetentionBoundary => {
                "utf8_incomplete_at_retention_boundary"
            }
            ffhn_core::StderrEncoding::Utf8Lossy => "utf8_lossy",
        },
        capture.original_len_bytes(),
        capture.truncated(),
    )
}

pub(super) fn operation_label(operation: DiagnosticOperation) -> &'static str {
    match operation {
        DiagnosticOperation::TargetLoad => "target load",
        DiagnosticOperation::TargetValidation => "target validation",
        DiagnosticOperation::LockAcquire => "lock acquisition",
        DiagnosticOperation::StateLoad => "state load",
        DiagnosticOperation::StateCommit => "state commit",
        DiagnosticOperation::HttpFetch => "HTTP fetch",
        DiagnosticOperation::FileRead => "file read",
        DiagnosticOperation::JsonPointerSelection => "JSON Pointer selection",
        DiagnosticOperation::HtmlExtraction => "HTML extraction",
        DiagnosticOperation::ValueParse => "value parse",
        DiagnosticOperation::PolicyEvaluation => "policy evaluation",
        DiagnosticOperation::DeliveryProcess => "delivery process",
        DiagnosticOperation::OutboxDrain => "outbox drain",
        DiagnosticOperation::OutboxStateCommit => "outbox state commit",
    }
}

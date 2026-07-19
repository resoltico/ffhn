//! Signal-first rendering and durable failure evidence for maintainer commands.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Instant;

use clap::ValueEnum;
use serde::Serialize;

use crate::app::command::prepare_command;
use crate::model::{CommandSpec, DynResult};

const SCHEMA: &str = "ffhn.gate-event@1";
const FAILURE_TAIL_BYTES: usize = 16 * 1024;

/// Selects the maintained rendering format for a gate invocation.
#[derive(Debug, Clone, Copy, Default, ValueEnum, PartialEq, Eq)]
pub(crate) enum GateOutputFormat {
    /// Render concise terminal events for people.
    #[default]
    Human,
    /// Render one JSON event per line for automation.
    Json,
}

/// Selects how much successful child-process output reaches the terminal.
#[derive(Debug, Clone, Copy, Default, ValueEnum, PartialEq, Eq)]
pub(crate) enum GateVerbosity {
    /// Emit lifecycle events and actionable diagnostics only.
    #[default]
    Concise,
    /// Stream the raw stdout and stderr of each child command.
    Verbose,
}

/// Canonical rendering and evidence policy for a gate invocation.
#[derive(Debug, Clone, Default)]
pub(crate) struct GateOutputOptions {
    /// Selected rendering format.
    pub(crate) format: GateOutputFormat,
    /// Selected terminal verbosity.
    pub(crate) verbosity: GateVerbosity,
    /// Optional caller-owned directory for retained raw evidence.
    pub(crate) log_dir: Option<PathBuf>,
    /// Keeps raw logs after a successful run when explicitly requested.
    pub(crate) retain_passing_logs: bool,
}

/// Owns one gate run's rendered lifecycle and raw child-process evidence.
pub(crate) struct GateReporter {
    options: GateOutputOptions,
    gate_id: &'static str,
    log_dir: PathBuf,
    failed: bool,
}

#[derive(Serialize)]
struct GateEvent<'a> {
    schema: &'static str,
    gate: &'a str,
    step: &'a str,
    state: &'a str,
    elapsed_ms: Option<u128>,
    status: Option<i32>,
    message: Option<&'a str>,
    log_dir: Option<&'a Path>,
}

impl GateReporter {
    /// Creates one reporter with raw evidence outside the checkout by default.
    pub(crate) fn new(
        repo_root: &Path,
        gate_id: &'static str,
        options: GateOutputOptions,
    ) -> DynResult<Self> {
        let log_dir = options.log_dir.clone().unwrap_or_else(|| {
            repo_root
                .parent()
                .unwrap_or(repo_root)
                .join(".ffhn-artifacts")
                .join("gate-logs")
                .join(format!("{gate_id}-{}", std::process::id()))
        });
        fs::create_dir_all(&log_dir)?;
        let reporter = Self {
            options,
            gate_id,
            log_dir,
            failed: false,
        };
        reporter.emit("run", "started", None, None, None, None);
        Ok(reporter)
    }

    /// Runs one named command, capturing every raw byte and rendering only the selected signal.
    pub(crate) fn run_spec(&mut self, repo_root: &Path, spec: &CommandSpec) -> DynResult<()> {
        self.emit(&spec.step_id, "started", None, None, None, None);
        let started = Instant::now();
        let step_dir = self.log_dir.join(&spec.step_id);
        fs::create_dir_all(&step_dir)?;

        let mut command = Command::new(&spec.program);
        prepare_command(&mut command, repo_root, spec)?;
        command.stdin(Stdio::inherit());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let mut child = command.spawn()?;
        let stdout = child.stdout.take().ok_or("child stdout was not captured")?;
        let stderr = child.stderr.take().ok_or("child stderr was not captured")?;
        let verbose = self.options.format == GateOutputFormat::Human
            && self.options.verbosity == GateVerbosity::Verbose;
        let stdout_path = step_dir.join("stdout.log");
        let stderr_path = step_dir.join("stderr.log");
        let stdout_reader = spawn_capture(stdout, stdout_path.clone(), false, verbose);
        let stderr_reader = spawn_capture(stderr, stderr_path.clone(), true, verbose);
        let status = child.wait()?;
        join_capture(stdout_reader)?;
        join_capture(stderr_reader)?;
        let elapsed_ms = started.elapsed().as_millis();

        if status.success() {
            self.emit(
                &spec.step_id,
                "passed",
                Some(elapsed_ms),
                status.code(),
                None,
                None,
            );
            self.emit_warnings(&spec.step_id, &stdout_path, &stderr_path)?;
            return Ok(());
        }

        self.failed = true;
        let message = format!("command failed: {}", render_command(spec));
        self.emit(
            &spec.step_id,
            "failed",
            Some(elapsed_ms),
            status.code(),
            Some(&message),
            Some(&step_dir),
        );
        self.emit_failure_tail(&spec.step_id, &stdout_path, &stderr_path)?;
        Err(format!("gate step `{}` failed with status {status}", spec.step_id).into())
    }

    /// Runs one in-process policy step with the same lifecycle contract as an external command.
    pub(crate) fn run_operation<T>(
        &mut self,
        step: &str,
        operation: impl FnOnce() -> DynResult<T>,
    ) -> DynResult<T> {
        self.emit(step, "started", None, None, None, None);
        let started = Instant::now();
        match operation() {
            Ok(value) => {
                self.emit(
                    step,
                    "passed",
                    Some(started.elapsed().as_millis()),
                    None,
                    None,
                    None,
                );
                Ok(value)
            }
            Err(error) => {
                self.failed = true;
                let message = error.to_string();
                self.emit(
                    step,
                    "failed",
                    Some(started.elapsed().as_millis()),
                    None,
                    Some(&message),
                    Some(&self.log_dir),
                );
                Err(error)
            }
        }
    }

    /// Emits the terminal completion event and retains raw output only when it is useful evidence.
    pub(crate) fn finish(mut self, success: bool) -> DynResult<()> {
        self.failed |= !success;
        let retained = self.failed || self.options.retain_passing_logs;
        self.emit(
            "run",
            if success { "passed" } else { "failed" },
            None,
            None,
            None,
            retained.then_some(self.log_dir.as_path()),
        );
        if !retained {
            fs::remove_dir_all(&self.log_dir)?;
        }
        Ok(())
    }

    fn emit_warnings(&self, step: &str, stdout_path: &Path, stderr_path: &Path) -> DynResult<()> {
        for line in warning_lines(stdout_path)?
            .into_iter()
            .chain(warning_lines(stderr_path)?)
        {
            self.emit(step, "warning", None, None, Some(&line), None);
        }
        Ok(())
    }

    fn emit_failure_tail(
        &self,
        step: &str,
        stdout_path: &Path,
        stderr_path: &Path,
    ) -> DynResult<()> {
        let tail = format!(
            "stdout:\n{}\nstderr:\n{}",
            tail_text(stdout_path)?,
            tail_text(stderr_path)?
        );
        self.emit(step, "diagnostic", None, None, Some(&tail), None);
        Ok(())
    }

    fn emit(
        &self,
        step: &str,
        state: &str,
        elapsed_ms: Option<u128>,
        status: Option<i32>,
        message: Option<&str>,
        log_dir: Option<&Path>,
    ) {
        match self.options.format {
            GateOutputFormat::Human => {
                render_human_event(step, state, elapsed_ms, status, message, log_dir)
            }
            GateOutputFormat::Json => println!(
                "{}",
                serde_json::to_string(&GateEvent {
                    schema: SCHEMA,
                    gate: self.gate_id,
                    step,
                    state,
                    elapsed_ms,
                    status,
                    message,
                    log_dir,
                })
                .expect("gate events are serializable")
            ),
        }
    }
}

fn spawn_capture<R>(
    reader: R,
    path: PathBuf,
    stderr: bool,
    verbose: bool,
) -> thread::JoinHandle<io::Result<()>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = reader;
        let mut file = fs::File::create(path)?;
        let mut buffer = [0_u8; 8192];
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                return Ok(());
            }
            file.write_all(&buffer[..count])?;
            if verbose {
                if stderr {
                    io::stderr().write_all(&buffer[..count])?;
                } else {
                    io::stdout().write_all(&buffer[..count])?;
                }
            }
        }
    })
}

fn join_capture(handle: thread::JoinHandle<io::Result<()>>) -> DynResult<()> {
    handle
        .join()
        .map_err(|_| "gate output capture thread panicked")??;
    Ok(())
}

fn warning_lines(path: &Path) -> DynResult<Vec<String>> {
    let bytes = fs::read(path)?;
    let output = String::from_utf8_lossy(&bytes);
    Ok(output
        .lines()
        .filter(|line| line.contains("warning:") || line.contains("warning["))
        .map(str::to_owned)
        .collect())
}

fn tail_text(path: &Path) -> DynResult<String> {
    let bytes = fs::read(path)?;
    let start = bytes.len().saturating_sub(FAILURE_TAIL_BYTES);
    Ok(String::from_utf8_lossy(&bytes[start..]).into_owned())
}

fn render_command(spec: &CommandSpec) -> String {
    std::iter::once(spec.program.display().to_string())
        .chain(spec.args.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_human_event(
    step: &str,
    state: &str,
    elapsed_ms: Option<u128>,
    status: Option<i32>,
    message: Option<&str>,
    log_dir: Option<&Path>,
) {
    let elapsed = elapsed_ms
        .map(|value| format!(" ({value} ms)"))
        .unwrap_or_default();
    let status = status
        .map(|value| format!(", status {value}"))
        .unwrap_or_default();
    let message = message
        .map(|value| format!(": {value}"))
        .unwrap_or_default();
    let log_dir = log_dir
        .map(|value| format!("; evidence: {}", value.display()))
        .unwrap_or_default();
    println!("[{state}] {step}{elapsed}{status}{message}{log_dir}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warning_lines_select_only_compiler_style_warnings() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("output.log");
        fs::write(&path, "progress\nwarning: useful\nwarning[lint]\nERROR\n").expect("write");
        assert_eq!(
            warning_lines(&path).expect("warnings"),
            ["warning: useful", "warning[lint]"]
        );
    }

    #[test]
    fn tail_text_returns_the_full_small_file_and_the_end_of_a_large_file() {
        let directory = tempfile::tempdir().expect("tempdir");
        let small = directory.path().join("small.log");
        fs::write(&small, "small").expect("write small");
        assert_eq!(tail_text(&small).expect("tail"), "small");
        let large = directory.path().join("large.log");
        fs::write(&large, format!("{}end", "x".repeat(FAILURE_TAIL_BYTES))).expect("write large");
        assert_eq!(
            tail_text(&large).expect("tail"),
            format!("{}end", "x".repeat(FAILURE_TAIL_BYTES - 3))
        );
    }

    #[test]
    fn command_rendering_includes_the_program_and_arguments() {
        let spec = CommandSpec::new("cargo", ["fmt", "--check"], false).with_step_id("format");
        assert_eq!(render_command(&spec), "cargo fmt --check");
    }

    #[cfg(unix)]
    #[test]
    fn reporter_captures_success_warnings_and_discards_default_success_logs() {
        let directory = tempfile::tempdir().expect("tempdir");
        let log_dir = directory.path().join("logs");
        let mut reporter = GateReporter::new(
            directory.path(),
            "test",
            GateOutputOptions {
                log_dir: Some(log_dir.clone()),
                ..GateOutputOptions::default()
            },
        )
        .expect("reporter");
        let spec = CommandSpec::new(
            "sh",
            [
                "-c",
                "printf 'warning: retained diagnostic\\n'; printf 'stderr\\n' >&2",
            ],
            false,
        )
        .with_step_id("success");

        reporter.run_spec(directory.path(), &spec).expect("success");
        assert!(log_dir.join("success/stdout.log").is_file());
        reporter.finish(true).expect("finish");
        assert!(!log_dir.exists());
    }

    #[cfg(unix)]
    #[test]
    fn reporter_retains_failed_evidence_and_renders_json_or_verbose_modes() {
        let directory = tempfile::tempdir().expect("tempdir");
        let log_dir = directory.path().join("failure-logs");
        let mut reporter = GateReporter::new(
            directory.path(),
            "test",
            GateOutputOptions {
                format: GateOutputFormat::Json,
                verbosity: GateVerbosity::Verbose,
                log_dir: Some(log_dir.clone()),
                retain_passing_logs: true,
            },
        )
        .expect("reporter");
        let spec = CommandSpec::new("sh", ["-c", "printf 'failure\\n' >&2; exit 7"], false)
            .with_step_id("failure");

        let error = reporter
            .run_spec(directory.path(), &spec)
            .expect_err("failure should propagate");
        assert!(error.to_string().contains("gate step `failure` failed"));
        reporter.finish(false).expect("finish");
        assert_eq!(
            fs::read_to_string(log_dir.join("failure/stderr.log")).expect("stderr"),
            "failure\n"
        );
    }

    #[test]
    fn reporter_tracks_internal_operation_outcomes() {
        let directory = tempfile::tempdir().expect("tempdir");
        let log_dir = directory.path().join("operation-logs");
        let mut reporter = GateReporter::new(
            directory.path(),
            "test",
            GateOutputOptions {
                log_dir: Some(log_dir.clone()),
                retain_passing_logs: true,
                ..GateOutputOptions::default()
            },
        )
        .expect("reporter");
        assert_eq!(
            reporter
                .run_operation("internal-pass", || Ok::<_, Box<dyn std::error::Error>>(7))
                .expect("pass"),
            7
        );
        assert!(
            reporter
                .run_operation::<()>("internal-fail", || Err("boom".into()))
                .expect_err("failure")
                .to_string()
                .contains("boom")
        );
        reporter.finish(false).expect("finish");
        assert!(log_dir.is_dir());
    }

    #[test]
    fn reporter_default_logs_and_capture_helpers_cover_failure_paths() {
        let directory = tempfile::tempdir().expect("tempdir");
        let repo_root = directory.path().join("repo");
        fs::create_dir_all(&repo_root).expect("repo");
        let reporter = GateReporter::new(&repo_root, "default", GateOutputOptions::default())
            .expect("default reporter");
        let default_log_dir = directory
            .path()
            .join(".ffhn-artifacts/gate-logs")
            .join(format!("default-{}", std::process::id()));
        reporter.finish(true).expect("finish");
        assert!(!default_log_dir.exists());

        let missing = directory.path().join("missing.log");
        assert!(warning_lines(&missing).is_err());
        assert!(tail_text(&missing).is_err());
        let directory_target = directory.path().join("directory-target");
        fs::create_dir_all(&directory_target).expect("directory");
        let capture = spawn_capture(std::io::Cursor::new(b"raw"), directory_target, false, false);
        assert!(join_capture(capture).is_err());
        assert!(join_capture(thread::spawn(|| -> io::Result<()> { panic!("expected") })).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn reporter_verbose_human_mode_streams_both_child_channels() {
        let directory = tempfile::tempdir().expect("tempdir");
        let log_dir = directory.path().join("verbose-logs");
        let mut reporter = GateReporter::new(
            directory.path(),
            "test",
            GateOutputOptions {
                verbosity: GateVerbosity::Verbose,
                log_dir: Some(log_dir.clone()),
                retain_passing_logs: true,
                ..GateOutputOptions::default()
            },
        )
        .expect("reporter");
        let spec = CommandSpec::new("sh", ["-c", "printf out; printf err >&2"], false)
            .with_step_id("verbose");
        reporter.run_spec(directory.path(), &spec).expect("success");
        reporter.finish(true).expect("finish");
        assert_eq!(
            fs::read_to_string(log_dir.join("verbose/stdout.log")).expect("stdout"),
            "out"
        );
        assert_eq!(
            fs::read_to_string(log_dir.join("verbose/stderr.log")).expect("stderr"),
            "err"
        );
    }
}

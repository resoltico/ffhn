use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

struct FlushProbe(Arc<AtomicBool>);

impl Write for FlushProbe {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.store(true, Ordering::SeqCst);
        Ok(())
    }
}

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
    fs::write(&large, format!("{}end", "x".repeat(20_000))).expect("write large");
    let tail = tail_text(&large).expect("tail");
    assert_eq!(tail.len(), 16_384);
    assert!(tail.ends_with("end"));
}

#[test]
fn command_rendering_includes_the_program_and_arguments() {
    let spec = CommandSpec::new("cargo", ["fmt", "--check"], false).with_step_id("format");
    assert_eq!(render_command(&spec), "cargo fmt --check");
}

#[test]
fn success_output_streaming_requires_human_and_verbose_together() {
    assert!(!streams_success_output(
        GateOutputFormat::Human,
        GateVerbosity::Concise
    ));
    assert!(streams_success_output(
        GateOutputFormat::Human,
        GateVerbosity::Verbose
    ));
    assert!(!streams_success_output(
        GateOutputFormat::Json,
        GateVerbosity::Concise
    ));
    assert!(!streams_success_output(
        GateOutputFormat::Json,
        GateVerbosity::Verbose
    ));
}

#[test]
fn terminal_failure_and_event_rendering_preserve_every_output_fact() {
    assert!(!terminal_failure(false, true));
    assert!(terminal_failure(false, false));
    assert!(terminal_failure(true, true));
    assert!(terminal_failure(true, false));
    assert_eq!(
        render_human_event(
            "compile",
            "failed",
            Some(17),
            Some(7),
            Some("compiler failed"),
            Some(Path::new("evidence")),
        ),
        "[failed] compile (17 ms), status 7: compiler failed; evidence: evidence"
    );

    let directory = tempfile::tempdir().expect("tempdir");
    let reporter = GateReporter::new(
        directory.path(),
        "test-gate",
        GateOutputOptions {
            format: GateOutputFormat::Json,
            log_dir: Some(directory.path().join("logs")),
            ..GateOutputOptions::default()
        },
    )
    .expect("reporter");
    let rendered = reporter.emit(
        "compile",
        "failed",
        Some(17),
        Some(7),
        Some("compiler failed"),
        Some(Path::new("evidence")),
    );
    let event: serde_json::Value = serde_json::from_str(&rendered).expect("JSON event");
    assert_eq!(event["schema"], SCHEMA);
    assert_eq!(event["gate"], "test-gate");
    assert_eq!(event["step"], "compile");
    assert_eq!(event["state"], "failed");
    assert_eq!(event["elapsed_ms"], 17);
    assert_eq!(event["status"], 7);
    assert_eq!(event["message"], "compiler failed");
    assert_eq!(event["log_dir"], "evidence");
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

    let empty_capture_path = directory.path().join("empty-capture.log");
    join_capture(spawn_capture(
        std::io::Cursor::new(Vec::<u8>::new()),
        empty_capture_path.clone(),
        false,
        false,
    ))
    .expect("empty capture completes");
    assert_eq!(fs::read(empty_capture_path).expect("empty capture"), b"");

    let raw_capture_path = directory.path().join("raw-capture.log");
    join_capture(spawn_capture(
        std::io::Cursor::new(b"raw"),
        raw_capture_path.clone(),
        false,
        false,
    ))
    .expect("raw capture completes");
    assert_eq!(fs::read(raw_capture_path).expect("raw capture"), b"raw");

    let flush_observed = Arc::new(AtomicBool::new(false));
    let mut sink = CaptureSink {
        file: fs::File::create(directory.path().join("flush-probe.log"))
            .expect("create flush probe file"),
        live: Some(Box::new(FlushProbe(Arc::clone(&flush_observed)))),
    };
    sink.flush().expect("flush capture sink");
    assert!(flush_observed.load(Ordering::SeqCst));
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
    let spec =
        CommandSpec::new("sh", ["-c", "printf out; printf err >&2"], false).with_step_id("verbose");
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

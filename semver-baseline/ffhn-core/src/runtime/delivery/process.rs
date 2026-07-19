use std::io::{Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::{
    DeliveryProcessAttempt, DeliveryRoute, IoErrorClass, StderrCapture, StderrOutcome,
    TerminalOutcome, WriterOutcome,
};

/// Executes one process-stdin route and preserves every independently observed outcome.
pub(super) fn deliver(route: &DeliveryRoute, payload: &[u8]) -> DeliveryProcessAttempt {
    deliver_with_wait(route, payload, &mut Child::try_wait)
}

fn deliver_with_wait<F>(
    route: &DeliveryRoute,
    payload: &[u8],
    wait: &mut F,
) -> DeliveryProcessAttempt
where
    F: FnMut(&mut Child) -> std::io::Result<Option<ExitStatus>>,
{
    let (program, args, timeout_ms) = route.adapter().process_stdin();
    let started = Instant::now();
    let mut child = match Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return DeliveryProcessAttempt::new(
                TerminalOutcome::NotStarted {
                    io: IoErrorClass::from_error(&error),
                },
                WriterOutcome::NotAttempted,
                StderrOutcome::Absent,
            );
        }
    };
    deliver_started_with_wait(&mut child, payload, started, timeout_ms, wait)
}

/// Completes an already-spawned process delivery. Keeping this separate from spawning makes the
/// two standard-library pipe-availability facts independently executable and preserves them as
/// distinct durable evidence rather than treating either as an impossible panic.
fn deliver_started_with_wait<F>(
    child: &mut Child,
    payload: &[u8],
    started: Instant,
    timeout_ms: u64,
    wait: &mut F,
) -> DeliveryProcessAttempt
where
    F: FnMut(&mut Child) -> std::io::Result<Option<ExitStatus>>,
{
    let Some(stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return DeliveryProcessAttempt::new(
            TerminalOutcome::StdinUnavailable,
            WriterOutcome::NotAttempted,
            StderrOutcome::Absent,
        );
    };
    let stderr_reader = child
        .stderr
        .take()
        .map(|stderr| thread::spawn(move || capture_stderr(stderr)));
    let payload = payload.to_vec();
    let writer = thread::spawn(move || -> std::io::Result<()> {
        let mut stdin = stdin;
        stdin.write_all(&payload)?;
        stdin.write_all(b"\n")
    });
    let deadline = started + Duration::from_millis(timeout_ms);
    let terminal = loop {
        match wait(child) {
            Ok(Some(status)) => {
                break TerminalOutcome::Exited {
                    exit_code: status.code(),
                };
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break TerminalOutcome::TimedOut { timeout_ms };
            }
            Ok(None) => thread::sleep(Duration::from_millis(5)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break TerminalOutcome::WaitFailed {
                    io: IoErrorClass::from_error(&error),
                };
            }
        }
    };
    let stderr = match stderr_reader {
        Some(reader) => join_stderr(reader),
        // `stderr` is configured as piped above. Keep an impossible standard-library state typed
        // and visible without falsely claiming that a reader thread panicked.
        None => StderrOutcome::ReaderUnavailable,
    };
    DeliveryProcessAttempt::new(terminal, join_writer(writer), stderr)
}

fn capture_stderr(mut stderr: impl Read) -> StderrOutcome {
    let mut buffer = [0_u8; 1024];
    let mut capture = StderrCapture::accumulator();
    loop {
        match stderr.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                capture.record(&buffer[..read]);
            }
            Err(error) => {
                return StderrOutcome::read_failed(
                    IoErrorClass::from_error(&error),
                    capture.finish(),
                );
            }
        }
    }
    StderrOutcome::captured(capture.finish())
}

fn join_writer(writer: thread::JoinHandle<std::io::Result<()>>) -> WriterOutcome {
    match writer.join() {
        Ok(Ok(())) => WriterOutcome::Completed,
        Ok(Err(error)) => WriterOutcome::IoFailed {
            io: IoErrorClass::from_error(&error),
        },
        Err(_) => WriterOutcome::Panicked,
    }
}

fn join_stderr(reader: thread::JoinHandle<StderrOutcome>) -> StderrOutcome {
    match reader.join() {
        Ok(outcome) => outcome,
        Err(_) => StderrOutcome::ReaderPanicked,
    }
}

#[cfg(test)]
const DELIVERY_PROCESS_HELPER_TEST: &str =
    "runtime::delivery::process::tests::delivery_process_helper";

/// Builds the portable test-process command used by runtime integration tests.
#[cfg(test)]
pub(crate) fn test_process_command(
    mode: &str,
    output: Option<&std::path::Path>,
) -> (std::path::PathBuf, Vec<String>) {
    let mut arguments = vec![
        "--exact".to_owned(),
        DELIVERY_PROCESS_HELPER_TEST.to_owned(),
        "--nocapture".to_owned(),
        "--".to_owned(),
        mode.to_owned(),
    ];
    if let Some(output) = output {
        arguments.push(output.display().to_string());
    }
    (std::env::current_exe().expect("test executable"), arguments)
}

#[cfg(test)]
mod tests {
    use std::{env, fs, io, path::Path, process::Child};

    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    enum HelperAction {
        Write(String),
        Fail,
        ExitEarly,
    }

    fn parse_helper_action(arguments: &[String]) -> Result<HelperAction, String> {
        match arguments {
            [mode, output] if mode == "write" => Ok(HelperAction::Write(output.clone())),
            [mode] if mode == "fail" => Ok(HelperAction::Fail),
            [mode] if mode == "exit_early" => Ok(HelperAction::ExitEarly),
            _ => Err(format!(
                "invalid delivery-process helper invocation: {arguments:?}"
            )),
        }
    }

    fn helper_arguments(mode: &str, output: Option<&Path>) -> Vec<String> {
        test_process_command(mode, output).1
    }

    fn route(mode: &str, output: Option<&Path>, timeout_ms: u64) -> DeliveryRoute {
        let program = env::current_exe().expect("test executable");
        serde_json::from_value(serde_json::json!({
            "route_id": "delivery",
            "route_family": "on_run",
            "adapter": {
                "kind": "process_stdin",
                "program": program,
                "args": helper_arguments(mode, output),
                "timeout_ms": timeout_ms,
            }
        }))
        .expect("delivery route")
    }

    fn spawn_piped_helper(mode: &str) -> Child {
        let route = route(mode, None, 1_000);
        let (program, arguments, _) = route.adapter().process_stdin();
        Command::new(program)
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn helper")
    }

    fn helper_invocation() -> Option<Vec<String>> {
        let mut arguments = env::args();
        while let Some(argument) = arguments.next() {
            if argument == "--" {
                let arguments = arguments.collect::<Vec<_>>();
                return (!arguments.is_empty()).then_some(arguments);
            }
        }
        None
    }

    #[test]
    fn delivery_process_helper() {
        let Some(arguments) = helper_invocation() else {
            return;
        };
        let action = parse_helper_action(&arguments).unwrap_or_else(|message| panic!("{message}"));
        match action {
            HelperAction::Write(output) => {
                let mut payload = Vec::new();
                io::stdin().read_to_end(&mut payload).expect("read payload");
                use std::io::Write as _;

                fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(output)
                    .expect("open payload log")
                    .write_all(&payload)
                    .expect("append payload");
            }
            HelperAction::Fail => {
                io::stderr().write_all(b"bad").expect("write stderr");
                std::process::exit(7);
            }
            HelperAction::ExitEarly => {}
        }
    }

    struct PartialThenBrokenReader {
        reads: u8,
    }

    impl Read for PartialThenBrokenReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.reads += 1;
            if self.reads == 1 {
                buffer[..3].copy_from_slice(b"bad");
                Ok(3)
            } else {
                Err(io::Error::other("read broke"))
            }
        }
    }

    #[test]
    fn stderr_capture_preserves_empty_and_partial_failure_facts_without_prose_markers() {
        let empty = serde_json::to_value(capture_stderr(&b""[..])).expect("stderr JSON");
        assert_eq!(empty["kind"], "captured");
        assert_eq!(empty["retained_bytes_base64"], "");
        assert_eq!(empty["original_len_bytes"], "0");
        assert_eq!(empty["truncated"], false);

        let partial = serde_json::to_value(capture_stderr(PartialThenBrokenReader { reads: 0 }))
            .expect("stderr JSON");
        assert_eq!(partial["kind"], "read_failed");
        assert_eq!(partial["partial"]["retained_bytes_base64"], "YmFk");
        assert_eq!(partial["partial"]["original_len_bytes"], "3");

        let bounded = serde_json::to_value(capture_stderr(std::io::Cursor::new(vec![b'x'; 2_049])))
            .expect("stderr JSON");
        assert_eq!(bounded["kind"], "captured");
        assert_eq!(
            bounded["retained_bytes_base64"].as_str().map(str::len),
            Some(2_732)
        );
        assert_eq!(bounded["truncated"], true);

        let utf8_source = [vec![b'x'; 2_047], "é".repeat(50).into_bytes()].concat();
        let boundary = serde_json::to_value(capture_stderr(std::io::Cursor::new(utf8_source)))
            .expect("stderr JSON");
        assert_eq!(boundary["kind"], "captured");
        assert_eq!(boundary["original_len_bytes"], "2147");
        assert_eq!(boundary["truncated"], true);
    }

    #[test]
    fn helper_action_parser_is_closed_and_rejects_non_protocol_arguments() {
        assert_eq!(
            parse_helper_action(&["write".to_owned(), "output".to_owned()]),
            Ok(HelperAction::Write("output".to_owned()))
        );
        assert!(parse_helper_action(&["read".to_owned(), "output".to_owned()]).is_err());
        assert_eq!(
            parse_helper_action(&["fail".to_owned()]),
            Ok(HelperAction::Fail)
        );
        assert_eq!(
            parse_helper_action(&["exit_early".to_owned()]),
            Ok(HelperAction::ExitEarly)
        );
        assert!(parse_helper_action(&["write".to_owned()]).is_err());
    }

    #[test]
    fn process_attempt_is_total_and_retains_terminal_writer_and_stderr_facts() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let output = directory.path().join("payload.json");
        let succeeded = deliver(&route("write", Some(&output), 1_000), br#"{"payload":1}"#);
        assert!(succeeded.is_success());
        assert!(matches!(
            succeeded.terminal(),
            TerminalOutcome::Exited { exit_code: Some(0) }
        ));
        assert!(matches!(succeeded.writer(), WriterOutcome::Completed));
        assert!(matches!(succeeded.stderr(), StderrOutcome::Captured { .. }));
        assert_eq!(
            fs::read(&output).expect("written payload"),
            b"{\"payload\":1}\n"
        );

        let failed = deliver(&route("fail", None, 1_000), b"payload");
        assert_eq!(
            failed.primary(),
            Some(crate::DeliveryFailurePrimary::UnsuccessfulExit)
        );
        assert!(matches!(
            failed.terminal(),
            TerminalOutcome::Exited { exit_code: Some(7) }
        ));
        assert!(matches!(failed.writer(), WriterOutcome::Completed));
        let stderr = serde_json::to_value(failed.stderr()).expect("stderr JSON");
        assert_eq!(stderr["kind"], "captured");
        assert_eq!(stderr["retained_bytes_base64"], "YmFk");

        fn pending_wait(_: &mut Child) -> io::Result<Option<ExitStatus>> {
            Ok(None)
        }
        let mut pending_wait = pending_wait;
        let timed_out = deliver_with_wait(
            &route("write", Some(&output), 1),
            b"payload",
            &mut pending_wait,
        );
        assert_eq!(
            timed_out.primary(),
            Some(crate::DeliveryFailurePrimary::TimedOut)
        );
        assert!(matches!(timed_out.stderr(), StderrOutcome::Captured { .. }));
    }

    #[test]
    fn wait_failure_remains_typed_after_workers_are_joined() {
        fn wait(_: &mut Child) -> io::Result<Option<ExitStatus>> {
            Err(io::Error::other("wait broke"))
        }
        let directory = tempfile::tempdir().expect("temporary directory");
        let output = directory.path().join("payload.json");
        let mut wait = wait;
        let attempt =
            deliver_with_wait(&route("write", Some(&output), 1_000), b"payload", &mut wait);
        assert_eq!(
            attempt.primary(),
            Some(crate::DeliveryFailurePrimary::WaitFailed)
        );
        assert!(matches!(
            attempt.writer(),
            WriterOutcome::Completed | WriterOutcome::IoFailed { .. }
        ));
        assert!(matches!(attempt.stderr(), StderrOutcome::Captured { .. }));
    }

    #[test]
    fn delivery_preserves_every_process_boundary_failure_without_panicking() {
        let unavailable_program: DeliveryRoute = serde_json::from_value(serde_json::json!({
            "route_id": "delivery",
            "route_family": "on_run",
            "adapter": {
                "kind": "process_stdin",
                "program": "/definitely/not/an/ffhn-delivery-program",
                "args": [],
                "timeout_ms": 1_000,
            }
        }))
        .expect("route shape");
        assert!(matches!(
            deliver(&unavailable_program, b"payload").terminal(),
            TerminalOutcome::NotStarted { .. }
        ));

        let mut missing_stdin = spawn_piped_helper("exit_early");
        let _ = missing_stdin.stdin.take();
        let mut wait = Child::try_wait;
        let attempt = deliver_started_with_wait(
            &mut missing_stdin,
            b"payload",
            Instant::now(),
            1_000,
            &mut wait,
        );
        assert!(matches!(
            attempt.terminal(),
            TerminalOutcome::StdinUnavailable
        ));

        let mut missing_stderr = spawn_piped_helper("exit_early");
        let _ = missing_stderr.stderr.take();
        let mut wait = Child::try_wait;
        let attempt = deliver_started_with_wait(
            &mut missing_stderr,
            b"payload",
            Instant::now(),
            1_000,
            &mut wait,
        );
        assert!(matches!(attempt.stderr(), StderrOutcome::ReaderUnavailable));

        assert!(matches!(
            join_writer(thread::spawn(|| Err(io::Error::other("writer stopped")))),
            WriterOutcome::IoFailed { .. }
        ));
        assert!(matches!(
            join_writer(thread::spawn(|| -> io::Result<()> {
                panic!("writer panic")
            })),
            WriterOutcome::Panicked
        ));
        assert!(matches!(
            join_stderr(thread::spawn(|| -> StderrOutcome {
                panic!("reader panic")
            })),
            StderrOutcome::ReaderPanicked
        ));
    }
}

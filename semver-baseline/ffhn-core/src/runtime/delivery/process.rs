use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::DeliveryRoute;

const STDERR_LIMIT: usize = 4_096;

pub(super) fn deliver(route: &DeliveryRoute, payload: &[u8]) -> Result<(), String> {
    deliver_with_wait(route, payload, &mut Child::try_wait)
}

fn deliver_with_wait<F>(route: &DeliveryRoute, payload: &[u8], wait: &mut F) -> Result<(), String>
where
    F: FnMut(&mut Child) -> std::io::Result<Option<ExitStatus>>,
{
    let (program, args, timeout_ms) = route.adapter().process_stdin();
    let started = Instant::now();
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start delivery process: {error}"))?;
    let stdin = take_stdin(&mut child)?;
    let stderr = child.stderr.take();
    let payload = payload.to_vec();
    let writer = thread::spawn(move || -> std::io::Result<()> {
        let mut stdin = stdin;
        stdin.write_all(&payload)?;
        stdin.write_all(b"\n")
    });
    let stderr_reader = stderr.map(|stderr| thread::spawn(move || capture_stderr(stderr)));
    let deadline = started + Duration::from_millis(timeout_ms);

    loop {
        match wait(&mut child) {
            Ok(Some(status)) => {
                let writer_result = writer
                    .join()
                    .map_err(|_| "delivery payload writer panicked".to_owned())?;
                let stderr = join_stderr(stderr_reader);
                writer_result.map_err(|error| {
                    delivery_error("could not write payload", error, stderr.clone())
                })?;
                if status.success() {
                    return Ok(());
                }
                return Err(delivery_error(
                    &format!("delivery process exited with status {status}"),
                    "",
                    stderr,
                ));
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = writer.join();
                return Err(delivery_error(
                    "delivery process timed out",
                    "",
                    join_stderr(stderr_reader),
                ));
            }
            Ok(None) => thread::sleep(Duration::from_millis(5)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = writer.join();
                return Err(delivery_error(
                    "could not wait for delivery process",
                    error,
                    join_stderr(stderr_reader),
                ));
            }
        }
    }
}

fn take_stdin(child: &mut Child) -> Result<ChildStdin, String> {
    let Some(stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("delivery process stdin was unavailable".to_owned());
    };
    Ok(stdin)
}

fn capture_stderr(mut stderr: impl Read) -> String {
    let mut captured = Vec::new();
    let mut buffer = [0_u8; 1024];
    let mut truncated = false;
    loop {
        match stderr.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                let remaining = STDERR_LIMIT.saturating_sub(captured.len());
                let copied = remaining.min(read);
                captured.extend_from_slice(&buffer[..copied]);
                truncated |= copied < read;
            }
            Err(error) => return format!("stderr read failed: {error}"),
        }
    }
    let mut text = String::from_utf8_lossy(&captured).trim().to_owned();
    if truncated {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str("[truncated]");
    }
    text
}

fn join_stderr(reader: Option<thread::JoinHandle<String>>) -> Option<String> {
    reader
        .and_then(|reader| reader.join().ok())
        .filter(|text| !text.is_empty())
}

fn delivery_error(context: &str, error: impl std::fmt::Display, stderr: Option<String>) -> String {
    let mut message = context.to_owned();
    let error = error.to_string();
    if !error.is_empty() {
        message.push_str(": ");
        message.push_str(&error);
    }
    if let Some(stderr) = stderr {
        message.push_str("; stderr: ");
        message.push_str(&stderr);
    }
    message
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
    use std::{
        env, fs, io,
        path::Path,
        process::{Child, ExitStatus},
    };

    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    enum HelperAction {
        Write(String),
        Fail,
        ExitEarly,
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

    fn helper_command(mode: &str, output: Option<&Path>) -> Command {
        let program = env::current_exe().expect("test executable");
        let mut command = Command::new(program);
        command.args(helper_arguments(mode, output));
        command
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

    fn helper_action(arguments: &[String]) -> Result<HelperAction, ()> {
        match arguments {
            [mode, output] if mode == "write" => Ok(HelperAction::Write(output.clone())),
            [mode] if mode == "fail" => Ok(HelperAction::Fail),
            [mode] if mode == "exit_early" => Ok(HelperAction::ExitEarly),
            _ => Err(()),
        }
    }

    #[test]
    fn delivery_process_helper() {
        let Some(arguments) = helper_invocation() else {
            return;
        };
        match helper_action(&arguments).unwrap_or_else(|()| {
            panic!("invalid delivery-process helper invocation: {arguments:?}")
        }) {
            HelperAction::Write(output) => {
                let mut payload = Vec::new();
                io::stdin()
                    .read_to_end(&mut payload)
                    .expect("read delivery payload");
                fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(output)
                    .expect("open delivery payload")
                    .write_all(&payload)
                    .expect("write delivery payload");
            }
            HelperAction::Fail => {
                io::stderr().write_all(b"bad").expect("write stderr");
                panic!("delivery-process helper failure");
            }
            HelperAction::ExitEarly => {}
        }
    }

    #[test]
    fn helper_action_recognizes_each_cross_platform_process_mode() {
        assert_eq!(
            helper_action(&["write".to_owned(), "output".to_owned()]),
            Ok(HelperAction::Write("output".to_owned()))
        );
        assert_eq!(helper_action(&["fail".to_owned()]), Ok(HelperAction::Fail));
        assert_eq!(
            helper_action(&["exit_early".to_owned()]),
            Ok(HelperAction::ExitEarly)
        );
        assert!(helper_action(&["write".to_owned()]).is_err());
        assert!(helper_action(&["invalid".to_owned(), "output".to_owned()]).is_err());
    }

    struct BrokenReader;

    impl Read for BrokenReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("read broke"))
        }
    }

    #[test]
    fn stderr_capture_and_error_assembly_are_bounded_and_diagnostic() {
        assert_eq!(capture_stderr(&b"  useful stderr\n"[..]), "useful stderr");
        assert_eq!(
            capture_stderr(BrokenReader),
            "stderr read failed: read broke"
        );
        let bounded = capture_stderr(std::io::Cursor::new(vec![b'x'; STDERR_LIMIT + 1]));
        assert!(bounded.starts_with(&"x".repeat(STDERR_LIMIT)));
        assert!(bounded.ends_with(" [truncated]"));
        assert_eq!(
            capture_stderr(std::io::Cursor::new(vec![b' '; STDERR_LIMIT + 1])),
            "[truncated]"
        );
        assert_eq!(join_stderr(None), None);
        assert_eq!(
            join_stderr(Some(thread::spawn(|| " worker stderr ".to_owned()))),
            Some(" worker stderr ".to_owned())
        );
        assert_eq!(
            delivery_error("failed", "io error", Some("stderr".to_owned())),
            "failed: io error; stderr: stderr"
        );
        assert_eq!(delivery_error("failed", "", None), "failed");
    }

    #[test]
    fn process_stdin_delivery_writes_exact_payload_and_reports_exit_and_timeout() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let output = directory.path().join("payload.json");
        deliver(&route("write", Some(&output), 1_000), br#"{"payload":1}"#)
            .expect("delivery succeeds");
        assert_eq!(
            fs::read(&output).expect("written payload"),
            b"{\"payload\":1}\n"
        );

        let failed =
            deliver(&route("fail", None, 1_000), b"payload").expect_err("nonzero status fails");
        assert!(failed.contains("exited with status"));
        assert!(failed.contains("stderr: bad"));

        fn pending_wait(_: &mut Child) -> io::Result<Option<ExitStatus>> {
            Ok(None)
        }

        let mut pending_wait = pending_wait;
        let timed_out = deliver_with_wait(
            &route("write", Some(&output), 100),
            b"payload",
            &mut pending_wait,
        )
        .expect_err("timeout");
        assert!(timed_out.contains("timed out"));

        let write_failed = deliver(&route("exit_early", None, 5_000), &vec![b'x'; 1_000_000])
            .expect_err("closed stdin rejects the payload");
        assert!(
            write_failed.contains("could not write payload"),
            "{write_failed}"
        );

        let program = env::current_exe().expect("test executable");
        let invalid = Command::new(program)
            .args(helper_arguments("invalid", Some(Path::new("unused"))))
            .status()
            .expect("invalid helper process");
        assert!(!invalid.success());
    }

    #[test]
    fn wait_errors_kill_and_reap_the_child_with_a_delivery_error() {
        fn wait(_: &mut Child) -> io::Result<Option<ExitStatus>> {
            Err(io::Error::other("wait broke"))
        }

        let directory = tempfile::tempdir().expect("temporary directory");
        let output = directory.path().join("payload.json");
        let mut wait = wait;
        let error = deliver_with_wait(&route("write", Some(&output), 1_000), b"payload", &mut wait)
            .expect_err("wait errors must fail delivery");
        assert!(error.contains("could not wait for delivery process"));
        assert!(error.contains("wait broke"));
    }

    #[test]
    fn missing_piped_stdin_kills_and_reaps_the_child() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let output = directory.path().join("payload.json");
        let mut child = helper_command("write", Some(&output))
            .stdin(Stdio::piped())
            .spawn()
            .expect("child");
        let _stdin = child.stdin.take().expect("piped stdin");
        assert_eq!(
            take_stdin(&mut child).expect_err("stdin was already taken"),
            "delivery process stdin was unavailable"
        );
        assert!(child.try_wait().expect("reaped child").is_some());
    }
}

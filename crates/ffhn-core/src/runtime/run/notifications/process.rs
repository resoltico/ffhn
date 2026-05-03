use std::io::{self, Read};
use std::process::{ChildStderr, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};

use crate::time::elapsed_ms;
use crate::{NotificationDeliveryOutcome, RunNotificationDelivery};

const NOTIFICATION_STDERR_CAPTURE_LIMIT: usize = 16 * 1024;

pub(crate) trait NotificationProcess {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>;
    fn kill(&mut self) -> io::Result<()>;
    fn wait(&mut self) -> io::Result<ExitStatus>;
}

impl NotificationProcess for std::process::Child {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        std::process::Child::try_wait(self)
    }

    fn kill(&mut self) -> io::Result<()> {
        std::process::Child::kill(self)
    }

    fn wait(&mut self) -> io::Result<ExitStatus> {
        std::process::Child::wait(self)
    }
}

pub(crate) fn wait_for_notification_process<P: NotificationProcess>(
    process: &mut P,
    hook_name: &str,
    started: Instant,
    timeout_ms: u64,
) -> RunNotificationDelivery {
    let deadline = started + Duration::from_millis(timeout_ms);
    loop {
        match process.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    RunNotificationDelivery::delivered(
                        hook_name,
                        elapsed_ms(&started),
                        status.code().unwrap_or(0),
                    )
                } else {
                    RunNotificationDelivery::failed(
                        hook_name,
                        elapsed_ms(&started),
                        status.code(),
                        format!("hook exited with status {status}"),
                    )
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = process.kill();
                    let _ = process.wait();
                    return notification_failure(hook_name, started, true, None, "hook timed out");
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                return notification_failure(hook_name, started, false, None, error);
            }
        }
    }
}

pub(crate) fn notification_failure(
    hook_name: &str,
    started: Instant,
    timed_out: bool,
    exit_code: Option<i32>,
    error: impl ToString,
) -> RunNotificationDelivery {
    if timed_out {
        RunNotificationDelivery::timed_out(hook_name, elapsed_ms(&started), error.to_string())
    } else {
        RunNotificationDelivery::failed(
            hook_name,
            elapsed_ms(&started),
            exit_code,
            error.to_string(),
        )
    }
}

pub(crate) fn abort_child_process(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub(crate) fn spawn_notification_stderr_capture(
    child: &mut std::process::Child,
) -> Option<thread::JoinHandle<io::Result<String>>> {
    child
        .stderr
        .take()
        .map(|stderr| thread::spawn(move || capture_notification_stderr(stderr)))
}

fn capture_notification_stderr(mut stderr: ChildStderr) -> io::Result<String> {
    capture_notification_stderr_from_reader(&mut stderr)
}

fn capture_notification_stderr_from_reader(mut stderr: impl Read) -> io::Result<String> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    let mut truncated = false;
    loop {
        let read = stderr.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = NOTIFICATION_STDERR_CAPTURE_LIMIT.saturating_sub(bytes.len());
        if remaining > 0 {
            let to_copy = remaining.min(read);
            bytes.extend_from_slice(&buffer[..to_copy]);
            if to_copy < read {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }

    let mut stderr_text = String::from_utf8_lossy(&bytes).trim().to_owned();
    if truncated {
        if !stderr_text.is_empty() {
            stderr_text.push(' ');
        }
        stderr_text.push_str("[truncated]");
    }
    Ok(stderr_text)
}

pub(crate) fn join_notification_stderr_capture(
    stderr_capture: Option<thread::JoinHandle<io::Result<String>>>,
) -> Option<String> {
    let stderr_capture = stderr_capture?;
    match stderr_capture.join() {
        Ok(Ok(stderr_text)) if !stderr_text.is_empty() => Some(stderr_text),
        _ => None,
    }
}

pub(crate) fn append_notification_stderr(
    delivery: &mut RunNotificationDelivery,
    stderr_text: Option<String>,
) {
    let Some(stderr_text) = stderr_text else {
        return;
    };
    let error = match &mut delivery.outcome {
        NotificationDeliveryOutcome::Delivered { .. } => return,
        NotificationDeliveryOutcome::TimedOut { error }
        | NotificationDeliveryOutcome::Failed { error, .. } => error,
    };
    if error.is_empty() {
        error.push_str("stderr: ");
    } else {
        error.push_str("; stderr: ");
    }
    error.push_str(&stderr_text);
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;

    struct ChunkedReader {
        chunks: VecDeque<Vec<u8>>,
    }

    impl Read for ChunkedReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let Some(chunk) = self.chunks.pop_front() else {
                return Ok(0);
            };
            let read = chunk.len().min(buffer.len());
            buffer[..read].copy_from_slice(&chunk[..read]);
            if read < chunk.len() {
                self.chunks.push_front(chunk[read..].to_vec());
            }
            Ok(read)
        }
    }

    #[test]
    fn append_notification_stderr_preserves_success_and_populates_missing_errors() {
        let mut delivered = RunNotificationDelivery::delivered("notify", 1, 0);
        append_notification_stderr(&mut delivered, Some("ignored".to_owned()));
        assert!(delivered.error().is_none());

        let mut untouched = RunNotificationDelivery::timed_out("notify", 1, "hook timed out");
        append_notification_stderr(&mut untouched, None);
        assert_eq!(untouched.error(), Some("hook timed out"));

        let mut timed_out = RunNotificationDelivery::timed_out("notify", 1, "hook timed out");
        append_notification_stderr(&mut timed_out, Some("hook stderr".to_owned()));
        assert_eq!(
            timed_out.error(),
            Some("hook timed out; stderr: hook stderr")
        );

        let mut failed = RunNotificationDelivery::failed("notify", 1, Some(7), "");
        append_notification_stderr(&mut failed, Some("hook stderr".to_owned()));
        assert_eq!(failed.error(), Some("stderr: hook stderr"));
    }

    #[test]
    fn stderr_capture_truncates_large_payloads() {
        let stderr_text = capture_notification_stderr_from_reader(ChunkedReader {
            chunks: VecDeque::from([
                vec![b'x'; NOTIFICATION_STDERR_CAPTURE_LIMIT - 2],
                vec![b'y'; 10],
            ]),
        })
        .expect("capture stderr");
        assert!(stderr_text.ends_with("[truncated]"));
        assert!(stderr_text.len() <= NOTIFICATION_STDERR_CAPTURE_LIMIT + "[truncated]".len() + 1);

        let stderr_text = capture_notification_stderr_from_reader(ChunkedReader {
            chunks: VecDeque::from([
                vec![b'x'; NOTIFICATION_STDERR_CAPTURE_LIMIT],
                vec![b'y'; 10],
            ]),
        })
        .expect("capture stderr after limit");
        assert!(stderr_text.ends_with("[truncated]"));
    }

    #[test]
    fn stderr_capture_and_join_helpers_cover_empty_and_existing_error_paths() {
        let stderr_text = capture_notification_stderr_from_reader(ChunkedReader {
            chunks: VecDeque::from([vec![b' '; NOTIFICATION_STDERR_CAPTURE_LIMIT], vec![b' '; 8]]),
        })
        .expect("capture whitespace stderr after limit");
        assert_eq!(stderr_text, "[truncated]");

        let empty_capture = thread::spawn(|| Ok::<_, io::Error>(String::new()));
        assert!(join_notification_stderr_capture(Some(empty_capture)).is_none());

        let panic_capture = thread::spawn(|| -> io::Result<String> { panic!("stderr panic") });
        assert!(join_notification_stderr_capture(Some(panic_capture)).is_none());

        let mut failed =
            RunNotificationDelivery::failed("notify", 1, Some(7), "hook exited with status 7");
        append_notification_stderr(&mut failed, Some("hook stderr".to_owned()));
        assert_eq!(
            failed.error(),
            Some("hook exited with status 7; stderr: hook stderr")
        );
    }
}

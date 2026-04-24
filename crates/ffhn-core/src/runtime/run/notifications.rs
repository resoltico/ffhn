use std::io::{self, Read, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::stable_json::stable_json;
use crate::{
    NOTIFICATION_PAYLOAD_SCHEMA_NAME, NOTIFICATION_PAYLOAD_SCHEMA_VERSION, NotificationEvent,
    NotificationHook, NotificationPayload, RunNotificationDelivery, RunOutcome, RunReport,
    TargetDocument,
};

use super::super::storage::now_utc;

const NOTIFICATION_STDERR_CAPTURE_LIMIT: usize = 16 * 1024;

pub(super) fn dispatch_notifications(
    target: &TargetDocument,
    report: &RunReport,
) -> Vec<RunNotificationDelivery> {
    let event = notification_event(report.run_outcome);
    let hooks = target
        .notifications
        .iter()
        .filter(|hook| hook.on.contains(&event))
        .collect::<Vec<_>>();

    hooks
        .into_iter()
        .map(|hook| deliver_notification(hook, event, target, report))
        .collect()
}

pub(super) fn notification_event(run_outcome: RunOutcome) -> NotificationEvent {
    match run_outcome {
        RunOutcome::Initialized => NotificationEvent::Initialized,
        RunOutcome::Changed => NotificationEvent::Changed,
        RunOutcome::Unchanged => NotificationEvent::Unchanged,
        RunOutcome::FailedTransient => NotificationEvent::FailedTransient,
        RunOutcome::FailedPermanent => NotificationEvent::FailedPermanent,
        RunOutcome::SkippedDisabled => NotificationEvent::SkippedDisabled,
    }
}

pub(super) trait NotificationProcess {
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

pub(super) fn write_notification_payload_or_failure<W: Write>(
    hook_name: &str,
    event: NotificationEvent,
    started: Instant,
    stdin: Option<W>,
    payload: &str,
    mut abort: impl FnMut(),
) -> Option<RunNotificationDelivery> {
    let mut stdin = stdin?;
    if let Err(error) = stdin.write_all(payload.as_bytes()) {
        abort();
        return Some(notification_failure(
            hook_name, event, started, false, None, error,
        ));
    }
    None
}

pub(super) fn wait_for_notification_process<P: NotificationProcess>(
    process: &mut P,
    hook_name: &str,
    event: NotificationEvent,
    started: Instant,
    timeout_ms: u64,
) -> RunNotificationDelivery {
    let deadline = started + Duration::from_millis(timeout_ms);
    loop {
        match process.try_wait() {
            Ok(Some(status)) => {
                return RunNotificationDelivery {
                    hook_name: hook_name.to_owned(),
                    event,
                    delivered: status.success(),
                    timed_out: false,
                    exit_code: status.code(),
                    duration_ms: started.elapsed().as_millis() as u64,
                    error: if status.success() {
                        None
                    } else {
                        Some(format!("hook exited with status {status}"))
                    },
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = process.kill();
                    let _ = process.wait();
                    return notification_failure(
                        hook_name,
                        event,
                        started,
                        true,
                        None,
                        "hook timed out",
                    );
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                return notification_failure(hook_name, event, started, false, None, error);
            }
        }
    }
}

pub(super) fn deliver_notification(
    hook: &NotificationHook,
    event: NotificationEvent,
    target: &TargetDocument,
    report: &RunReport,
) -> RunNotificationDelivery {
    let started = Instant::now();
    let payload = match serialize_notification_payload(hook, event, report) {
        Ok(payload) => payload,
        Err(error) => {
            return notification_failure(
                &hook.name,
                event,
                started,
                false,
                None,
                format!("failed to serialize notification payload: {error}"),
            );
        }
    };
    let run_outcome = report.run_outcome.as_str();
    let reason_code = report.reason_code.as_str();
    let run_mode = report.run_mode.as_str();
    let notification_event = event.as_str();
    let failure_class = report
        .failure_class
        .map(crate::FailureClass::as_str)
        .unwrap_or_default();
    let mut child = match Command::new(&hook.shell)
        .arg("-c")
        .arg(&hook.command)
        .env("FFHN_TARGET_ID", target.target_id.as_str())
        .env("FFHN_RUN_OUTCOME", run_outcome)
        .env("FFHN_REASON_CODE", reason_code)
        .env("FFHN_RUN_MODE", run_mode)
        .env("FFHN_FAILURE_CLASS", failure_class)
        .env("FFHN_NOTIFICATION_EVENT", notification_event)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return notification_failure(&hook.name, event, started, false, None, error),
    };

    let stderr_capture = spawn_notification_stderr_capture(&mut child);
    match write_child_notification_payload_or_failure(
        &hook.name, event, started, &mut child, &payload,
    ) {
        Some(mut delivery) => {
            append_notification_stderr(
                &mut delivery,
                join_notification_stderr_capture(stderr_capture),
            );
            delivery
        }
        None => {
            let mut delivery = wait_for_notification_process(
                &mut child,
                &hook.name,
                event,
                started,
                hook.timeout_ms,
            );
            append_notification_stderr(
                &mut delivery,
                join_notification_stderr_capture(stderr_capture),
            );
            delivery
        }
    }
}

fn notification_failure(
    hook_name: &str,
    event: NotificationEvent,
    started: Instant,
    timed_out: bool,
    exit_code: Option<i32>,
    error: impl ToString,
) -> RunNotificationDelivery {
    RunNotificationDelivery {
        hook_name: hook_name.to_owned(),
        event,
        delivered: false,
        timed_out,
        exit_code,
        duration_ms: started.elapsed().as_millis() as u64,
        error: Some(error.to_string()),
    }
}

fn abort_child_process(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn spawn_notification_stderr_capture(
    child: &mut std::process::Child,
) -> Option<thread::JoinHandle<io::Result<String>>> {
    child
        .stderr
        .take()
        .map(|mut stderr| thread::spawn(move || capture_notification_stderr(&mut stderr)))
}

fn capture_notification_stderr(mut stderr: impl Read) -> io::Result<String> {
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

fn join_notification_stderr_capture(
    stderr_capture: Option<thread::JoinHandle<io::Result<String>>>,
) -> Option<String> {
    let stderr_capture = stderr_capture?;
    match stderr_capture.join() {
        Ok(Ok(stderr_text)) if !stderr_text.is_empty() => Some(stderr_text),
        _ => None,
    }
}

fn append_notification_stderr(delivery: &mut RunNotificationDelivery, stderr_text: Option<String>) {
    let Some(stderr_text) = stderr_text else {
        return;
    };
    if delivery.delivered {
        return;
    }
    match delivery.error.as_mut() {
        Some(error) => {
            error.push_str("; stderr: ");
            error.push_str(&stderr_text);
        }
        None => delivery.error = Some(format!("stderr: {stderr_text}")),
    }
}

pub(super) fn write_child_notification_payload_or_failure(
    hook_name: &str,
    event: NotificationEvent,
    started: Instant,
    child: &mut std::process::Child,
    payload: &str,
) -> Option<RunNotificationDelivery> {
    write_notification_payload_or_failure(
        hook_name,
        event,
        started,
        child.stdin.take(),
        payload,
        || abort_child_process(child),
    )
}

fn serialize_notification_payload(
    hook: &NotificationHook,
    event: NotificationEvent,
    report: &RunReport,
) -> Result<String, crate::CoreError> {
    let payload = NotificationPayload {
        schema_name: NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: hook.name.clone(),
        event,
        delivery_started_at: now_utc()?,
        run_report: report.clone(),
        extensions: None,
    };
    payload.validate()?;
    stable_json(&payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

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
        let mut delivered = RunNotificationDelivery {
            hook_name: "notify".to_owned(),
            event: NotificationEvent::Changed,
            delivered: true,
            timed_out: false,
            exit_code: Some(0),
            duration_ms: 1,
            error: None,
        };
        append_notification_stderr(&mut delivered, Some("ignored".to_owned()));
        assert!(delivered.error.is_none());

        let mut failed = RunNotificationDelivery {
            hook_name: "notify".to_owned(),
            event: NotificationEvent::Changed,
            delivered: false,
            timed_out: false,
            exit_code: Some(7),
            duration_ms: 1,
            error: None,
        };
        append_notification_stderr(&mut failed, Some("hook stderr".to_owned()));
        assert_eq!(failed.error.as_deref(), Some("stderr: hook stderr"));
    }

    #[test]
    fn stderr_capture_truncates_large_payloads() {
        let stderr_text = capture_notification_stderr(ChunkedReader {
            chunks: VecDeque::from([
                vec![b'x'; NOTIFICATION_STDERR_CAPTURE_LIMIT - 2],
                vec![b'y'; 10],
            ]),
        })
        .expect("capture stderr");
        assert!(stderr_text.ends_with("[truncated]"));
        assert!(stderr_text.len() <= NOTIFICATION_STDERR_CAPTURE_LIMIT + "[truncated]".len() + 1);

        let stderr_text = capture_notification_stderr(ChunkedReader {
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
        let stderr_text = capture_notification_stderr(ChunkedReader {
            chunks: VecDeque::from([vec![b' '; NOTIFICATION_STDERR_CAPTURE_LIMIT], vec![b' '; 8]]),
        })
        .expect("capture whitespace stderr after limit");
        assert_eq!(stderr_text, "[truncated]");

        let empty_capture = thread::spawn(|| Ok::<_, io::Error>(String::new()));
        assert!(join_notification_stderr_capture(Some(empty_capture)).is_none());

        let panic_capture = thread::spawn(|| -> io::Result<String> { panic!("stderr panic") });
        assert!(join_notification_stderr_capture(Some(panic_capture)).is_none());

        let mut failed = RunNotificationDelivery {
            hook_name: "notify".to_owned(),
            event: NotificationEvent::Changed,
            delivered: false,
            timed_out: false,
            exit_code: Some(7),
            duration_ms: 1,
            error: Some("hook exited with status 7".to_owned()),
        };
        append_notification_stderr(&mut failed, Some("hook stderr".to_owned()));
        assert_eq!(
            failed.error.as_deref(),
            Some("hook exited with status 7; stderr: hook stderr")
        );
    }
}

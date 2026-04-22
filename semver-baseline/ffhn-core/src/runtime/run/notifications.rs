use std::io::{self, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::stable_json::stable_json;
use crate::{
    NotificationEvent, NotificationHook, RunNotificationDelivery, RunOutcome, RunReport,
    TargetDocument,
};

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
    let payload = match stable_json(report) {
        Ok(payload) => payload,
        Err(error) => {
            return hooks
                .into_iter()
                .map(|hook| {
                    notification_failure(
                        &hook.name,
                        event,
                        Instant::now(),
                        false,
                        None,
                        format!("failed to serialize notification payload: {error}"),
                    )
                })
                .collect();
        }
    };

    hooks
        .into_iter()
        .map(|hook| deliver_notification(hook, event, target, report, &payload))
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
    payload: &str,
) -> RunNotificationDelivery {
    let started = Instant::now();
    let Some(run_outcome) = serde_variant_name(report.run_outcome) else {
        return notification_failure(
            &hook.name,
            event,
            started,
            false,
            None,
            "failed to serialize FFHN_RUN_OUTCOME",
        );
    };
    let Some(reason_code) = serde_variant_name(report.reason_code) else {
        return notification_failure(
            &hook.name,
            event,
            started,
            false,
            None,
            "failed to serialize FFHN_REASON_CODE",
        );
    };
    let Some(run_mode) = serde_variant_name(report.run_mode) else {
        return notification_failure(
            &hook.name,
            event,
            started,
            false,
            None,
            "failed to serialize FFHN_RUN_MODE",
        );
    };
    let Some(notification_event) = serde_variant_name(event) else {
        return notification_failure(
            &hook.name,
            event,
            started,
            false,
            None,
            "failed to serialize FFHN_NOTIFICATION_EVENT",
        );
    };
    let failure_class = match report.failure_class {
        Some(failure_class) => match serde_variant_name(failure_class) {
            Some(failure_class) => failure_class,
            None => {
                return notification_failure(
                    &hook.name,
                    event,
                    started,
                    false,
                    None,
                    "failed to serialize FFHN_FAILURE_CLASS",
                );
            }
        },
        None => String::new(),
    };
    let mut child = match Command::new(&hook.shell)
        .arg("-c")
        .arg(&hook.command)
        .env("FFHN_TARGET_ID", &target.target_id)
        .env("FFHN_RUN_OUTCOME", run_outcome)
        .env("FFHN_REASON_CODE", reason_code)
        .env("FFHN_RUN_MODE", run_mode)
        .env("FFHN_FAILURE_CLASS", failure_class)
        .env("FFHN_NOTIFICATION_EVENT", notification_event)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return notification_failure(&hook.name, event, started, false, None, error),
    };

    write_child_notification_payload_or_failure(&hook.name, event, started, &mut child, payload)
        .unwrap_or_else(|| {
            wait_for_notification_process(&mut child, &hook.name, event, started, hook.timeout_ms)
        })
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

fn serde_variant_name<T: serde::Serialize>(value: T) -> Option<String> {
    serde_json::to_value(value)
        .ok()?
        .as_str()
        .map(ToOwned::to_owned)
}

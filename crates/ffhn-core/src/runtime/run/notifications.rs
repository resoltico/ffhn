use std::process::{Command, Stdio};
use std::time::Instant;

use crate::{NotificationHook, RunNotificationDelivery, RunReport, TargetDocument};

mod payload;
mod process;

#[cfg(all(test, unix))]
pub(super) use payload::write_child_notification_payload_or_failure;
#[cfg(test)]
pub(super) use payload::write_notification_payload_or_failure;
#[cfg(test)]
pub(super) use process::{NotificationProcess, wait_for_notification_process};

pub(super) fn dispatch_notifications(
    target: &TargetDocument,
    report: &RunReport,
) -> Vec<RunNotificationDelivery> {
    let hooks = target
        .notifications
        .iter()
        .filter(|hook| hook.on.contains(&report.run_outcome))
        .collect::<Vec<_>>();

    hooks
        .into_iter()
        .map(|hook| deliver_notification(hook, target, report))
        .collect()
}

pub(super) fn deliver_notification(
    hook: &NotificationHook,
    target: &TargetDocument,
    report: &RunReport,
) -> RunNotificationDelivery {
    let started = Instant::now();
    let payload = match payload::serialize_notification_payload(hook, report) {
        Ok(payload) => payload,
        Err(error) => {
            return process::notification_failure(
                &hook.name,
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
    let failure_class = report
        .failure_class
        .map(crate::FailureClass::as_str)
        .unwrap_or_default();
    let mut child = match Command::new(&hook.program)
        .args(&hook.args)
        .env("FFHN_TARGET_ID", target.target_id.as_str())
        .env("FFHN_RUN_OUTCOME", run_outcome)
        .env("FFHN_REASON_CODE", reason_code)
        .env("FFHN_RUN_MODE", run_mode)
        .env("FFHN_FAILURE_CLASS", failure_class)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return process::notification_failure(&hook.name, started, false, None, error);
        }
    };

    let stderr_capture = process::spawn_notification_stderr_capture(&mut child);
    match payload::write_child_notification_payload_or_failure(
        &hook.name, started, &mut child, &payload,
    ) {
        Some(mut delivery) => {
            process::append_notification_stderr(
                &mut delivery,
                process::join_notification_stderr_capture(stderr_capture),
            );
            delivery
        }
        None => {
            let mut delivery = process::wait_for_notification_process(
                &mut child,
                &hook.name,
                started,
                hook.timeout_ms,
            );
            process::append_notification_stderr(
                &mut delivery,
                process::join_notification_stderr_capture(stderr_capture),
            );
            delivery
        }
    }
}

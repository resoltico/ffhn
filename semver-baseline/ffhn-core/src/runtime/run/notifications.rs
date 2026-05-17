use std::process::{Command, Stdio};
use std::time::Instant;

use crate::{NotificationRoute, RunNotificationDelivery, RunReport, TargetDocument};

mod payload;
mod process;

#[cfg(all(test, unix))]
pub(super) use payload::write_child_notification_payload_or_failure;
#[cfg(test)]
pub(super) use payload::write_notification_payload_or_failure;
#[cfg(test)]
pub(super) use process::{
    NotificationProcess, notification_time_remaining, recv_with_notification_deadline,
    wait_for_notification_process,
};

pub(super) fn dispatch_notifications(
    target: &TargetDocument,
    report: &RunReport,
) -> Vec<RunNotificationDelivery> {
    let hooks = target
        .notification_routes
        .iter()
        .filter(|route| route.on.contains(&report.run_outcome()))
        .collect::<Vec<_>>();

    hooks
        .into_iter()
        .map(|route| deliver_notification(route, target, report))
        .collect()
}

pub(super) fn deliver_notification(
    route: &NotificationRoute,
    target: &TargetDocument,
    report: &RunReport,
) -> RunNotificationDelivery {
    let endpoint = target
        .notification_endpoints
        .iter()
        .find(|endpoint| endpoint.name() == route.endpoint())
        .expect("validated notification route endpoint");
    let started = Instant::now();
    let payload = match payload::serialize_notification_payload(route, report) {
        Ok(payload) => payload,
        Err(error) => {
            return process::notification_failure(
                route.name(),
                started,
                false,
                None,
                format!("failed to serialize notification payload: {error}"),
            );
        }
    };
    let run_outcome = report.run_outcome().as_str();
    let failure_cause = report.failure_cause().map(crate::RunFailureCause::as_str);
    let run_mode = report.run_mode().as_str();
    let failure_class = report
        .failure_class()
        .map(crate::FailureClass::as_str)
        .unwrap_or_default();
    let adapter = endpoint.adapter();
    let mut child = match Command::new(adapter.program())
        .args(adapter.args())
        .env("FFHN_TARGET_ID", target.target_id.as_str())
        .env("FFHN_RUN_OUTCOME", run_outcome)
        .env("FFHN_FAILURE_CAUSE", failure_cause.unwrap_or_default())
        .env("FFHN_RUN_MODE", run_mode)
        .env("FFHN_FAILURE_CLASS", failure_class)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return process::notification_failure(route.name(), started, false, None, error);
        }
    };

    let stderr_capture = process::spawn_notification_stderr_capture(&mut child);
    let timeout_ms = adapter.timeout_ms();
    match payload::write_child_notification_payload_or_failure(
        route.name(),
        started,
        timeout_ms,
        &mut child,
        &payload,
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
                route.name(),
                started,
                timeout_ms,
            );
            process::append_notification_stderr(
                &mut delivery,
                process::join_notification_stderr_capture(stderr_capture),
            );
            delivery
        }
    }
}

use std::io::Write;
use std::process::Child;
use std::time::Instant;

use crate::stable_json::stable_json;
use crate::{
    NOTIFICATION_PAYLOAD_SCHEMA_NAME, NOTIFICATION_PAYLOAD_SCHEMA_VERSION, NotificationHook,
    NotificationPayload, RunNotificationDelivery, RunReport,
};

use crate::runtime::storage::now_utc;

pub(crate) fn write_notification_payload_or_failure<W: Write>(
    hook_name: &str,
    started: Instant,
    stdin: Option<W>,
    payload: &str,
    mut abort: impl FnMut(),
) -> Option<RunNotificationDelivery> {
    let mut stdin = stdin?;
    if let Err(error) = stdin.write_all(payload.as_bytes()) {
        abort();
        return Some(super::process::notification_failure(
            hook_name, started, false, None, error,
        ));
    }
    if let Err(error) = stdin.write_all(b"\n") {
        abort();
        return Some(super::process::notification_failure(
            hook_name, started, false, None, error,
        ));
    }
    None
}

pub(crate) fn write_child_notification_payload_or_failure(
    hook_name: &str,
    started: Instant,
    child: &mut Child,
    payload: &str,
) -> Option<RunNotificationDelivery> {
    write_notification_payload_or_failure(hook_name, started, child.stdin.take(), payload, || {
        super::process::abort_child_process(child)
    })
}

pub(crate) fn serialize_notification_payload(
    hook: &NotificationHook,
    report: &RunReport,
) -> Result<String, crate::CoreError> {
    let payload = NotificationPayload {
        schema_name: NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        hook_name: hook.name.clone(),
        delivery_started_at: now_utc()?,
        run_report: report.clone(),
        extensions: None,
    };
    payload.validate()?;
    stable_json(&payload)
}

use std::io::{self, Write};
use std::process::Child;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use crate::stable_json::stable_json;
use crate::{
    NOTIFICATION_PAYLOAD_SCHEMA_NAME, NOTIFICATION_PAYLOAD_SCHEMA_VERSION, NotificationPayload,
    NotificationRoute, RunNotificationDelivery, RunReport,
};

use crate::runtime::storage::now_utc;

#[cfg(test)]
pub(crate) fn write_notification_payload_or_failure<W: Write>(
    route_name: &str,
    started: Instant,
    stdin: Option<W>,
    payload: &str,
    mut abort: impl FnMut(),
) -> Option<RunNotificationDelivery> {
    let mut stdin = stdin?;
    if let Err(error) = stdin.write_all(payload.as_bytes()) {
        abort();
        return Some(super::process::notification_failure(
            route_name, started, false, None, error,
        ));
    }
    if let Err(error) = stdin.write_all(b"\n") {
        abort();
        return Some(super::process::notification_failure(
            route_name, started, false, None, error,
        ));
    }
    None
}

fn write_notification_payload<W: Write>(mut stdin: W, payload: &str) -> io::Result<()> {
    stdin.write_all(payload.as_bytes())?;
    stdin.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn write_child_notification_payload_or_failure(
    route_name: &str,
    started: Instant,
    timeout_ms: u64,
    child: &mut Child,
    payload: &str,
) -> Option<RunNotificationDelivery> {
    let Some(stdin) = child.stdin.take() else {
        super::process::abort_child_process(child);
        return Some(super::process::notification_failure(
            route_name,
            started,
            false,
            None,
            "notification child stdin was unavailable",
        ));
    };

    let (result_tx, result_rx) = mpsc::channel();
    let payload = payload.to_owned();
    let writer = thread::spawn(move || {
        let result = write_notification_payload(stdin, &payload);
        let _ = result_tx.send(result);
    });

    match super::process::recv_with_notification_deadline(
        &result_rx, route_name, started, timeout_ms,
    ) {
        Ok(Ok(())) => {
            let _ = writer.join();
            None
        }
        Ok(Err(error)) => {
            super::process::abort_child_process(child);
            let _ = writer.join();
            Some(super::process::notification_failure(
                route_name, started, false, None, error,
            ))
        }
        Err(delivery) => {
            super::process::abort_child_process(child);
            let _ = writer.join();
            Some(delivery)
        }
    }
}

pub(crate) fn serialize_notification_payload(
    route: &NotificationRoute,
    report: &RunReport,
) -> Result<String, crate::CoreError> {
    let payload = NotificationPayload {
        schema_name: NOTIFICATION_PAYLOAD_SCHEMA_NAME.to_owned(),
        schema_version: NOTIFICATION_PAYLOAD_SCHEMA_VERSION,
        route_name: route.name().to_owned(),
        delivery_started_at: now_utc()?,
        run_report: report.clone(),
        extensions: None,
    };
    payload.validate()?;
    stable_json(&payload)
}

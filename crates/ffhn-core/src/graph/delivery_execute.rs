//! Snapshot-only process and HTTPS webhook delivery attempts.

#[cfg(not(any(unix, windows)))]
compile_error!("FFHN process delivery requires Unix process groups or Windows Job Objects");

use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
use process_wrap::std::JobObject;
#[cfg(unix)]
use process_wrap::std::ProcessGroup;
use process_wrap::std::{ChildWrapper, CommandWrap};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use ureq::tls::{RootCerts, TlsConfig};

use crate::CoreError;

use super::{DeadLetter, DeliveryAttemptFailure, DeliveryRecord, GraphDeliveryAdapter};

/// Result of one delivery attempt, ready for a delivery-result generation commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeliveryExecution {
    /// The receiver acknowledged success and the pending record must be removed.
    Delivered,
    /// A nonterminal failure appended one attempt and retained the updated pending record.
    Retry(DeliveryRecord),
    /// A terminal failure appended one final attempt and produced a dead letter.
    DeadLetter(DeadLetter),
}

/// Executes one due pending record strictly from its immutable adapter and policy snapshot.
pub fn execute_delivery_attempt(
    record: DeliveryRecord,
    attempted_at_utc: String,
) -> Result<DeliveryExecution, CoreError> {
    execute_delivery_attempt_with_clock(record, attempted_at_utc, || {
        OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(CoreError::from)
    })
}

fn execute_delivery_attempt_with_clock(
    mut record: DeliveryRecord,
    attempted_at_utc: String,
    retry_state_clock: impl FnOnce() -> Result<String, CoreError>,
) -> Result<DeliveryExecution, CoreError> {
    record.validate()?;
    let failure = match record.adapter() {
        GraphDeliveryAdapter::ProcessStdin {
            program,
            args,
            timeout_ms,
        } => deliver_process(record.envelope(), program, args, *timeout_ms),
        GraphDeliveryAdapter::HttpWebhook {
            url,
            timeout_ms,
            header_secrets,
        } => deliver_webhook(record.envelope(), url, *timeout_ms, header_secrets),
    };
    let Err(failure) = failure else {
        return Ok(DeliveryExecution::Delivered);
    };
    let attempt = u32::try_from(record.attempts().len())
        .map_err(|_| CoreError::contract("delivery attempt count overflowed"))?
        .checked_add(1)
        .ok_or_else(|| CoreError::contract("delivery attempt count overflowed"))?;
    let retry_state_commit_time_utc = retry_state_clock()?;
    let retry = super::retry::deterministic_retry_at(
        record.event_id(),
        record.route_id(),
        attempt,
        record.delivery_policy(),
        &retry_state_commit_time_utc,
    )?;
    record.record_failure(attempted_at_utc, failure, retry)?;
    if attempt == record.delivery_policy().max_attempts() {
        Ok(DeliveryExecution::DeadLetter(record.into_dead_letter()?))
    } else {
        Ok(DeliveryExecution::Retry(record))
    }
}

fn deliver_process(
    envelope: &super::EventEnvelope,
    program: &str,
    args: &[String],
    timeout_ms: u64,
) -> Result<(), DeliveryAttemptFailure> {
    let payload = process_payload(envelope)?;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::null());
    let child =
        spawn_managed_process(command).map_err(|error| DeliveryAttemptFailure::Process {
            message: message(error),
        })?;
    deliver_spawned_process(child, payload, timeout_ms)
}

fn deliver_spawned_process(
    child: Box<dyn ChildWrapper>,
    payload: String,
    timeout_ms: u64,
) -> Result<(), DeliveryAttemptFailure> {
    deliver_spawned_process_with_poll(child, payload, timeout_ms, |child| child.try_wait())
}

fn deliver_spawned_process_with_poll(
    mut child: Box<dyn ChildWrapper>,
    payload: String,
    timeout_ms: u64,
    mut poll: impl FnMut(&mut dyn ChildWrapper) -> std::io::Result<Option<std::process::ExitStatus>>,
) -> Result<(), DeliveryAttemptFailure> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let stderr = child.stderr().take().map(read_stderr);
    let Some(mut stdin) = child.stdin().take() else {
        terminate_process_group(child.as_mut(), deadline);
        return Err(DeliveryAttemptFailure::Process {
            message: "process delivery did not provide stdin".to_owned(),
        });
    };
    let (writer_sender, writer_results) = mpsc::sync_channel(1);
    let writer = thread::spawn(move || {
        let result = stdin
            .write_all(payload.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .map_err(|error| DeliveryAttemptFailure::Process {
                message: message(error),
            });
        let _ = writer_sender.send(result);
    });
    drop(writer);
    let status = loop {
        match poll(child.as_mut()) {
            Ok(Some(status)) => {
                terminate_process_group(child.as_mut(), deadline);
                break status;
            }
            Ok(None) if before_deadline(Instant::now(), deadline) => {
                thread::sleep(Duration::from_millis(5));
            }
            Ok(None) => {
                terminate_process_group(child.as_mut(), deadline);
                return Err(DeliveryAttemptFailure::Process {
                    message: "process delivery timed out".to_owned(),
                });
            }
            Err(error) => {
                terminate_process_group(child.as_mut(), deadline);
                return Err(DeliveryAttemptFailure::Process {
                    message: message(error),
                });
            }
        }
    };
    if status.success() {
        await_writer_result(writer_results, deadline)?;
        Ok(())
    } else {
        let suffix = await_completed_stderr(stderr, deadline)
            .filter(|value| !value.is_empty())
            .map(|value| format!(": {value}"))
            .unwrap_or_default();
        Err(DeliveryAttemptFailure::Process {
            message: bounded_message(format!("process exited unsuccessfully{suffix}")),
        })
    }
}

fn before_deadline(now: Instant, deadline: Instant) -> bool {
    now < deadline
}

/// Starts complete process-tree termination and waits only within the delivery deadline.
///
/// A closed adapter parent can leave a child or inherited pipe open. Cleanup is best effort, but
/// it must never make a configured attempt deadline unenforceable.
fn terminate_process_group(child: &mut dyn ChildWrapper, deadline: Instant) {
    let _ = child.start_kill();
    while before_deadline(Instant::now(), deadline) {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => break,
            Ok(None) => thread::sleep(Duration::from_millis(5)),
        }
    }
}

fn spawn_managed_process(command: Command) -> std::io::Result<Box<dyn ChildWrapper>> {
    let mut command = CommandWrap::from(command);
    #[cfg(unix)]
    command.wrap(ProcessGroup::leader());
    #[cfg(windows)]
    command.wrap(JobObject);
    command.spawn()
}

fn process_payload(value: &impl serde::Serialize) -> Result<String, DeliveryAttemptFailure> {
    crate::stable_json::stable_json(value).map_err(|error| DeliveryAttemptFailure::Process {
        message: message(error),
    })
}

fn await_writer_result(
    receiver: mpsc::Receiver<Result<(), DeliveryAttemptFailure>>,
    deadline: Instant,
) -> Result<(), DeliveryAttemptFailure> {
    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(DeliveryAttemptFailure::Process {
            message: "process stdin writer did not finish before delivery deadline".to_owned(),
        }),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(DeliveryAttemptFailure::Process {
            message: "process stdin writer failed".to_owned(),
        }),
    }
}

fn deliver_webhook(
    envelope: &super::EventEnvelope,
    url: &url::Url,
    timeout_ms: u64,
    header_secrets: &std::collections::BTreeMap<String, super::DeliveryHeaderSecret>,
) -> Result<(), DeliveryAttemptFailure> {
    deliver_webhook_with_secret_lookup(envelope, url, timeout_ms, header_secrets, &|env| {
        std::env::var(env).ok()
    })
}

fn deliver_webhook_with_secret_lookup(
    envelope: &super::EventEnvelope,
    url: &url::Url,
    timeout_ms: u64,
    header_secrets: &std::collections::BTreeMap<String, super::DeliveryHeaderSecret>,
    secret_lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<(), DeliveryAttemptFailure> {
    let mut request = ureq::Agent::config_builder()
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .timeout_global(Some(Duration::from_millis(timeout_ms)))
        .max_redirects(0)
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .build()
        .new_agent()
        .post(url.as_str())
        .header("Content-Type", "application/json");
    for (name, secret) in header_secrets {
        let value = resolve_delivery_secret(secret, secret_lookup)?;
        request = request.header(name, value);
    }
    let payload = webhook_payload(envelope)?;
    send_and_finish_webhook(payload, |payload| request.send(payload))
}

fn send_and_finish_webhook(
    payload: String,
    sender: impl FnOnce(String) -> Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<(), DeliveryAttemptFailure> {
    let mut response = sender(payload).map_err(|error| DeliveryAttemptFailure::HttpWebhook {
        message: message(error),
        status: None,
    })?;
    let status = response.status().as_u16();
    finish_webhook_response(status, &mut response.body_mut().as_reader())
}

fn finish_webhook_response(
    status: u16,
    reader: &mut impl Read,
) -> Result<(), DeliveryAttemptFailure> {
    discard_bounded_response_body(reader);
    classify_webhook_status(status)
}

fn webhook_payload(value: &impl serde::Serialize) -> Result<String, DeliveryAttemptFailure> {
    crate::stable_json::stable_json(value).map_err(|error| DeliveryAttemptFailure::HttpWebhook {
        message: message(error),
        status: None,
    })
}

fn read_stderr(mut stderr: std::process::ChildStderr) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(read_bounded_stderr(&mut stderr));
    });
    receiver
}

/// Returns bounded stderr before the existing attempt deadline; diagnostics never extend delivery.
fn await_completed_stderr(
    receiver: Option<mpsc::Receiver<String>>,
    deadline: Instant,
) -> Option<String> {
    receiver.and_then(|receiver| {
        match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(stderr) => Some(stderr),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => Some("stderr reader failed".to_owned()),
        }
    })
}

fn read_bounded_stderr(stderr: &mut impl Read) -> String {
    let mut retained = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        match stderr.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => {
                let remaining = 2_048_usize.saturating_sub(retained.len());
                retained.extend_from_slice(&chunk[..count.min(remaining)]);
            }
            Err(_) => break,
        }
    }
    bounded_message(String::from_utf8_lossy(&retained).replace(['\n', '\r'], " "))
}

fn resolve_delivery_secret(
    secret: &super::DeliveryHeaderSecret,
    lookup: impl FnOnce(&str) -> Option<String>,
) -> Result<String, DeliveryAttemptFailure> {
    let unavailable = || DeliveryAttemptFailure::SecretUnavailable {
        env: secret.env.clone(),
    };
    let value = lookup(&secret.env).ok_or_else(&unavailable)?;
    let value = secret.format.replace("{value}", &value);
    ureq::http::HeaderValue::from_bytes(value.as_bytes()).map_err(|_| unavailable())?;
    Ok(value)
}

fn classify_webhook_status(status: u16) -> Result<(), DeliveryAttemptFailure> {
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(DeliveryAttemptFailure::HttpWebhook {
            message: "webhook returned a non-success status".to_owned(),
            status: Some(status),
        })
    }
}

fn discard_bounded_response_body(reader: &mut impl Read) {
    let mut discarded = Vec::new();
    let _ = reader.take(65_536).read_to_end(&mut discarded);
}

fn message(error: impl std::fmt::Display) -> String {
    bounded_message(error.to_string())
}

fn bounded_message(value: impl Into<String>) -> String {
    let mut text = value.into().replace(['\n', '\r'], " ");
    let end = (0..=text.len().min(2_048))
        .rev()
        .find(|end| text.is_char_boundary(*end))
        .expect("zero is always a UTF-8 character boundary");
    text.truncate(end);
    text
}

#[cfg(test)]
#[path = "delivery_execute/tests.rs"]
mod tests;

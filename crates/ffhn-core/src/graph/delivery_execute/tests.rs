use super::*;
use crate::{
    ConditionId,
    graph::{
        DeliveryPolicy, EventEnvelope, EventEnvelopeParts, EventKey, EventObservation, GraphId,
        GraphRoute, MeasurementId, MeasurementInstanceId, OutboxAdmission, SourceId,
        SourceInstanceId,
    },
};

fn record_with(
    program: &str,
    args: &[String],
    canonical_value: &str,
    timeout_ms: u64,
) -> DeliveryRecord {
    let policy: DeliveryPolicy = toml::from_str(
        "max_pending = 1\nmax_attempts = 2\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n",
    ).expect("policy");
    let args = args
        .iter()
        .map(|argument| format!("{argument:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let route: GraphRoute = toml::from_str(&format!(
        "route_id = \"critical\"\nroute_family = \"on_condition\"\n[adapter]\nkind = \"process_stdin\"\nprogram = {program:?}\nargs = [{args}]\ntimeout_ms = {timeout_ms}\n",
    )).expect("route");
    let envelope = envelope(canonical_value);
    OutboxAdmission::admit(
        &[],
        [envelope],
        &[route],
        Some(&policy),
        "2026-08-25T00:00:00Z",
    )
    .expect("admission")
    .records()[0]
        .clone()
}

fn envelope(canonical_value: &str) -> EventEnvelope {
    let graph_id = GraphId::mint();
    let source_instance_id = SourceInstanceId::mint();
    EventEnvelope::new(EventEnvelopeParts {
        graph_id: graph_id.clone(),
        source_instance_id: source_instance_id.clone(),
        event_key: EventKey::ConditionSatisfied {
            graph_id,
            source_id: SourceId::new("shop").expect("source"),
            source_instance_id,
            measurement_id: MeasurementId::new("price").expect("measurement"),
            measurement_instance_id: MeasurementInstanceId::mint(),
            condition_id: ConditionId::new("changed").expect("condition"),
            condition_defn_digest: "a".repeat(64),
            observation_seq: 1,
        },
        display_name: "Price".to_owned(),
        committed_at_utc: "2026-08-25T00:00:00Z".to_owned(),
        observation: Some(
            EventObservation::new(canonical_value.to_owned(), 1).expect("observation"),
        ),
        lifecycle_fact: None,
        policy_revision: None,
    })
    .expect("envelope")
}

fn webhook_record(secret: bool) -> DeliveryRecord {
    let policy: DeliveryPolicy = toml::from_str(
        "max_pending = 1\nmax_attempts = 2\nbase_backoff_ms = 10\nmax_backoff_ms = 100\njitter_ratio = \"0\"\n",
    )
    .expect("policy");
    let secret = if secret {
        "[adapter.header_secrets]\nAuthorization = { env = \"FFHN_TEST_MISSING_WEBHOOK_SECRET_F74A\", format = \"Bearer {value}\" }\n"
    } else {
        ""
    };
    let route: GraphRoute = toml::from_str(&format!(
        "route_id = \"webhook\"\nroute_family = \"on_condition\"\n[adapter]\nkind = \"http_webhook\"\nurl = \"https://127.0.0.1:9/hook\"\ntimeout_ms = 100\n{secret}"
    ))
    .expect("webhook route");
    OutboxAdmission::admit(
        &[],
        [envelope("1")],
        &[route],
        Some(&policy),
        "2026-08-25T00:00:00Z",
    )
    .expect("admission")
    .records()[0]
        .clone()
}

#[test]
fn process_delivery_uses_the_snapshot_and_turns_a_nonzero_exit_into_a_retry_record() {
    let successful = crate::graph::test_support::successful_process();
    assert_eq!(
        execute_delivery_attempt(
            record_with(&successful.program, &successful.args, "1", 1_000),
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("success"),
        DeliveryExecution::Delivered,
    );
    let failing = crate::graph::test_support::failing_process();
    let DeliveryExecution::Retry(record) = execute_delivery_attempt_with_clock(
        record_with(&failing.program, &failing.args, "1", 1_000),
        "2026-08-25T00:00:00Z".to_owned(),
        || Ok("2026-08-25T00:01:00Z".to_owned()),
    )
    .expect("retry") else {
        panic!("false must retry");
    };
    assert_eq!(record.attempts().len(), 1);
    assert_eq!(record.next_retry_at_utc(), "2026-08-25T00:01:00.01Z");

    let DeliveryExecution::DeadLetter(letter) =
        execute_delivery_attempt_with_clock(record, "2026-08-25T00:02:00Z".to_owned(), || {
            Ok("2026-08-25T00:03:00Z".to_owned())
        })
        .expect("terminal attempt")
    else {
        panic!("second failure must dead-letter");
    };
    assert_eq!(letter.record().attempts().len(), 2);
}

#[test]
fn process_and_webhook_attempts_preserve_spawn_stderr_secret_transport_and_clock_failures() {
    let execution = execute_delivery_attempt_with_clock(
        record_with(
            &crate::graph::test_support::missing_process_path(),
            &[],
            "1",
            1_000,
        ),
        "2026-08-25T00:00:00Z".to_owned(),
        || Ok("2026-08-25T00:01:00Z".to_owned()),
    )
    .expect("spawn retry");
    assert!(matches!(execution, DeliveryExecution::Retry(_)));

    #[cfg(unix)]
    {
        let execution = execute_delivery_attempt_with_clock(
            record_with(
                "/bin/sh",
                &[
                    "-c".to_owned(),
                    "exec 0<&-; printf 'first\\nsecond' >&2; exit 1".to_owned(),
                ],
                &"x".repeat(1_000_000),
                1_000,
            ),
            "2026-08-25T00:00:00Z".to_owned(),
            || Ok("2026-08-25T00:01:00Z".to_owned()),
        )
        .expect("stderr retry");
        let DeliveryExecution::Retry(record) = execution else {
            panic!("process failure");
        };
        let wire = serde_json::to_value(&record).expect("record wire");
        let message = wire["attempts"][0]["failure"]["message"]
            .as_str()
            .expect("message");
        assert!(message.contains("first second"));
    }

    let DeliveryExecution::Retry(secret) = execute_delivery_attempt_with_clock(
        webhook_record(true),
        "2026-08-25T00:00:00Z".to_owned(),
        || Ok("2026-08-25T00:01:00Z".to_owned()),
    )
    .expect("secret retry") else {
        panic!("missing secret retry");
    };
    assert!(matches!(
        serde_json::to_value(secret).expect("wire")["attempts"][0]["failure"]["kind"].as_str(),
        Some("secret_unavailable")
    ));
    assert!(matches!(
        execute_delivery_attempt_with_clock(
            webhook_record(false),
            "2026-08-25T00:00:00Z".to_owned(),
            || Ok("2026-08-25T00:01:00Z".to_owned()),
        )
        .expect("transport retry"),
        DeliveryExecution::Retry(_)
    ));
    let webhook = webhook_record(true);
    let GraphDeliveryAdapter::HttpWebhook {
        url,
        timeout_ms,
        header_secrets,
    } = webhook.adapter()
    else {
        panic!("webhook adapter");
    };
    assert!(matches!(
        deliver_webhook_with_secret_lookup(
            webhook.envelope(),
            url,
            *timeout_ms,
            header_secrets,
            &|_| Some("secret".to_owned()),
        ),
        Err(DeliveryAttemptFailure::HttpWebhook { status: None, .. })
    ));
    assert!(
        execute_delivery_attempt_with_clock(
            record_with(
                &crate::graph::test_support::missing_process_path(),
                &[],
                "1",
                1_000,
            ),
            "2026-08-25T00:00:00Z".to_owned(),
            || Err(CoreError::internal("clock failed")),
        )
        .is_err()
    );
}

#[test]
fn diagnostic_messages_are_single_line_utf8_prefixes_bounded_by_bytes() {
    let value = format!("line one\n{}", "é".repeat(2_048));
    let bounded = bounded_message(value);
    assert!(!bounded.contains(['\n', '\r']));
    assert!(bounded.len() <= 2_048);
    assert!(std::str::from_utf8(bounded.as_bytes()).is_ok());
    let exact = "x".repeat(2_048);
    assert_eq!(bounded_message(exact.clone()), exact);
    assert_eq!(bounded_message("x".repeat(2_049)).len(), 2_048);

    struct FailingSerialize;
    impl serde::Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("failed"))
        }
    }
    assert!(matches!(
        process_payload(&FailingSerialize),
        Err(DeliveryAttemptFailure::Process { .. })
    ));
    assert!(matches!(
        webhook_payload(&FailingSerialize),
        Err(DeliveryAttemptFailure::HttpWebhook { status: None, .. })
    ));
    let writer_failure = DeliveryAttemptFailure::Process {
        message: "write failed".to_owned(),
    };
    assert_eq!(
        classify_writer_result(Ok(Err(writer_failure.clone()))),
        Err(writer_failure)
    );
    assert!(matches!(
        classify_writer_result(Err(Box::new("writer panic"))),
        Err(DeliveryAttemptFailure::Process { message }) if message == "process stdin writer failed"
    ));
}

#[test]
fn process_deadline_and_empty_stderr_diagnostics_are_exact() {
    let now = Instant::now();
    assert!(before_deadline(now, now + Duration::from_nanos(1)));
    assert!(!before_deadline(now, now));
    assert!(!before_deadline(now + Duration::from_nanos(1), now));

    #[cfg(unix)]
    {
        let DeliveryExecution::Retry(record) = execute_delivery_attempt_with_clock(
            record_with(
                "/bin/sh",
                &["-c".to_owned(), "exit 7".to_owned()],
                "value",
                1_000,
            ),
            "2026-08-25T00:00:00Z".to_owned(),
            || Ok("2026-08-25T00:01:00Z".to_owned()),
        )
        .expect("failed process becomes retry") else {
            panic!("failed process must retry");
        };
        let wire = serde_json::to_value(record).expect("record wire");
        assert_eq!(
            wire["attempts"][0]["failure"]["message"],
            "process exited unsuccessfully"
        );
    }
}

#[test]
fn irrelevant_webhook_response_bodies_are_discarded_under_a_fixed_read_bound() {
    let mut body = std::io::Cursor::new(vec![b'x'; 100_000]);
    discard_bounded_response_body(&mut body);
    assert_eq!(body.position(), 65_536);
    assert!(finish_webhook_response(204, &mut std::io::Cursor::new(b"ignored")).is_ok());
    assert!(matches!(
        finish_webhook_response(500, &mut std::io::Cursor::new(b"ignored")),
        Err(DeliveryAttemptFailure::HttpWebhook {
            status: Some(500),
            ..
        })
    ));
    assert!(matches!(
        send_and_finish_webhook("{}".to_owned(), |_| Err(ureq::Error::ConnectionFailed)),
        Err(DeliveryAttemptFailure::HttpWebhook { status: None, .. })
    ));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("request");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request).expect("request");
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("response");
    });
    let request = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .new_agent()
        .post(&format!("http://{address}/hook"));
    assert!(send_and_finish_webhook("{}".to_owned(), |payload| request.send(payload)).is_ok());
    worker.join().expect("worker");

    assert!(classify_webhook_status(200).is_ok());
    assert!(classify_webhook_status(299).is_ok());
    assert!(matches!(
        classify_webhook_status(500),
        Err(DeliveryAttemptFailure::HttpWebhook {
            status: Some(500),
            ..
        })
    ));

    let secret = super::super::DeliveryHeaderSecret {
        env: "TOKEN".to_owned(),
        format: "Bearer {value}".to_owned(),
    };
    assert_eq!(
        resolve_delivery_secret(&secret, |_| Some("value".to_owned())).expect("secret"),
        "Bearer value"
    );
    assert!(resolve_delivery_secret(&secret, |_| None).is_err());
    assert!(resolve_delivery_secret(&secret, |_| Some("line\nbreak".to_owned())).is_err());

    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("broken"))
        }
    }
    assert_eq!(read_bounded_stderr(&mut BrokenReader), "");
    assert_eq!(
        read_bounded_stderr(&mut std::io::Cursor::new("line one\nline two")),
        "line one line two"
    );
    assert_eq!(
        read_bounded_stderr(&mut std::io::Cursor::new(vec![b'x'; 3_000])).len(),
        2_048
    );
}

#[cfg(unix)]
#[test]
fn process_timeout_bounds_a_writer_blocked_by_a_child_that_never_reads_stdin() {
    let started = std::time::Instant::now();
    let result = execute_delivery_attempt(
        record_with(
            "/bin/sh",
            &["-c".to_owned(), "sleep 5".to_owned()],
            &"x".repeat(1_000_000),
            100,
        ),
        "2026-08-25T00:00:00Z".to_owned(),
    )
    .expect("bounded retry");
    assert!(matches!(result, DeliveryExecution::Retry(_)));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));

    let mut child_command = std::process::Command::new("/bin/sh");
    child_command
        .args(["-c", "exit 0"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null());
    let child = spawn_managed_process(child_command).expect("child");
    assert!(matches!(
        deliver_spawned_process(child, "payload".to_owned(), 1_000),
        Err(DeliveryAttemptFailure::Process { message }) if message == "process delivery did not provide stdin"
    ));
    let mut child_command = std::process::Command::new("/bin/sh");
    child_command
        .args(["-c", "cat >/dev/null"])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null());
    let child = spawn_managed_process(child_command).expect("child");
    assert!(matches!(
        deliver_spawned_process_with_poll(child, "payload".to_owned(), 1_000, |_| {
            Err(std::io::Error::other("wait failed"))
        }),
        Err(DeliveryAttemptFailure::Process { message }) if message.contains("wait failed")
    ));
}

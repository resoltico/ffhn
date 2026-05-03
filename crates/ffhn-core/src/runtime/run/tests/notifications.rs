use super::support::*;
use std::collections::BTreeMap;
use std::io::Write;

struct FailOnSecondWrite {
    writes: usize,
}

impl Write for FailOnSecondWrite {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writes += 1;
        if self.writes == 1 {
            Ok(buf.len())
        } else {
            Err(std::io::Error::other("newline write failed"))
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn notification_helpers_cover_payload_failures_wait_paths_and_panics() {
    noop_abort();
    assert!(
        write_notification_payload_or_failure::<BrokenWriter>(
            "notify",
            Instant::now(),
            None,
            "payload",
            noop_abort,
        )
        .is_none()
    );

    let payload_failure = write_notification_payload_or_failure(
        "notify",
        Instant::now(),
        Some(BrokenWriter),
        "payload",
        || {},
    )
    .expect("payload failure");
    assert!(!payload_failure.is_delivered());
    assert!(payload_failure.error().is_some());
    assert!(BrokenWriter.flush().is_err());

    #[cfg(unix)]
    {
        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg("exec 0<&-; sleep 5")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("notification helper child");
        thread::sleep(Duration::from_millis(100));
        let child_payload_failure = write_child_notification_payload_or_failure(
            "notify",
            Instant::now(),
            &mut child,
            &"payload".repeat(16_384),
        )
        .expect("child payload failure");
        assert!(!child_payload_failure.is_delivered());
        assert!(child_payload_failure.error().is_some());
        assert!(child.try_wait().expect("child try_wait").is_some());
    }

    let success = wait_for_notification_process(
        &mut FakeNotificationProcess {
            polls: vec![Ok(Some(exit_status(0)))],
            killed: 0,
            waited: 0,
        },
        "notify",
        Instant::now(),
        50,
    );
    assert!(success.is_delivered());

    let failure = wait_for_notification_process(
        &mut FakeNotificationProcess {
            polls: vec![Ok(Some(exit_status(7)))],
            killed: 0,
            waited: 0,
        },
        "notify",
        Instant::now(),
        50,
    );
    assert_eq!(failure.exit_code(), Some(7));
    assert!(!failure.is_delivered());

    let mut timed_out_process = FakeNotificationProcess {
        polls: vec![Ok(None)],
        killed: 0,
        waited: 0,
    };
    let timed_out = wait_for_notification_process(
        &mut timed_out_process,
        "notify",
        Instant::now() - Duration::from_millis(100),
        10,
    );
    assert!(timed_out.is_timed_out());
    assert_eq!(timed_out_process.killed, 1);
    assert_eq!(timed_out_process.waited, 1);

    let wait_error = wait_for_notification_process(
        &mut FakeNotificationProcess {
            polls: vec![Err(std::io::Error::other("wait failed"))],
            killed: 0,
            waited: 0,
        },
        "notify",
        Instant::now(),
        50,
    );
    assert!(
        wait_error
            .error()
            .expect("wait error")
            .contains("wait failed")
    );

    let panic = join_batch_handle(thread::spawn(|| panic!("boom")));
    assert!(panic.is_err());
    assert!(
        panic
            .expect_err("panic")
            .to_string()
            .contains("batch worker panicked: boom")
    );
}

#[test]
fn notification_helper_writes_newline_delimited_json_payloads() {
    let mut sink = Vec::new();
    let failure = write_notification_payload_or_failure(
        "notify",
        Instant::now(),
        Some(&mut sink),
        "{\"kind\":\"payload\"}",
        noop_abort,
    );
    assert!(failure.is_none());
    assert_eq!(
        String::from_utf8(sink).expect("utf8 payload"),
        "{\"kind\":\"payload\"}\n"
    );

    let newline_failure = write_notification_payload_or_failure(
        "notify",
        Instant::now(),
        Some(FailOnSecondWrite { writes: 0 }),
        "{\"kind\":\"payload\"}",
        noop_abort,
    )
    .expect("newline write failure");
    assert!(!newline_failure.is_delivered());
    assert_eq!(newline_failure.exit_code(), None);
    assert!(
        newline_failure
            .error()
            .is_some_and(|error| error.contains("newline write failed"))
    );
}

#[test]
fn finish_report_and_notifications_cover_failure_paths() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let target = target_document(
        "demo",
        true,
        Url::parse("https://example.com").expect("url"),
        "main",
        SelectionMatch::Single,
    );

    std::fs::create_dir_all(paths.last_run_file()).expect("block last_run path");
    let report = finish_report(&paths, Some(&target), live_success_report("demo"))
        .expect("finish report with blocked last_run");
    assert!(!report.persist.wrote_last_run());
    assert!(report.persist.error().is_some());

    let no_target = finish_report(&paths, None, live_success_report("demo"))
        .expect("finish report without target");
    assert!(no_target.notifications.is_empty());
    assert!(!no_target.persist.wrote_last_run());

    // The remaining sub-tests spawn /bin/sh processes; skip them on non-Unix platforms.
    #[cfg(unix)]
    {
        // Successful hooks must drain stdin; an immediate `exit 0` races the payload write and can
        // turn a success-path test into a broken-pipe test under load.
        let delivered = deliver_notification(
            &NotificationHook {
                name: "ok".to_owned(),
                on: vec![RunOutcome::Changed],
                program: "/bin/sh".to_owned(),
                args: vec!["-c".to_owned(), "cat >/dev/null; exit 0".to_owned()],
                timeout_ms: 500,
            },
            &target,
            &live_success_report("demo"),
        );
        assert!(delivered.is_delivered());

        let delivered_with_stderr = deliver_notification(
            &NotificationHook {
                name: "ok-with-stderr".to_owned(),
                on: vec![RunOutcome::Changed],
                program: "/bin/sh".to_owned(),
                args: vec![
                    "-c".to_owned(),
                    "cat >/dev/null; echo ok-msg >&2; exit 0".to_owned(),
                ],
                timeout_ms: 500,
            },
            &target,
            &live_success_report("demo"),
        );
        assert!(delivered_with_stderr.is_delivered());
        assert!(delivered_with_stderr.error().is_none());

        let exited = deliver_notification(
            &NotificationHook {
                name: "fail".to_owned(),
                on: vec![RunOutcome::Changed],
                program: "/bin/sh".to_owned(),
                args: vec![
                    "-c".to_owned(),
                    "cat >/dev/null; echo hook-broke >&2; exit 7".to_owned(),
                ],
                timeout_ms: 500,
            },
            &target,
            &live_success_report("demo"),
        );
        assert_eq!(exited.exit_code(), Some(7));
        assert!(!exited.is_delivered());
        assert!(
            exited
                .error()
                .is_some_and(|error| error.contains("hook-broke"))
        );

        let timed_out = deliver_notification(
            &NotificationHook {
                name: "timeout".to_owned(),
                on: vec![RunOutcome::Changed],
                program: "/bin/sh".to_owned(),
                args: vec!["-c".to_owned(), "cat >/dev/null; sleep 1".to_owned()],
                timeout_ms: 10,
            },
            &target,
            &live_success_report("demo"),
        );
        assert!(timed_out.is_timed_out());

        let payload_failure = deliver_notification(
            &NotificationHook {
                name: "payload".to_owned(),
                on: vec![RunOutcome::Changed],
                program: "/bin/sh".to_owned(),
                args: vec![
                    "-c".to_owned(),
                    "echo payload-stderr >&2; exec 0<&-; sleep 1".to_owned(),
                ],
                timeout_ms: 500,
            },
            &target,
            &{
                let mut report = live_success_report("demo");
                report.extensions = Some(BTreeMap::from([(
                    "blob".to_owned(),
                    json!("x".repeat(200_000)),
                )]));
                report.with_digest().expect("large payload digest")
            },
        );
        assert!(!payload_failure.is_delivered());
        assert!(
            payload_failure
                .error()
                .is_some_and(|error| error.contains("payload-stderr"))
        );

        let spawn_error = deliver_notification(
            &NotificationHook {
                name: "spawn".to_owned(),
                on: vec![RunOutcome::Changed],
                program: "/no/such/program".to_owned(),
                args: vec!["-c".to_owned(), "exit 0".to_owned()],
                timeout_ms: 500,
            },
            &target,
            &live_success_report("demo"),
        );
        assert!(spawn_error.error().is_some());

        let payload_serialization_failure = deliver_notification(
            &NotificationHook {
                name: "invalid-report".to_owned(),
                on: vec![RunOutcome::Changed],
                program: "/bin/sh".to_owned(),
                args: vec!["-c".to_owned(), "exit 0".to_owned()],
                timeout_ms: 500,
            },
            &target,
            &{
                let mut report = live_success_report("demo");
                report.schema_name = "not.ffhn.run_report".to_owned();
                report
            },
        );
        assert!(!payload_serialization_failure.is_delivered());
        assert!(
            payload_serialization_failure
                .error()
                .is_some_and(|error| error.contains("failed to serialize notification payload"))
        );
    } // end #[cfg(unix)]
}

#[cfg(unix)]
#[test]
fn deliver_notification_allows_predelivery_persist_error_reports() {
    let temp = tempdir().expect("tempdir");
    let payload_path = temp.path().join("payload.json");
    let env_path = temp.path().join("env.txt");
    let target = target_document(
        "demo",
        true,
        Url::parse("https://example.com").expect("url"),
        "main",
        SelectionMatch::Single,
    );
    let mut report = live_success_report("demo");
    report.run_outcome = RunOutcome::FailedTransient;
    report.reason_code = ReasonCode::PersistError;
    report.failure_class = Some(crate::FailureClass::Transient);
    report.error_detail = Some(crate::ProcessErrorDetail {
        kind: crate::ProcessErrorKind::Io,
        message: "permission denied".to_owned(),
        path: Some("/tmp/watch/demo/state.json".to_owned()),
    });
    report.persist.state_write = PersistWriteStatus::Failed {
        error: crate::ProcessErrorDetail {
            kind: crate::ProcessErrorKind::Io,
            message: "permission denied".to_owned(),
            path: Some("/tmp/watch/demo/state.json".to_owned()),
        },
    };
    report.persist.last_run_write = PersistWriteStatus::NotAttempted;
    report = report.with_digest().expect("persist error digest");

    let command = format!(
        "cat > '{}'; printf '%s\\n%s\\n' \
\"$FFHN_RUN_OUTCOME\" \"$FFHN_REASON_CODE\" > '{}'",
        payload_path.display(),
        env_path.display(),
    );
    let delivered = deliver_notification(
        &NotificationHook {
            name: "capture".to_owned(),
            on: vec![RunOutcome::FailedTransient],
            program: "/bin/sh".to_owned(),
            args: vec!["-c".to_owned(), command],
            timeout_ms: 500,
        },
        &target,
        &report,
    );
    assert!(delivered.is_delivered());
    let payload: crate::NotificationPayload =
        serde_json::from_str(&std::fs::read_to_string(&payload_path).expect("payload"))
            .expect("notification payload");
    payload.validate().expect("valid persist-error payload");
    assert!(payload.run_report.persist.error().is_some());
    assert_eq!(
        std::fs::read_to_string(&env_path).expect("env"),
        "failed_transient\npersist_error\n"
    );
}

#[cfg(unix)]
#[test]
fn deliver_notification_passes_documented_env_vars_and_stdin_payload() {
    let temp = tempdir().expect("tempdir");
    let payload_path = temp.path().join("payload.json");
    let env_path = temp.path().join("env.txt");
    let target = target_document(
        "demo",
        true,
        Url::parse("https://example.com").expect("url"),
        "main",
        SelectionMatch::Single,
    );
    let report = live_success_report("demo");
    let command = format!(
        "cat > '{}'; printf '%s\\n%s\\n%s\\n%s\\n%s\\n' \
\"$FFHN_TARGET_ID\" \"$FFHN_RUN_OUTCOME\" \"$FFHN_REASON_CODE\" \
\"$FFHN_RUN_MODE\" \"$FFHN_FAILURE_CLASS\" > '{}'",
        payload_path.display(),
        env_path.display(),
    );
    let delivered = deliver_notification(
        &NotificationHook {
            name: "capture".to_owned(),
            on: vec![RunOutcome::Changed],
            program: "/bin/sh".to_owned(),
            args: vec!["-c".to_owned(), command],
            timeout_ms: 500,
        },
        &target,
        &report,
    );
    assert!(delivered.is_delivered());
    let payload: crate::NotificationPayload =
        serde_json::from_str(&std::fs::read_to_string(&payload_path).expect("payload"))
            .expect("notification payload");
    payload.validate().expect("valid notification payload");
    assert_eq!(payload.hook_name(), "capture");
    assert_eq!(payload.run_report.target_id(), "demo");
    assert!(!payload.run_report.persist.wrote_last_run());
    assert!(payload.run_report.notifications.is_empty());
    assert_eq!(
        std::fs::read_to_string(&env_path).expect("env"),
        "demo\nchanged\nok\nlive\n\n"
    );
}

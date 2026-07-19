use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::*;
use crate::model::{ProcessStdinEventKey, ProcessStdinPayload, StagedOutboxRecord};
use crate::model::{delivery_failure_detail, io_detail};
use crate::{
    ConditionIssue, DeliveryEventKind, DeliveryProcessAttempt, DeliveryStatus, DiagnosticDetail,
    DiagnosticOperation, ExactByteCount, IntegrationFaultCode, IoErrorClass, OnRunEventCause,
    OutboxPolicy, PermanentErrorCode, PolicyRunInput, RouteFamily, SourceSuspectReason,
    StagedEventEligibility, StateDocument, StderrCapture, StderrOutcome, TargetDocument, TargetId,
    TargetPaths, TerminalOutcome, WriterOutcome,
};

mod route_admission;

const FIRST: &str = "2026-07-15T12:00:00Z";
const SECOND: &str = "2026-07-15T12:00:01Z";

fn delivery_failure() -> DiagnosticDetail {
    delivery_failure_detail(DeliveryProcessAttempt::new(
        TerminalOutcome::Exited { exit_code: Some(1) },
        WriterOutcome::Completed,
        StderrOutcome::captured(
            StderrCapture::from_bytes(Vec::new(), ExactByteCount::zero()).expect("empty capture"),
        ),
    ))
    .expect("delivery failure detail")
}

fn routed_target() -> TargetDocument {
    let source_path = crate::test_support::absolute_file_path("source.json");
    let program = crate::test_support::PROCESS_PROGRAM;
    let target: TargetDocument = toml::from_str(&format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 2\ndeclared_type = \"integer\"\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n\n[[conditions]]\ncondition_id = \"changed\"\n\n[conditions.predicate]\nkind = \"changed\"\nreference = \"last_accepted_observation\"\n\n[[conditions]]\ncondition_id = \"low\"\n\n[conditions.predicate]\nkind = \"lt\"\nthreshold = \"75\"\n\n[[routes]]\nroute_id = \"condition\"\nroute_family = \"on_condition\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = {program:?}\ntimeout_ms = 1000\n\n[[routes]]\nroute_id = \"run\"\nroute_family = \"on_run\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = {program:?}\ntimeout_ms = 1000\n",
        ))
        .expect("target TOML");
    target.validate().expect("valid target");
    target
}

fn observation(target: &TargetDocument, value: i64) -> crate::Observation {
    target
        .parse_json_scalar_token(value.to_string())
        .expect("typed observation")
}

fn state_with_condition_facts(target: &TargetDocument) -> StateDocument {
    let digest = target.contract_digest_sha256().expect("contract digest");
    let mut state = StateDocument::new(TargetId::new("demo").expect("target id"), digest);
    let first = observation(target, 100);
    let staged = target
        .stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: &first,
            },
            &state.condition_contexts(target),
        )
        .expect("initial policy stage");
    state
        .apply_valid_observation(
            target,
            first,
            staged.condition_evaluations().expect("evaluations"),
            FIRST,
        )
        .expect("initial state");

    let second = observation(target, 50);
    let staged = target
        .stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: &second,
            },
            &state.condition_contexts(target),
        )
        .expect("second policy stage");
    state
        .apply_valid_observation(
            target,
            second,
            staged.condition_evaluations().expect("evaluations"),
            SECOND,
        )
        .expect("second state");
    state
}

fn failing_delivery_target(max_attempts: u32) -> TargetDocument {
    let mut value = serde_json::to_value(routed_target()).expect("target value");
    value["outbox"] = serde_json::json!({
        "max_pending": 100,
        "max_attempts": max_attempts,
        "base_backoff_ms": 1_000,
        "max_backoff_ms": 1_000,
    });
    let (program, args) = process::test_process_command("fail", None);
    value["routes"][0]["adapter"]["program"] = serde_json::json!(program);
    value["routes"][0]["adapter"]["args"] = serde_json::json!(args);
    let target: TargetDocument = serde_json::from_value(value).expect("failing target");
    target.validate().expect("valid failing target");
    target
}

fn queued_delivery_state(target: &TargetDocument, _event_id: &str) -> StateDocument {
    let mut state = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        target.contract_digest_sha256().expect("digest"),
    );
    let route_id = "condition".parse().expect("route id");
    let payload = ProcessStdinPayload::new(
        &route_id,
        RouteFamily::OnCondition,
        &TargetId::new("demo").expect("target id"),
        "Demo",
        ProcessStdinEventKey::ConditionEvent {
            condition_id: "changed".parse().expect("condition id"),
            observation_seq: 1,
        },
        "Demo[condition=changed]: satisfied at observation 1 (value=1)",
        Some("changed".parse().expect("condition id")),
        Some(1),
        Some("1".to_owned()),
        None,
        None,
        None,
        None,
    )
    .expect("payload");
    let event_id = payload.event_id().to_owned();
    let payload = payload.immutable_bytes().expect("payload bytes");
    state
        .enqueue_outbox(
            vec![StagedOutboxRecord {
                event_id,
                route_id,
                route_family: RouteFamily::OnCondition,
                event_kind: DeliveryEventKind::ConditionSatisfied,
                condition_id: Some("changed".parse().expect("condition id")),
                immutable_payload: payload,
            }],
            target.outbox(),
            FIRST,
        )
        .expect("queued delivery");
    state
}

fn queued_event_id(state: &StateDocument) -> String {
    state
        .next_due_outbox_record(FIRST)
        .expect("queued state")
        .expect("queued record")
        .event_id()
        .to_owned()
}

#[test]
fn materialization_preserves_all_eligibility_facts_in_immutable_route_payloads() {
    let target = routed_target();
    let digest = target.contract_digest_sha256().expect("contract digest");
    let state = state_with_condition_facts(&target);
    let eligibilities = [
        StagedEventEligibility::OnCondition {
            condition_id: "changed".parse().expect("condition id"),
        },
        StagedEventEligibility::OnCondition {
            condition_id: "low".parse().expect("condition id"),
        },
        StagedEventEligibility::OnRun {
            cause: OnRunEventCause::Initialized,
        },
        StagedEventEligibility::OnRun {
            cause: OnRunEventCause::Reset,
        },
        StagedEventEligibility::OnRun {
            cause: OnRunEventCause::ConditionIssue {
                condition_id: "changed".parse().expect("condition id"),
                issue: ConditionIssue::ArithmeticOverflow,
            },
        },
        StagedEventEligibility::OnRun {
            cause: OnRunEventCause::ConditionIssue {
                condition_id: "low".parse().expect("condition id"),
                issue: ConditionIssue::ZeroReference,
            },
        },
    ];
    let records = materialize(&target, &state, &eligibilities, &digest).expect("records");
    assert_eq!(records.len(), eligibilities.len());
    let payloads = records
        .iter()
        .map(|record| serde_json::from_slice::<serde_json::Value>(&record.immutable_payload))
        .collect::<Result<Vec<_>, _>>()
        .expect("payload JSON");
    for payload in &payloads {
        assert_eq!(payload["schema_name"], "ffhn.process_stdin");
        assert_eq!(payload["schema_version"], 4);
        assert_eq!(payload["target_id"], "demo");
    }
    assert_eq!(payloads[0]["route_id"], "condition");
    assert_eq!(payloads[0]["event_kind"], "condition_satisfied");
    assert_eq!(payloads[0]["condition_id"], "changed");
    assert_eq!(payloads[0]["observation_seq"], 2);
    assert_eq!(payloads[1]["condition_id"], "low");
    assert_eq!(
        payloads[1]["event_key"],
        serde_json::json!({
            "key_kind": "condition_level",
            "condition_id": "low",
            "entry_at": SECOND,
        })
    );
    assert_eq!(payloads[2]["route_id"], "run");
    assert_eq!(payloads[2]["event_kind"], "initialized");
    assert_eq!(payloads[3]["event_kind"], "reset");
    assert_eq!(payloads[4]["event_kind"], "arithmetic_overflow");
    assert_eq!(payloads[5]["event_kind"], "zero_reference");

    let changed_id = crate::stable_json::stable_digest(&serde_json::json!({
        "target_id": "demo",
        "route_family": "on_condition",
        "key": {"condition_id": "changed", "observation_seq": 2},
    }))
    .expect("changed id");
    assert_eq!(records[0].event_id, changed_id);
    let low_id = crate::stable_json::stable_digest(&serde_json::json!({
        "target_id": "demo",
        "route_family": "on_condition",
        "key": {"condition_id": "low", "entry_at": SECOND},
    }))
    .expect("level id");
    assert_eq!(records[1].event_id, low_id);
}

#[test]
fn on_run_health_and_contract_episodes_have_stable_keys_and_missing_facts_fail_closed() {
    let target = routed_target();
    let digest = target.contract_digest_sha256().expect("contract digest");
    let mut state = StateDocument::new(TargetId::new("demo").expect("target id"), digest.clone());
    let detail = io_detail(
        IoErrorClass::ConnectionRefused,
        DiagnosticOperation::HttpFetch,
        "source unavailable",
        None,
    );
    assert!(
        state
            .apply_source_suspect(SourceSuspectReason::FetchFailed, detail, FIRST, 1)
            .expect("source episode")
    );
    let source = materialize(
        &target,
        &state,
        &[StagedEventEligibility::OnRun {
            cause: OnRunEventCause::SourceSuspectEscalated {
                reason_class: SourceSuspectReason::FetchFailed,
            },
        }],
        &digest,
    )
    .expect("source materialization");
    let payload: serde_json::Value =
        serde_json::from_slice(&source[0].immutable_payload).expect("source payload");
    assert_eq!(payload["reason_class"], "fetch_failed");
    assert_eq!(payload["episode_started_at"], FIRST);

    assert!(
        state
            .apply_permanent_error(PermanentErrorCode::InvalidJsonPointer, SECOND)
            .expect("permanent episode")
    );
    let permanent = materialize(
        &target,
        &state,
        &[StagedEventEligibility::OnRun {
            cause: OnRunEventCause::PermanentContractErrorEpisodeBegan {
                error_code: PermanentErrorCode::InvalidJsonPointer,
            },
        }],
        &digest,
    )
    .expect("permanent materialization");
    let payload: serde_json::Value =
        serde_json::from_slice(&permanent[0].immutable_payload).expect("permanent payload");
    assert_eq!(payload["error_code"], "invalid_json_pointer");
    assert_eq!(payload["episode_started_at"], SECOND);

    assert!(
        state
            .apply_integration_fault(IntegrationFaultCode::HtmlcutInternalError, FIRST)
            .expect("integration episode")
    );
    let integration = materialize(
        &target,
        &state,
        &[StagedEventEligibility::OnRun {
            cause: OnRunEventCause::IntegrationFaultEpisodeBegan {
                integration_fault_code: IntegrationFaultCode::HtmlcutInternalError,
            },
        }],
        &digest,
    )
    .expect("integration materialization");
    let payload: serde_json::Value =
        serde_json::from_slice(&integration[0].immutable_payload).expect("integration payload");
    assert_eq!(payload["event_kind"], "integration_fault");
    assert_eq!(payload["integration_fault_code"], "htmlcut_internal_error");
    assert_eq!(payload["event_key"]["key_kind"], "integration_fault");
    assert_eq!(payload["event_key"]["contract_digest_sha256"], digest);
    assert_eq!(payload["event_key"]["first_seen_at"], FIRST);

    let empty = StateDocument::new(TargetId::new("demo").expect("target id"), digest.clone());
    assert!(
        materialize(
            &target,
            &empty,
            &[StagedEventEligibility::OnCondition {
                condition_id: "missing".parse().expect("condition id"),
            }],
            &digest,
        )
        .is_err()
    );
    assert!(
        materialize(
            &target,
            &empty,
            &[StagedEventEligibility::OnRun {
                cause: OnRunEventCause::IntegrationFaultEpisodeBegan {
                    integration_fault_code: IntegrationFaultCode::HtmlcutInternalError,
                },
            }],
            &digest,
        )
        .is_err()
    );
    assert!(
        materialize(
            &target,
            &empty,
            &[StagedEventEligibility::OnCondition {
                condition_id: "low".parse().expect("condition id"),
            }],
            &digest,
        )
        .is_err()
    );
    assert!(
        materialize(
            &target,
            &empty,
            &[StagedEventEligibility::OnRun {
                cause: OnRunEventCause::SourceSuspectEscalated {
                    reason_class: SourceSuspectReason::FetchFailed,
                },
            }],
            &digest,
        )
        .is_err()
    );
    assert!(
        materialize(
            &target,
            &empty,
            &[StagedEventEligibility::OnRun {
                cause: OnRunEventCause::PermanentContractErrorEpisodeBegan {
                    error_code: PermanentErrorCode::InvalidJsonPointer,
                },
            }],
            &digest,
        )
        .is_err()
    );
}

#[test]
fn drain_preserves_a_clock_failure_as_post_commit_delivery_evidence() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let watch_root = temporary.path().join("watchlist");
    std::fs::create_dir_all(&watch_root).expect("watch root");
    let paths = TargetPaths::try_new(&watch_root, "demo").expect("target paths");
    let target = routed_target();
    let mut state = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        target.contract_digest_sha256().expect("digest"),
    );
    let failure = drain_with_clock(&paths, &target, &mut state, &mut || {
        Err(crate::CoreError::internal("clock failed"))
    })
    .expect_err("clock failure");
    assert!(failure.error.to_string().contains("clock failed"));
    assert!(failure.drain.outcomes().is_empty());
}

#[test]
fn drain_rejects_invalid_due_records_and_missing_routes_before_delivery() {
    let target = routed_target();
    let queued = queued_delivery_state(&target, &"b".repeat(64));
    let mut wire = serde_json::to_value(&queued).expect("queued state JSON");
    wire["outbox"][0]["next_retry_at"] = serde_json::json!("not-a-timestamp");
    let mut invalid_due = StateDocument::from_unvalidated_wire_for_test(wire);
    let mut clock = || Ok(FIRST.to_owned());
    let mut persist = |_: &StateDocument| Ok(());
    let failure = drain_with_clock_and_persist(&target, &mut invalid_due, &mut clock, &mut persist)
        .expect_err("invalid pending retry time");
    assert!(failure.error.to_string().contains("outbox timestamp"));
    assert!(failure.drain.outcomes().is_empty());

    let queued = queued_delivery_state(&target, &"c".repeat(64));
    let mut wire = serde_json::to_value(&queued).expect("queued state JSON");
    wire["outbox"][0]["route_id"] = serde_json::json!("missing");
    let mut missing_route = StateDocument::from_unvalidated_wire_for_test(wire);
    let mut clock = || Ok(FIRST.to_owned());
    let mut persist = |_: &StateDocument| Ok(());
    let failure =
        drain_with_clock_and_persist(&target, &mut missing_route, &mut clock, &mut persist)
            .expect_err("missing delivery route");
    assert!(
        failure
            .error
            .to_string()
            .contains("outbox route disappeared")
    );
    assert!(failure.drain.outcomes().is_empty());
}

#[test]
fn drain_reports_every_uncommitted_outbox_transition() {
    let target = failing_delivery_target(1);
    let mut exhausted = queued_delivery_state(&target, &"e".repeat(64));
    let exhausted_id = queued_event_id(&exhausted);
    exhausted
        .record_outbox_failure(
            &exhausted_id,
            &"condition".parse().expect("route id"),
            delivery_failure(),
            FIRST.to_owned(),
        )
        .expect("exhaust prior attempt");
    let mut clock = || Ok(FIRST.to_owned());
    let mut persist = |_: &StateDocument| Err(crate::CoreError::internal("persist refused"));
    let failure = drain_with_clock_and_persist(&target, &mut exhausted, &mut clock, &mut persist)
        .expect_err("preexisting terminal removal cannot persist");
    let outcomes = failure.drain.outcomes();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].status(), DeliveryStatus::DeadLetterUncommitted);
    assert_eq!(outcomes[0].attempt_count(), 1);

    let mut terminal = queued_delivery_state(&target, &"d".repeat(64));
    let mut clock = || Ok(FIRST.to_owned());
    let mut persist = |_: &StateDocument| Err(crate::CoreError::internal("persist refused"));
    let failure = drain_with_clock_and_persist(&target, &mut terminal, &mut clock, &mut persist)
        .expect_err("terminal removal cannot persist");
    let outcomes = failure.drain.outcomes();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].status(), DeliveryStatus::DeadLetterUncommitted);
    assert_eq!(outcomes[0].attempt_count(), 1);

    let retry_target = failing_delivery_target(2);
    let mut retry = queued_delivery_state(&retry_target, &"a".repeat(64));
    let mut clock = || Ok(FIRST.to_owned());
    let mut persist = |_: &StateDocument| Err(crate::CoreError::internal("persist refused"));
    let failure = drain_with_clock_and_persist(&retry_target, &mut retry, &mut clock, &mut persist)
        .expect_err("retry state cannot persist");
    let outcomes = failure.drain.outcomes();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].status(), DeliveryStatus::RetryUncommitted);
    assert_eq!(outcomes[0].attempt_count(), 1);
}

#[test]
fn failed_delivery_reports_retry_commit_clock_and_recording_failures_as_uncommitted() {
    let target = failing_delivery_target(2);

    let mut after_delivery_clock = queued_delivery_state(&target, &"f".repeat(64));
    let mut calls = 0;
    let mut clock = || {
        calls += 1;
        if calls == 1 {
            Ok(FIRST.to_owned())
        } else {
            Err(crate::CoreError::internal("retry-commit clock failed"))
        }
    };
    let mut persist = |_: &StateDocument| Ok(());
    let failure =
        drain_with_clock_and_persist(&target, &mut after_delivery_clock, &mut clock, &mut persist)
            .expect_err("retry-commit clock failure");
    let outcomes = failure.drain.outcomes();
    assert_eq!(outcomes[0].status(), DeliveryStatus::RetryUncommitted);
    assert!(
        outcomes[0]
            .outbox_error_detail()
            .is_some_and(|detail| detail.message().contains("clock failed"))
    );

    let mut invalid_retry_commit_time = queued_delivery_state(&target, &"1".repeat(64));
    let mut calls = 0;
    let mut clock = || {
        calls += 1;
        Ok(if calls == 1 {
            FIRST.to_owned()
        } else {
            "not-a-timestamp".to_owned()
        })
    };
    let mut persist = |_: &StateDocument| Ok(());
    let failure = drain_with_clock_and_persist(
        &target,
        &mut invalid_retry_commit_time,
        &mut clock,
        &mut persist,
    )
    .expect_err("invalid retry-commit timestamp");
    let outcomes = failure.drain.outcomes();
    assert_eq!(outcomes[0].status(), DeliveryStatus::RetryUncommitted);
    assert!(outcomes[0].outbox_error_detail().is_some());

    let mut recording = queued_delivery_state(&target, &"2".repeat(64));
    let mut clock = || Ok(FIRST.to_owned());
    let mut persist = |_: &StateDocument| Ok(());
    let mut record_failure =
        |_: &mut StateDocument, _: &str, _: &crate::RouteId, _: DiagnosticDetail, _: String| {
            Err(crate::CoreError::internal("record retry failed"))
        };
    let failure = drain_with_dependencies(
        &target,
        &mut recording,
        &mut clock,
        &mut persist,
        &mut record_failure,
    )
    .expect_err("recording failure");
    let outcomes = failure.drain.outcomes();
    assert_eq!(outcomes[0].status(), DeliveryStatus::RetryUncommitted);
    assert!(
        outcomes[0]
            .outbox_error_detail()
            .is_some_and(|detail| detail.message().contains("record retry failed"))
    );
}

#[test]
fn retry_backoff_is_exact_deterministic_and_capped() {
    let policy: OutboxPolicy = serde_json::from_value(serde_json::json!({
        "max_pending": 3,
        "max_attempts": 4,
        "base_backoff_ms": 100,
        "max_backoff_ms": 250,
    }))
    .expect("outbox policy");
    let base = OffsetDateTime::parse("2026-01-01T00:00:00Z", &Rfc3339).expect("base");
    for (attempt, expected_ms) in [(1, 100), (2, 200), (3, 250), (20, 250)] {
        let retry = retry_at("2026-01-01T00:00:00Z", attempt, &policy).expect("retry");
        let retry = OffsetDateTime::parse(&retry, &Rfc3339).expect("retry timestamp");
        assert_eq!(
            (retry - base).whole_milliseconds(),
            expected_ms,
            "{attempt}"
        );
    }
}

#[test]
fn retry_state_commit_is_exact_and_waits_for_a_later_drain() {
    let target = failing_delivery_target(2);
    let mut state = queued_delivery_state(&target, &"f".repeat(64));
    let mut clock_values = [FIRST.to_owned(), SECOND.to_owned()].into_iter();
    let mut calls = 0;
    let mut clock = || {
        calls += 1;
        Ok(clock_values.next().expect("no unexpected clock read"))
    };
    let mut persist = |_: &StateDocument| Ok(());

    let drain = drain_with_clock_and_persist(&target, &mut state, &mut clock, &mut persist)
        .ok()
        .expect("retry remains pending for a later drain");

    let outcomes = drain.outcomes();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].status(), DeliveryStatus::RetryScheduled);
    assert_eq!(outcomes[0].attempt_count(), 1);
    assert_eq!(calls, 2);
    let state_json = serde_json::to_value(&state).expect("state JSON");
    assert_eq!(state_json["outbox"][0]["attempt_count"], 1);
    assert_eq!(
        state_json["outbox"][0]["next_retry_at"],
        "2026-07-15T12:00:02Z"
    );
}

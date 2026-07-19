use super::support::*;
use crate::model::StatusReportParts;

#[test]
fn state_reports_and_tokens_expose_one_complete_v2_document_surface() {
    let document = target("integer", "");
    let observation = document
        .parse_json_scalar_token("7".to_owned())
        .expect("observation");
    assert_eq!(observation.raw_selected(), "7");
    assert_eq!(observation.comparison_projection(), "7");
    assert_eq!(observation.canonical_value(), "7");
    let digest = document.contract_digest_sha256().expect("digest");
    let mut state = StateDocument::new(TargetId::new("demo").expect("target"), digest.clone());
    state
        .apply_valid_observation(&document, observation.clone(), &[], "2026-07-15T00:00:00Z")
        .expect("apply valid observation");
    state.validate().expect("state");
    assert_eq!(state.target_id(), "demo");
    assert_eq!(state.contract_digest_sha256(), digest);
    assert_eq!(state.accepted_observation(), Some(&observation));
    let mut invalid_state = mutate_state(&state, |wire| {
        wire["schema_name"] = serde_json::json!("other");
    });
    assert!(invalid_state.validate().is_err());
    invalid_state = mutate_state(&state, |wire| {
        wire["schema_version"] = serde_json::json!(14);
    });
    assert!(invalid_state.validate().is_err());
    invalid_state = mutate_state(&state, |wire| {
        wire["parser_id"] = serde_json::json!("other");
    });
    assert!(invalid_state.validate().is_err());
    invalid_state = mutate_state(&state, |wire| {
        wire["parser_grammar_version"] = serde_json::json!(0);
    });
    assert!(invalid_state.validate().is_err());
    invalid_state = mutate_state(&state, |wire| {
        wire["contract_digest_sha256"] = serde_json::json!("bad");
    });
    assert!(invalid_state.validate().is_err());

    let mut invalid_wire = serde_json::to_value(&state).expect("state JSON");
    invalid_wire["accepted_observation"]["canonical_value"] =
        serde_json::Value::String("not-an-integer".to_owned());
    assert!(serde_json::from_value::<StateDocument>(invalid_wire).is_err());
    let mut invalid_wire = serde_json::to_value(&state).expect("state JSON");
    invalid_wire["accepted_observation"]["parser_id"] =
        serde_json::Value::String("other-parser".to_owned());
    assert!(serde_json::from_value::<StateDocument>(invalid_wire).is_err());
    let mut invalid_wire = serde_json::to_value(&state).expect("state JSON");
    invalid_wire["accepted_observation"]["parser_grammar_version"] =
        serde_json::Value::Number(0.into());
    assert!(serde_json::from_value::<StateDocument>(invalid_wire).is_err());
    let invalid_state = mutate_state(&state, |wire| {
        wire["accepted_observation"]["comparison_projection"] = serde_json::json!("8");
    });
    assert!(invalid_state.validate().is_err());
    let invalid_state = mutate_state(&state, |wire| {
        wire["accepted_observation"]["raw_selected"] = serde_json::json!(" ");
        wire["accepted_observation"]["comparison_projection"] = serde_json::json!(" ");
    });
    assert!(invalid_state.validate().is_err());
    let invalid_state = mutate_state(&state, |wire| {
        wire["accepted_observation"]["raw_selected"] = serde_json::json!(r#""not-an-integer""#);
        wire["accepted_observation"]["comparison_projection"] =
            serde_json::json!(r#""not-an-integer""#);
    });
    assert!(invalid_state.validate().is_err());
    let invalid_state = mutate_state(&state, |wire| {
        wire["accepted_observation"]["parse_diagnostics"] =
            serde_json::json!(["invented diagnostic"]);
    });
    assert!(invalid_state.validate().is_err());

    let detail = plain_detail(
        DiagnosticKind::Contract,
        DiagnosticOperation::TargetValidation,
        "detail",
        Some("path".to_owned()),
    );
    let lifecycle = state.lifecycle_snapshot().expect("lifecycle snapshot");
    assert_eq!(detail.kind(), DiagnosticKind::Contract);
    assert_eq!(detail.message(), "detail");
    assert_eq!(detail.path(), Some("path"));
    let report = RunReport::new(RunReportParts {
        target_id: "demo".to_owned(),
        display_name: Some("Demo".to_owned()),
        run_mode: RunMode::Live,
        outcome: RunOutcome::Changed,
        started: "2026-01-01T00:00:00Z".to_owned(),
        finished: "2026-01-01T00:00:01Z".to_owned(),
        digest: Some(digest),
        observation: Some(observation),
        previous: Some("6".to_owned()),
        error: Some(detail),
        policy_evaluation: PolicyEvaluation::not_evaluated(),
        lifecycle_before: None,
        lifecycle_after: Some(lifecycle.clone()),
        state_persisted: false,
        delivery_outcomes: Vec::new(),
        outbox_overflow: Vec::new(),
        outbox_error_detail: None,
    })
    .expect("valid report");
    assert_eq!(report.target_id(), "demo");
    assert_eq!(report.display_name(), Some("Demo"));
    assert_eq!(report.run_mode(), RunMode::Live);
    assert_eq!(report.outcome(), RunOutcome::Changed);
    assert!(report.observation().is_some());
    assert!(report.error_detail().is_some());
    assert_eq!(report.previous_canonical_value(), Some("6"));
    assert_eq!(report.run_started_at(), "2026-01-01T00:00:00Z");
    assert_eq!(report.run_finished_at(), "2026-01-01T00:00:01Z");
    assert!(report.contract_digest_sha256().is_some());
    assert!(!report.policy_evaluation().is_evaluated());
    assert!(!report.state_persisted());
    let batch = BatchRunReport::new(RunMode::Live, vec!["demo".to_owned()], vec![report]);
    assert_eq!(batch.reports().len(), 1);
    assert_eq!(batch.run_mode(), RunMode::Live);
    assert_eq!(batch.requested_targets(), ["demo"]);
    let batch_wire = serde_json::to_value(&batch).expect("batch JSON");
    assert_eq!(batch_wire["schema_name"], "ffhn.batch_run_report");
    assert_eq!(batch_wire["schema_version"], 17);
    let mut legacy_batch = batch_wire.clone();
    legacy_batch["schema_version"] = serde_json::json!(14);
    assert!(serde_json::from_value::<BatchRunReport>(legacy_batch).is_err());
    let mut foreign_batch = batch_wire.clone();
    foreign_batch["schema_name"] = serde_json::json!("other.batch_report");
    assert!(serde_json::from_value::<BatchRunReport>(foreign_batch).is_err());
    let mut legacy_run = batch_wire["reports"][0].clone();
    legacy_run["schema_version"] = serde_json::json!(14);
    assert!(serde_json::from_value::<RunReport>(legacy_run).is_err());
    let mut foreign_run = batch_wire["reports"][0].clone();
    foreign_run["schema_name"] = serde_json::json!("other.run_report");
    assert!(serde_json::from_value::<RunReport>(foreign_run).is_err());
    let status = StatusReport::new(StatusReportParts {
        target_id: "demo".to_owned(),
        kind: StatusKind::Ready,
        display_name: Some("Demo".to_owned()),
        enabled: Some(true),
        digest: Some("a".repeat(64)),
        observation: state.accepted_observation().cloned(),
        error: None,
        lifecycle: Some(lifecycle),
    })
    .expect("valid status");
    assert_eq!(status.kind(), StatusKind::Ready);
    assert_eq!(status.target_id(), "demo");
    assert_eq!(status.display_name(), Some("Demo"));
    assert_eq!(status.enabled(), Some(true));
    assert!(status.contract_digest_sha256().is_some());
    assert!(status.accepted_observation().is_some());
    assert_eq!(status.error_detail(), None);
    let status_wire = serde_json::to_value(&status).expect("status JSON");
    assert_eq!(status_wire["schema_version"], 13);
    let mut legacy_status = status_wire.clone();
    legacy_status["schema_version"] = serde_json::json!(11);
    assert!(serde_json::from_value::<StatusReport>(legacy_status).is_err());
    let mut foreign_status = status_wire;
    foreign_status["schema_name"] = serde_json::json!("other.status_report");
    assert!(serde_json::from_value::<StatusReport>(foreign_status).is_err());

    let status_wire = serde_json::to_value(&status).expect("status JSON");
    let mut ready_without_observation = status_wire.clone();
    ready_without_observation["accepted_observation"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<StatusReport>(ready_without_observation).is_err());
    let mut ready_without_lifecycle = status_wire.clone();
    ready_without_lifecycle["lifecycle"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<StatusReport>(ready_without_lifecycle).is_err());
    let mut pending_with_observation = status_wire;
    pending_with_observation["kind"] = serde_json::json!("pending");
    assert!(serde_json::from_value::<StatusReport>(pending_with_observation).is_err());

    let reset = ResetReport::new("demo", true, Vec::new(), Vec::new(), None);
    assert!(reset.storage_cleared());
    assert_eq!(reset.target_id(), "demo");
    let reset_wire = serde_json::to_value(&reset).expect("reset JSON");
    assert_eq!(reset_wire["storage_cleared"], true);
    let mut legacy_reset = reset_wire.clone();
    legacy_reset["schema_version"] = serde_json::json!(6);
    assert!(serde_json::from_value::<ResetReport>(legacy_reset).is_err());
    let mut foreign_reset = reset_wire;
    foreign_reset["schema_name"] = serde_json::json!("other.reset_report");
    assert!(serde_json::from_value::<ResetReport>(foreign_reset).is_err());
    assert_eq!(RunMode::Live.as_str(), "live");
    assert_eq!(RunMode::DryRun.as_str(), "dry_run");
    let tokens = [
        RunOutcome::Initialized,
        RunOutcome::Changed,
        RunOutcome::Unchanged,
        RunOutcome::SkippedDisabled,
        RunOutcome::RefusedContractDigest,
        RunOutcome::AcquisitionFailed,
        RunOutcome::ValueUnparseable,
        RunOutcome::ConfigInvalid,
        RunOutcome::TargetUnavailable,
        RunOutcome::StateInvalid,
        RunOutcome::LockUnavailable,
        RunOutcome::FetchFailed,
        RunOutcome::PersistFailed,
        RunOutcome::IntegrationFault,
    ]
    .map(RunOutcome::as_str);
    assert_eq!(tokens.len(), 14);
    assert_eq!(
        [
            StatusKind::Pending,
            StatusKind::Ready,
            StatusKind::InvalidConfig,
            StatusKind::UnavailableTarget,
            StatusKind::InvalidState,
        ]
        .map(StatusKind::as_str),
        [
            "pending",
            "ready",
            "invalid_config",
            "unavailable_target",
            "invalid_state",
        ]
    );
    assert_eq!(
        [
            DiagnosticKind::Contract,
            DiagnosticKind::Io,
            DiagnosticKind::Json,
            DiagnosticKind::Htmlcut,
            DiagnosticKind::Toml,
            DiagnosticKind::ValueUnparseable,
            DiagnosticKind::PolicyInvariant,
            DiagnosticKind::Delivery,
        ]
        .map(DiagnosticKind::as_str),
        [
            "contract",
            "io",
            "json",
            "htmlcut",
            "toml",
            "value_unparseable",
            "policy_invariant",
            "delivery",
        ]
    );
}

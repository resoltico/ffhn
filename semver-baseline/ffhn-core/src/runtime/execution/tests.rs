use super::super::acquire::MeasurementAcquisitionFailure;
use super::super::storage::load_state;
use super::*;
use crate::model::{integration_detail, io_detail, plain_detail};
use crate::{
    CoreError, DiagnosticDetail, DiagnosticKind, DiagnosticOperation, IntegrationFaultCode,
    IoErrorClass, Observation, OnRunEventCause, PermanentErrorCode, RunMode, RunOutcome,
    SourceSuspectReason, StagedEventEligibility, StagedPolicyRun, StateDocument, TargetDocument,
    TargetPaths,
};

mod permanent_preflight;
mod policy_invariant_lifecycle;

fn detail(kind: DiagnosticKind, message: impl Into<String>) -> DiagnosticDetail {
    match kind {
        DiagnosticKind::Io => io_detail(
            IoErrorClass::Other,
            DiagnosticOperation::TargetValidation,
            message,
            None,
        ),
        _ => plain_detail(kind, DiagnosticOperation::TargetValidation, message, None),
    }
}

fn target() -> TargetDocument {
    let source_path = crate::test_support::absolute_file_path("source.json");
    let target: TargetDocument = toml::from_str(&format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 1\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
        ))
        .expect("target TOML");
    target.validate().expect("valid target");
    target
}

fn paths() -> (tempfile::TempDir, TargetPaths) {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let paths = TargetPaths::try_new(temporary.path(), "demo").expect("target paths");
    (temporary, paths)
}

fn empty_staged(target: &TargetDocument) -> StagedRun {
    StagedRun::from_eligibilities(
        StateDocument::new(
            crate::TargetId::new("demo").expect("target id"),
            target.contract_digest_sha256().expect("digest"),
        ),
        Vec::new(),
    )
}

fn invalid_fetch_target() -> TargetDocument {
    let mut value = serde_json::to_value(target()).expect("target JSON");
    value["fetch"]["max_bytes"] = serde_json::json!(0);
    serde_json::from_value(value).expect("structurally valid target")
}

fn state_for(target: &TargetDocument) -> StateDocument {
    StateDocument::new(
        crate::TargetId::new("demo").expect("target id"),
        target.contract_digest_sha256().expect("digest"),
    )
}

fn fault_routed_target() -> TargetDocument {
    let (program, args) = super::super::delivery::process::test_process_command("fail", None);
    let args = toml::Value::Array(args.into_iter().map(toml::Value::String).collect()).to_string();
    let program = toml::Value::String(program.display().to_string()).to_string();
    let source_path = crate::test_support::absolute_file_path("source.json");
    let target: TargetDocument = toml::from_str(&format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 1\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n\n[[routes]]\nroute_id = \"run\"\nroute_family = \"on_run\"\n\n[routes.adapter]\nkind = \"process_stdin\"\nprogram = {program}\nargs = {args}\ntimeout_ms = 1000\n"
        ))
        .expect("target TOML");
    target.validate().expect("valid target");
    target
}

#[test]
fn internal_staging_guards_reject_mismatched_policy_shapes() {
    let staged = StagedPolicyRun::SourceSuspect {
        reason_class: SourceSuspectReason::FetchFailed,
        event_eligibilities: Vec::new(),
    };
    assert!(valid_condition_evaluations(&staged).is_err());
    let state = StateDocument::new(
        crate::TargetId::new("demo").expect("target id"),
        "a".repeat(64),
    );
    let mismatched = StagedRun::from_eligibilities(
        state,
        vec![StagedEventEligibility::OnRun {
            cause: OnRunEventCause::Reset,
        }],
    );
    assert!(ensure_failure_event_staging(&mismatched, None).is_err());
    assert!(ensure_failure_event_staging(&mismatched, Some(OnRunEventCause::Reset)).is_ok());
    assert!(ensure_failure_event_staging(&mismatched, Some(OnRunEventCause::Initialized)).is_err());
}

#[test]
fn staging_boundaries_surface_invalid_inputs_before_any_commit() {
    let target = target();
    let observation = target
        .parse_json_scalar_token("10".to_owned())
        .expect("observation");
    assert!(
        stage_valid_observation_run(
            &invalid_fetch_target(),
            &state_for(&target),
            observation,
            "2026-07-15T00:00:00Z",
        )
        .is_err()
    );

    let mut state = state_for(&target);
    let initial = target
        .parse_json_scalar_token("10".to_owned())
        .expect("initial observation");
    state
        .apply_valid_observation(&target, initial, &[], "2026-07-15T00:00:00Z")
        .expect("initial state");
    let mut wire = serde_json::to_value(&state).expect("state JSON");
    wire["observation_seq"] = serde_json::json!(u64::MAX);
    let exhausted = StateDocument::from_unvalidated_wire_for_test(wire);
    let observation = target
        .parse_json_scalar_token("11".to_owned())
        .expect("next observation");
    assert!(
        stage_valid_observation_run(&target, &exhausted, observation, "2026-07-15T00:00:01Z",)
            .is_err()
    );
}

#[test]
fn failure_staging_surfaces_state_overflow_and_invalid_target_contracts() {
    let (temporary, paths) = paths();
    let target = target();
    let mut wire = serde_json::to_value(state_for(&target)).expect("state JSON");
    wire["source_health"] = serde_json::json!({
        "state": "suspect",
        "reason_class": "fetch_failed",
        "consecutive_unresolved": u32::MAX,
        "first_unresolved_at": "2026-07-15T00:00:00Z",
        "last_details": {"kind": "io", "operation": "http_fetch", "message": "prior failure", "io_error_class": "connection_refused"},
    });
    let overflowed = StateDocument::from_unvalidated_wire_for_test(wire);
    let run = StatefulRun {
        paths: &paths,
        target: &target,
        mode: RunMode::Live,
        started: "2026-07-15T00:00:01Z".to_owned(),
        digest: target.contract_digest_sha256().expect("digest"),
        previous: None,
        lifecycle_before: None,
    };
    assert!(
        finish_source_suspect(
            run,
            &overflowed,
            RunOutcome::FetchFailed,
            SourceSuspectReason::FetchFailed,
            detail(DiagnosticKind::Io, "current failure"),
        )
        .is_err()
    );

    let invalid = invalid_fetch_target();
    let state = state_for(&invalid);
    let run = StatefulRun {
        paths: &paths,
        target: &invalid,
        mode: RunMode::Live,
        started: "2026-07-15T00:00:00Z".to_owned(),
        digest: invalid.contract_digest_sha256().expect("digest"),
        previous: None,
        lifecycle_before: None,
    };
    assert!(
        finish_source_suspect(
            run,
            &state,
            RunOutcome::FetchFailed,
            SourceSuspectReason::FetchFailed,
            detail(DiagnosticKind::Io, "source failed"),
        )
        .is_err()
    );

    let state = state_for(&invalid);
    let run = StatefulRun {
        paths: &paths,
        target: &invalid,
        mode: RunMode::Live,
        started: "2026-07-15T00:00:00Z".to_owned(),
        digest: invalid.contract_digest_sha256().expect("digest"),
        previous: None,
        lifecycle_before: None,
    };
    assert!(
        finish_permanent_error(
            run,
            &state,
            PermanentErrorCode::InvalidJsonPointer,
            detail(DiagnosticKind::Contract, "invalid JSON pointer"),
        )
        .is_err()
    );
    drop(temporary);
}

#[test]
fn precommit_failures_preserve_the_nonpersistent_commit_boundary() {
    let target = target();
    let (_temporary, paths) = paths();

    let staged = empty_staged(&target);
    let mut unavailable_clock = || Err(CoreError::internal("clock unavailable"));
    let failure = commit_staged_run_with_clock(
        &paths,
        &target,
        RunMode::Live,
        &staged,
        target.contract_digest_sha256().expect("digest").as_str(),
        &mut unavailable_clock,
    )
    .err()
    .expect("clock failure must prevent a commit");
    assert!(!failure.persisted);
    assert!(failure.error.to_string().contains("clock unavailable"));

    let staged = StagedRun::from_eligibilities(
        StateDocument::new(
            crate::TargetId::new("demo").expect("target id"),
            target.contract_digest_sha256().expect("digest"),
        ),
        vec![StagedEventEligibility::OnCondition {
            condition_id: "missing".parse().expect("condition id"),
        }],
    );
    let mut clock = || Ok("2026-07-15T00:00:00Z".to_owned());
    let failure = commit_staged_run_with_clock(
        &paths,
        &target,
        RunMode::Live,
        &staged,
        target.contract_digest_sha256().expect("digest").as_str(),
        &mut clock,
    )
    .err()
    .expect("materialization failure must prevent a commit");
    assert!(!failure.persisted);
    assert!(failure.error.to_string().contains("condition absent"));

    let staged = empty_staged(&target);
    let mut invalid_clock = || Ok("not-a-timestamp".to_owned());
    let failure = commit_staged_run_with_clock(
        &paths,
        &target,
        RunMode::Live,
        &staged,
        target.contract_digest_sha256().expect("digest").as_str(),
        &mut invalid_clock,
    )
    .err()
    .expect("outbox enqueue failure must prevent a commit");
    assert!(!failure.persisted);
    assert!(failure.error.to_string().contains("outbox timestamp"));
}

#[test]
fn durable_commit_failure_cannot_enter_delivery() {
    let target = target();
    let (_temporary, paths) = paths();
    let staged = StagedRun::from_eligibilities(
        StateDocument::new(
            crate::TargetId::new("demo").expect("target id"),
            target.contract_digest_sha256().expect("digest"),
        ),
        vec![StagedEventEligibility::OnRun {
            cause: OnRunEventCause::Reset,
        }],
    );
    let mut clock = || Ok("2026-07-15T00:00:00Z".to_owned());
    let mut failed_persist =
        |_state: &StateDocument| Err(CoreError::internal("durability synchronization refused"));

    let failure = commit_staged_run_with_clock_and_persist(
        &paths,
        &target,
        RunMode::Live,
        &staged,
        target.contract_digest_sha256().expect("digest").as_str(),
        &mut clock,
        &mut failed_persist,
    )
    .err()
    .expect("durability failure must prevent delivery");

    assert!(!failure.persisted);
    assert!(
        failure
            .error
            .to_string()
            .contains("durability synchronization refused")
    );
    assert!(!paths.state_file().exists());
}

#[test]
fn integration_fault_dispatch_and_failure_paths_are_fail_closed() {
    let target = target();
    let digest = target.contract_digest_sha256().expect("digest");
    let (temporary, successful_paths) = paths();

    let missing_code = finish_integration_fault(
        StatefulRun {
            paths: &successful_paths,
            target: &target,
            mode: RunMode::Live,
            started: "2026-07-15T00:00:00Z".to_owned(),
            digest: digest.clone(),
            previous: None,
            lifecycle_before: None,
        },
        &state_for(&target),
        detail(DiagnosticKind::Htmlcut, "missing code"),
    )
    .expect_err("an integration failure without its closed code must be refused");
    assert!(
        missing_code
            .to_string()
            .contains("must carry integration_fault_code")
    );

    std::fs::create_dir_all(successful_paths.target_dir()).expect("target directory");
    let report = finish_measurement_acquisition_failure(
        StatefulRun {
            paths: &successful_paths,
            target: &target,
            mode: RunMode::Live,
            started: "2026-07-15T00:00:01Z".to_owned(),
            digest: digest.clone(),
            previous: None,
            lifecycle_before: None,
        },
        &state_for(&target),
        MeasurementAcquisitionFailure::Integration {
            detail: integration_detail(
                DiagnosticKind::Htmlcut,
                DiagnosticOperation::HtmlExtraction,
                "HTMLCut returned an internal error",
                IntegrationFaultCode::HtmlcutInternalError,
            ),
        },
    )
    .expect("integration dispatch");
    assert_eq!(report.outcome(), RunOutcome::IntegrationFault);
    assert!(report.state_persisted());

    let (failing_temporary, failing_paths) = paths();
    std::fs::create_dir_all(failing_paths.storage_root()).expect("storage root");
    std::fs::create_dir(failing_paths.state_file()).expect("state-file directory");
    let report = finish_integration_fault(
        StatefulRun {
            paths: &failing_paths,
            target: &target,
            mode: RunMode::Live,
            started: "2026-07-15T00:00:02Z".to_owned(),
            digest,
            previous: None,
            lifecycle_before: None,
        },
        &state_for(&target),
        integration_detail(
            DiagnosticKind::Htmlcut,
            DiagnosticOperation::HtmlExtraction,
            "boundary invariant",
            IntegrationFaultCode::FfhnBoundaryInvariantViolation,
        ),
    )
    .expect("durable integration failure is reported");
    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
    assert!(!report.state_persisted());

    drop(failing_temporary);
    drop(temporary);
}

#[test]
fn policy_invariant_fault_preserves_the_prior_baseline_and_retains_the_new_observation() {
    let target = target();
    let digest = target.contract_digest_sha256().expect("digest");
    let (temporary, paths) = paths();
    std::fs::create_dir_all(paths.target_dir()).expect("target directory");
    let mut state = state_for(&target);
    let accepted = target
        .parse_json_scalar_token("10".to_owned())
        .expect("accepted observation");
    state
        .apply_valid_observation(&target, accepted, &[], "2026-07-15T00:00:00Z")
        .expect("seed state");
    let observed = target
        .parse_json_scalar_token("20".to_owned())
        .expect("new observation");

    let report = finish_policy_invariant(
        StatefulRun {
            paths: &paths,
            target: &target,
            mode: RunMode::Live,
            started: "2026-07-15T00:00:01Z".to_owned(),
            digest,
            previous: Some("10".to_owned()),
            lifecycle_before: None,
        },
        &state,
        observed,
        "documented width proof failed".to_owned(),
    )
    .expect("target-scoped policy fault report");

    assert_eq!(report.outcome(), RunOutcome::IntegrationFault);
    assert!(report.state_persisted());
    assert_eq!(
        report.observation().map(Observation::canonical_value),
        Some("20")
    );
    let detail = report.error_detail().expect("fault detail");
    assert_eq!(detail.kind(), DiagnosticKind::PolicyInvariant);
    assert_eq!(
        detail.integration_fault_code(),
        Some(IntegrationFaultCode::FfhnPolicyInvariantViolation)
    );

    let persisted = load_state(&paths)
        .expect("read persisted policy fault state")
        .expect("persisted policy fault state");
    assert_eq!(
        persisted
            .accepted_observation()
            .map(Observation::canonical_value),
        Some("10")
    );
    assert_eq!(persisted.observation_seq(), 1);
    assert!(
        persisted
            .integration_fault_episode_started_at(
                IntegrationFaultCode::FfhnPolicyInvariantViolation
            )
            .is_some()
    );

    drop(temporary);
}

#[test]
fn integration_faults_persist_one_immediate_episode_event_and_dry_runs_leave_no_trace() {
    let target = fault_routed_target();
    let (temporary, live_paths) = paths();
    std::fs::create_dir_all(live_paths.target_dir()).expect("target directory");
    let digest = target.contract_digest_sha256().expect("digest");
    let detail = integration_detail(
        DiagnosticKind::Htmlcut,
        DiagnosticOperation::HtmlExtraction,
        "HTMLCut returned InternalError",
        IntegrationFaultCode::HtmlcutInternalError,
    );

    let first = finish_integration_fault(
        StatefulRun {
            paths: &live_paths,
            target: &target,
            mode: RunMode::Live,
            started: "2026-07-15T00:00:00Z".to_owned(),
            digest: digest.clone(),
            previous: None,
            lifecycle_before: None,
        },
        &state_for(&target),
        detail.clone(),
    )
    .expect("first integration fault");
    assert_eq!(first.outcome(), RunOutcome::IntegrationFault);
    let first_report = serde_json::to_value(&first).expect("run report JSON");
    assert_eq!(first_report["schema_name"], "ffhn.run_report");
    assert_eq!(first_report["schema_version"], 17);
    assert_eq!(first_report["outcome"], "integration_fault");
    assert_eq!(
        first_report["error_detail"]["integration_fault_code"],
        "htmlcut_internal_error"
    );
    assert!(first.state_persisted());
    assert_eq!(first.delivery_outcomes().len(), 1);
    assert_eq!(
        first.delivery_outcomes()[0].status(),
        crate::DeliveryStatus::RetryScheduled
    );

    let first_state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(live_paths.state_file()).expect("state bytes"))
            .expect("state JSON");
    assert_eq!(first_state["observation_seq"], 0);
    assert_eq!(first_state["source_health"]["state"], "healthy");
    assert_eq!(
        first_state["integration_fault_episode"]["integration_fault_code"],
        "htmlcut_internal_error"
    );
    assert_eq!(first_state["outbox"].as_array().expect("outbox").len(), 1);
    let first_event_id = first_state["outbox"][0]["event_id"].clone();
    let first_payload = &first_state["outbox"][0]["immutable_payload"];

    let stored: StateDocument = serde_json::from_value(first_state.clone()).expect("state");
    let second = finish_integration_fault(
        StatefulRun {
            paths: &live_paths,
            target: &target,
            mode: RunMode::Live,
            started: "2026-07-15T00:00:01Z".to_owned(),
            digest: digest.clone(),
            previous: None,
            lifecycle_before: None,
        },
        &stored,
        detail.clone(),
    )
    .expect("continued integration fault");
    assert_eq!(second.outcome(), RunOutcome::IntegrationFault);
    let second_state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(live_paths.state_file()).expect("state bytes"))
            .expect("state JSON");
    assert_eq!(second_state["outbox"].as_array().expect("outbox").len(), 1);
    assert_eq!(second_state["outbox"][0]["event_id"], first_event_id);
    assert_eq!(
        second_state["outbox"][0]["immutable_payload"],
        *first_payload
    );

    let stored: StateDocument = serde_json::from_value(second_state).expect("state");
    let changed = finish_integration_fault(
        StatefulRun {
            paths: &live_paths,
            target: &target,
            mode: RunMode::Live,
            started: "2026-07-15T00:00:02Z".to_owned(),
            digest: digest.clone(),
            previous: None,
            lifecycle_before: None,
        },
        &stored,
        integration_detail(
            DiagnosticKind::Htmlcut,
            DiagnosticOperation::HtmlExtraction,
            "FFHN detected an adapter-boundary invariant violation",
            IntegrationFaultCode::FfhnBoundaryInvariantViolation,
        ),
    )
    .expect("changed integration fault");
    assert_eq!(changed.outcome(), RunOutcome::IntegrationFault);
    let changed_state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(live_paths.state_file()).expect("state bytes"))
            .expect("state JSON");
    assert_eq!(changed_state["outbox"].as_array().expect("outbox").len(), 2);
    assert_eq!(
        changed_state["integration_fault_episode"]["integration_fault_code"],
        "ffhn_boundary_invariant_violation"
    );

    let (dry_temporary, dry_paths) = paths();
    std::fs::create_dir_all(dry_paths.target_dir()).expect("dry target directory");
    let dry_run = finish_integration_fault(
        StatefulRun {
            paths: &dry_paths,
            target: &target,
            mode: RunMode::DryRun,
            started: "2026-07-15T00:00:03Z".to_owned(),
            digest,
            previous: None,
            lifecycle_before: None,
        },
        &state_for(&target),
        integration_detail(
            DiagnosticKind::Htmlcut,
            DiagnosticOperation::HtmlExtraction,
            "dry run",
            IntegrationFaultCode::HtmlcutInternalError,
        ),
    )
    .expect("dry integration fault");
    assert_eq!(dry_run.outcome(), RunOutcome::IntegrationFault);
    assert!(!dry_run.state_persisted());
    assert!(dry_run.delivery_outcomes().is_empty());
    assert!(!dry_paths.storage_root().exists());

    drop(dry_temporary);
    drop(temporary);
}

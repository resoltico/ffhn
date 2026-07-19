use super::super::super::report::detail_from_error_for_operation;
use super::*;

fn policy_invariant_stager(
    _: &TargetDocument,
    _: &StateDocument,
    _: Observation,
    _: &str,
) -> Result<StagedRun, CoreError> {
    Err(CoreError::policy_invariant("fixed-width proof regression"))
}

fn unexpected_stager(
    _: &TargetDocument,
    _: &StateDocument,
    _: Observation,
    _: &str,
) -> Result<StagedRun, CoreError> {
    Err(CoreError::internal("unexpected policy staging failure"))
}

#[test]
fn coordinator_isolates_a_policy_proof_failure_to_its_target_lifecycle() {
    let (temporary, paths) = paths();
    let source = paths.target_dir().join("source.json");
    std::fs::create_dir_all(paths.target_dir()).expect("create target directory");
    std::fs::write(&source, r#"{"value":20}"#).expect("write source");
    std::fs::write(
        paths.target_file(),
        format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 1\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n"
        ),
    )
    .expect("write target");

    let report = run_once_with_stager(&paths, RunMode::Live, policy_invariant_stager)
        .expect("policy invariant becomes a target report");
    assert_eq!(report.outcome(), RunOutcome::IntegrationFault);
    assert!(report.state_persisted());
    assert_eq!(
        report
            .lifecycle()
            .after()
            .and_then(|snapshot| snapshot.integration_fault_episode())
            .map(|episode| episode.code()),
        Some(IntegrationFaultCode::FfhnPolicyInvariantViolation)
    );
    assert_eq!(
        report.error_detail().map(DiagnosticDetail::kind),
        Some(DiagnosticKind::PolicyInvariant)
    );
    assert_eq!(
        report
            .error_detail()
            .and_then(DiagnosticDetail::integration_fault_code),
        Some(IntegrationFaultCode::FfhnPolicyInvariantViolation)
    );
    let status = crate::status(&paths).expect("integration-fault status");
    assert_eq!(status.kind(), crate::StatusKind::Pending);
    let status_episode = status
        .lifecycle()
        .and_then(|snapshot| snapshot.integration_fault_episode())
        .expect("status exposes the complete durable integration-fault episode");
    assert_eq!(
        status_episode.code(),
        IntegrationFaultCode::FfhnPolicyInvariantViolation
    );
    let status_first_seen_at = status_episode.first_seen_at().to_owned();
    let state = load_state(&paths)
        .expect("read persisted target state")
        .expect("persisted target state");
    assert_eq!(state.accepted_observation(), None);
    let persisted_first_seen_at = state
        .integration_fault_episode_started_at(IntegrationFaultCode::FfhnPolicyInvariantViolation)
        .expect("persisted integration-fault episode");
    assert_eq!(
        status_first_seen_at, persisted_first_seen_at,
        "status exposes the exact durable integration-fault episode timestamp"
    );

    let error = run_once_with_stager(&paths, RunMode::DryRun, unexpected_stager)
        .expect_err("only proof failures become target-scoped reports");
    assert_eq!(
        error.to_string(),
        "internal error: unexpected policy staging failure"
    );

    drop(temporary);
}

#[test]
fn structured_policy_invariant_diagnostics_keep_category_and_payload_separate() {
    let detail = detail_from_error_for_operation(
        &CoreError::policy_invariant("fixed-width proof regression"),
        DiagnosticOperation::PolicyEvaluation,
        None,
    );
    assert_eq!(detail.kind(), DiagnosticKind::PolicyInvariant);
    assert_eq!(detail.message(), "fixed-width proof regression");
    assert_eq!(
        detail.integration_fault_code(),
        Some(IntegrationFaultCode::FfhnPolicyInvariantViolation)
    );
}

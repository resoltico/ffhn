//! Permanent-projection preflight failure reporting.

use super::*;

fn permanent_error_contract_failure(
    _: &TargetDocument,
) -> Result<Option<crate::model::PermanentTargetError>, CoreError> {
    Err(CoreError::contract(
        "HTMLCut returned permanent-preflight evidence outside FFHN's pinned contract",
    ))
}

#[test]
fn fallible_permanent_preflight_becomes_a_target_scoped_validation_report() {
    let (temporary, paths) = paths();
    let target = target();
    std::fs::create_dir_all(paths.target_dir()).expect("create target directory");
    std::fs::write(
        paths.target_file(),
        toml::to_string(&target).expect("serialize valid target"),
    )
    .expect("write target");

    let report = run_once_with_stager_and_permanent_error_resolver(
        &paths,
        RunMode::DryRun,
        stage_valid_observation_run,
        permanent_error_contract_failure,
    )
    .expect("fallible permanent preflight is reported rather than propagated");
    assert_eq!(report.outcome(), RunOutcome::ConfigInvalid);
    assert!(!report.state_persisted());
    assert_eq!(
        report.error_detail().map(|detail| detail.operation()),
        Some(DiagnosticOperation::TargetValidation)
    );
    drop(temporary);
}

use super::support::*;

#[test]
fn lifecycle_facet_keeps_failure_state_visible_without_creating_policy_events() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), "not JSON").expect("malformed source");

    let first = run_once(&paths).expect("first source failure");
    assert_eq!(first.lifecycle().before(), None);
    assert_eq!(
        first
            .lifecycle()
            .after()
            .expect("staged first lifecycle")
            .source_health()
            .consecutive_unresolved(),
        1
    );

    let second = run_once(&paths).expect("second source failure");
    assert_eq!(
        second
            .lifecycle()
            .before()
            .expect("durable lifecycle before second failure")
            .source_health()
            .consecutive_unresolved(),
        1
    );
    assert_eq!(
        second
            .lifecycle()
            .after()
            .expect("staged second lifecycle")
            .source_health()
            .consecutive_unresolved(),
        2
    );
    let durable_status = status(&paths).expect("source-health status");
    assert_eq!(durable_status.kind(), StatusKind::Pending);
    assert_eq!(
        durable_status
            .lifecycle()
            .expect("durable source-health lifecycle")
            .source_health()
            .consecutive_unresolved(),
        2
    );
    assert!(second.policy_evaluation().event_eligibilities().is_empty());
}

#[test]
fn lifecycle_before_after_and_persistence_are_independent_facts() {
    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value": 7}"#).expect("source");
    run_once(&paths).expect("seed durable state");

    let disabled = fs::read_to_string(paths.target_file())
        .expect("target")
        .replace("enabled = true", "enabled = false");
    fs::write(paths.target_file(), disabled).expect("disable target");
    let disabled = run_once(&paths).expect("disabled run");
    assert_eq!(disabled.outcome(), RunOutcome::SkippedDisabled);
    assert!(disabled.lifecycle().before().is_some());
    assert_eq!(disabled.lifecycle().after(), None);
    assert!(!disabled.state_persisted());

    let enabled = fs::read_to_string(paths.target_file())
        .expect("disabled target")
        .replace("enabled = false", "enabled = true");
    fs::write(paths.target_file(), enabled).expect("enable target");
    fs::write(paths.target_dir().join("source.json"), "not JSON").expect("malformed source");
    let dry_run = run_once_with_mode(&paths, RunMode::DryRun).expect("dry-run failure");
    assert!(dry_run.lifecycle().before().is_some());
    assert!(dry_run.lifecycle().after().is_some());
    assert!(!dry_run.state_persisted());
    assert_eq!(
        dry_run
            .lifecycle()
            .after()
            .expect("staged dry-run lifecycle")
            .source_health()
            .consecutive_unresolved(),
        1
    );
}

#[test]
fn status_exposes_a_verified_permanent_episode_but_never_stale_state() {
    let (_temporary, permanent_paths) = fixture_paths();
    write_target(&permanent_paths, "integer", "", "not-a-json-pointer");
    let status_without_state = status(&permanent_paths).expect("projection-invalid pending status");
    assert_eq!(status_without_state.kind(), StatusKind::InvalidConfig);
    assert_eq!(status_without_state.lifecycle(), None);
    let permanent = run_once(&permanent_paths).expect("permanent-error run");
    assert_eq!(permanent.outcome(), RunOutcome::ConfigInvalid);
    assert_eq!(
        permanent
            .lifecycle()
            .after()
            .and_then(|snapshot| snapshot.permanent_error_episode())
            .map(|episode| episode.error_code()),
        Some(PermanentErrorCode::InvalidJsonPointer)
    );
    let permanent_status = status(&permanent_paths).expect("projection-invalid status");
    assert_eq!(permanent_status.kind(), StatusKind::InvalidConfig);
    assert_eq!(
        permanent_status
            .lifecycle()
            .and_then(|snapshot| snapshot.permanent_error_episode())
            .map(|episode| episode.error_code()),
        Some(PermanentErrorCode::InvalidJsonPointer)
    );

    let (_temporary, stale_paths) = fixture_paths();
    write_target(&stale_paths, "integer", "", "/value");
    fs::write(
        stale_paths.target_dir().join("source.json"),
        r#"{"value": 7}"#,
    )
    .expect("source");
    run_once(&stale_paths).expect("seed state");
    write_target(&stale_paths, "integer", "", "/other");
    let stale = run_once(&stale_paths).expect("digest refusal");
    assert_eq!(stale.outcome(), RunOutcome::RefusedContractDigest);
    assert_eq!(stale.lifecycle().before(), None);
    assert_eq!(stale.lifecycle().after(), None);
    assert_eq!(
        status(&stale_paths).expect("stale status").lifecycle(),
        None
    );

    let mut retired_schema: serde_json::Value =
        serde_json::from_slice(&fs::read(stale_paths.state_file()).expect("stale state"))
            .expect("stale state JSON");
    retired_schema["schema_version"] = serde_json::json!(14);
    fs::write(
        stale_paths.state_file(),
        serde_json::to_vec(&retired_schema).expect("retired state JSON"),
    )
    .expect("write retired state");
    let retired_run = run_once(&stale_paths).expect("retired-schema run report");
    assert_eq!(retired_run.outcome(), RunOutcome::StateInvalid);
    assert_eq!(retired_run.lifecycle().before(), None);
    assert_eq!(retired_run.lifecycle().after(), None);
    assert_eq!(
        status(&stale_paths)
            .expect("retired-schema status")
            .lifecycle(),
        None
    );
}

#[cfg(unix)]
#[test]
fn failed_durable_commit_still_reports_the_staged_lifecycle() {
    use std::os::unix::fs::PermissionsExt;

    let (_temporary, paths) = fixture_paths();
    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value": 7}"#).expect("source");
    run_once(&paths).expect("seed durable state");
    fs::write(paths.target_dir().join("source.json"), "not JSON").expect("malformed source");
    fs::set_permissions(paths.storage_root(), fs::Permissions::from_mode(0o500))
        .expect("make storage read-only");
    let report = run_once(&paths).expect("persist-failed report");
    fs::set_permissions(paths.storage_root(), fs::Permissions::from_mode(0o700))
        .expect("restore storage permissions");

    assert_eq!(report.outcome(), RunOutcome::PersistFailed);
    assert!(report.lifecycle().before().is_some());
    assert_eq!(
        report
            .lifecycle()
            .after()
            .expect("staged lifecycle")
            .source_health()
            .consecutive_unresolved(),
        1
    );
    assert!(!report.state_persisted());
}

use super::support::*;

#[test]
fn target_load_lock_and_invalid_storage_failures_are_reported_without_hidden_fallbacks() {
    let (_temporary, paths) = fixture_paths();
    assert_eq!(
        run_once(&paths).expect("missing target report").outcome(),
        RunOutcome::TargetUnavailable
    );
    assert_eq!(
        status(&paths).expect("missing target status").kind(),
        StatusKind::UnavailableTarget
    );
    fs::create_dir_all(paths.target_dir()).expect("target directory");
    fs::write(paths.target_file(), "not TOML").expect("invalid target");
    assert_eq!(
        run_once(&paths).expect("invalid target report").outcome(),
        RunOutcome::ConfigInvalid
    );
    assert_eq!(
        status(&paths).expect("invalid target status").kind(),
        StatusKind::InvalidConfig
    );

    write_target(&paths, "integer", "", "/value");
    let target_text = fs::read_to_string(paths.target_file()).expect("target text");
    fs::write(
        paths.target_file(),
        target_text.replacen("pointer = \"/value\"", "pointer = \"/bad~2\"", 1),
    )
    .expect("invalid pointer target");
    let invalid_pointer = run_once(&paths).expect("invalid pointer report");
    let detail = invalid_pointer
        .error_detail()
        .expect("target validation detail");
    assert_eq!(detail.kind(), DiagnosticKind::Contract);
    assert_eq!(detail.operation(), DiagnosticOperation::TargetValidation);
    assert!(!detail.message().starts_with("contract error:"));

    write_target(&paths, "integer", "", "/value");
    fs::write(paths.target_dir().join("source.json"), r#"{"value":7}"#).expect("source");
    let lock = lock_exclusive(&paths).expect("hold lock");
    assert_eq!(
        run_once(&paths).expect("lock outcome").outcome(),
        RunOutcome::LockUnavailable
    );
    assert!(reset(&paths).is_err());
    drop(lock);

    fs::remove_dir_all(paths.storage_root()).expect("remove lock-created storage root");
    fs::write(paths.storage_root(), "not a directory").expect("storage blocker");
    assert_eq!(
        run_once(&paths).expect("invalid storage outcome").outcome(),
        RunOutcome::StateInvalid
    );
    assert_eq!(
        status(&paths).expect("invalid storage status").kind(),
        StatusKind::InvalidState
    );
}

#[test]
fn invalid_closed_target_enum_reports_its_exact_safe_field_and_value() {
    let (_temporary, paths) = fixture_paths();
    fs::create_dir_all(paths.target_dir()).expect("target directory");
    write_html_target(&paths, "html_text", "h1", None, "text", "");
    let target = fs::read_to_string(paths.target_file()).expect("target TOML");
    fs::write(
        paths.target_file(),
        target.replace(
            "whitespace = \"rendered\"",
            "whitespace = \"totally-bogus\"",
        ),
    )
    .expect("invalid target TOML");

    let report = run_once(&paths).expect("invalid target report");
    assert_eq!(report.outcome(), RunOutcome::ConfigInvalid);
    let detail = report.error_detail().expect("target-load detail");
    assert_eq!(detail.kind(), DiagnosticKind::Toml);
    assert_eq!(detail.operation(), DiagnosticOperation::TargetLoad);
    assert_eq!(
        detail.message(),
        "target field \"projection.selection.rendering.whitespace\" has unsupported value \"totally-bogus\""
    );
    let message = detail.message().to_owned();
    let wire = serde_json::to_value(report).expect("run report JSON");
    assert_eq!(wire["error_detail"]["message"], message);
}

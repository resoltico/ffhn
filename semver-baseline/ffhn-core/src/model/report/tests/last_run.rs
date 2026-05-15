use super::*;

#[test]
fn last_run_snapshot_accessors_expose_the_public_contract() {
    let snapshot = LastRunSnapshot::new(
        RunReport {
            persist: persist_section(
                2,
                PersistWriteStatus::Written,
                PersistWriteStatus::NotAttempted,
            ),
            ..valid_run_report()
        }
        .with_digest()
        .expect("digest"),
    )
    .expect("snapshot");

    assert_eq!(snapshot.schema_name(), LAST_RUN_SNAPSHOT_SCHEMA_NAME);
    assert_eq!(snapshot.schema_version(), LAST_RUN_SNAPSHOT_SCHEMA_VERSION);
    assert_eq!(snapshot.run_report().schema_name(), RUN_REPORT_SCHEMA_NAME);
    assert_eq!(snapshot.run_report().run_outcome(), RunOutcome::Changed);
    assert!(!snapshot.run_report().persist().wrote_last_run());
}

#[test]
fn last_run_snapshot_validation_accepts_and_round_trips_live_prepublication_reports() {
    let snapshot = LastRunSnapshot::new(
        RunReport {
            persist: persist_section(
                2,
                PersistWriteStatus::Written,
                PersistWriteStatus::NotAttempted,
            ),
            ..valid_run_report()
        }
        .with_digest()
        .expect("digest"),
    )
    .expect("snapshot");
    snapshot.validate().expect("validated snapshot");

    let json = serde_json::to_string(&snapshot).expect("snapshot json");
    let parsed: LastRunSnapshot = serde_json::from_str(&json).expect("parse snapshot");
    assert_eq!(parsed, snapshot);
}

#[test]
fn last_run_snapshot_validation_rejects_non_live_and_postpublication_reports() {
    let dry_run = LastRunSnapshot::new(
        RunReport {
            run_mode: RunMode::DryRun,
            persist: persist_section(
                0,
                PersistWriteStatus::NotAttempted,
                PersistWriteStatus::NotAttempted,
            ),
            notifications: Vec::new(),
            ..valid_run_report()
        }
        .with_digest()
        .expect("dry-run digest"),
    );
    assert!(dry_run.is_err());

    let written = LastRunSnapshot::new(valid_run_report());
    assert!(written.is_err());
}

#[test]
fn last_run_snapshot_deserialization_rejects_nonzero_last_run_write_duration() {
    let snapshot = LastRunSnapshot::new(
        RunReport {
            persist: persist_section(
                2,
                PersistWriteStatus::Written,
                PersistWriteStatus::NotAttempted,
            ),
            ..valid_run_report()
        }
        .with_digest()
        .expect("digest"),
    )
    .expect("snapshot");
    let mut invalid_report = snapshot.run_report().clone();
    invalid_report.persist = RunPersistSection::from_writes(
        2,
        PersistWriteStatus::Written,
        1,
        PersistWriteStatus::NotAttempted,
    );
    let invalid_report = invalid_report.with_digest().expect("invalid report digest");
    let json = serde_json::json!({
        "schema_name": LAST_RUN_SNAPSHOT_SCHEMA_NAME,
        "schema_version": LAST_RUN_SNAPSHOT_SCHEMA_VERSION,
        "run_report": invalid_report,
    });

    let error = serde_json::from_value::<LastRunSnapshot>(json)
        .expect_err("nonzero last-run write duration must be rejected");
    assert!(error.to_string().contains(
        "run_report.persist.last_run_write_duration_ms must be zero when the persist step was not attempted"
    ));
}

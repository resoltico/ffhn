use super::*;

#[test]
fn status_report_validation_enforces_state_phase_rules() {
    StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: "demo".to_owned(),
        target_status: TargetStatus::Invalid,
        reason_code: ReasonCode::ConfigInvalid,
        state_phase: None,
        artifacts: ArtifactStatus {
            current_valid: false,
            previous_valid: false,
        },
        current_snapshot: None,
        snapshot_history: Vec::new(),
        extensions: None,
    }
    .validate()
    .expect("config invalid report");

    let invalid = StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: "demo".to_owned(),
        target_status: TargetStatus::Ready,
        reason_code: ReasonCode::Ok,
        state_phase: None,
        artifacts: ArtifactStatus {
            current_valid: true,
            previous_valid: true,
        },
        current_snapshot: None,
        snapshot_history: Vec::new(),
        extensions: None,
    };
    assert!(invalid.validate().is_err());

    let wrong_target_status = StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: "demo".to_owned(),
        target_status: TargetStatus::Ready,
        reason_code: ReasonCode::ConfigInvalid,
        state_phase: None,
        artifacts: ArtifactStatus {
            current_valid: false,
            previous_valid: false,
        },
        current_snapshot: None,
        snapshot_history: Vec::new(),
        extensions: None,
    };
    assert!(wrong_target_status.validate().is_err());

    let invalid_identity = StatusReport {
        schema_name: "wrong".to_owned(),
        ..StatusReport {
            schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: STATUS_REPORT_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            target_status: TargetStatus::Ready,
            reason_code: ReasonCode::Ok,
            state_phase: Some(StatePhase::HasBaseline),
            artifacts: ArtifactStatus {
                current_valid: true,
                previous_valid: true,
            },
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        }
    };
    assert!(invalid_identity.validate().is_err());

    let mut invalid_snapshot = StatusReport {
        schema_name: STATUS_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: STATUS_REPORT_SCHEMA_VERSION,
        target_id: "demo".to_owned(),
        target_status: TargetStatus::Ready,
        reason_code: ReasonCode::Ok,
        state_phase: Some(StatePhase::HasBaseline),
        artifacts: ArtifactStatus {
            current_valid: true,
            previous_valid: true,
        },
        current_snapshot: Some(SnapshotDigestSummary {
            canonical_text_sha256: "bad".to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            captured_at: "2026-04-05T10:15:30Z".to_owned(),
        }),
        snapshot_history: Vec::new(),
        extensions: None,
    };
    assert!(invalid_snapshot.validate().is_err());

    invalid_snapshot.current_snapshot = Some(SnapshotDigestSummary {
        canonical_text_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        captured_at: "2026-04-05T10:15:30Z".to_owned(),
    });
    invalid_snapshot.snapshot_history = vec![SnapshotDigestSummary {
        canonical_text_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        captured_at: "2026-04-05T10:14:30Z".to_owned(),
    }];
    invalid_snapshot.validate().expect("ready status report");
}

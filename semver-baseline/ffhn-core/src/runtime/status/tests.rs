use super::super::storage::{write_exact_text, write_json, write_text};
use super::*;
use crate::{
    CompareBasis, CompareConfig, CoreError, EXTRACTION_RECORD_SCHEMA_NAME,
    EXTRACTION_RECORD_SCHEMA_VERSION, ExtractionRecord, FetchConfig, FetchEngine,
    HTMLCUT_INTEROP_PROFILE, HttpMethod, OutputKind, ReasonCode, RunOutcome, SelectionConfig,
    SelectionKind, SelectionMatch, SnapshotReference, SnapshotSlot, TargetSource, WhitespaceMode,
};
use serde_json::json;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;
use url::Url;

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn target_document(target_id: &str) -> TargetDocument {
    TargetDocument {
        schema_name: crate::TARGET_SCHEMA_NAME.to_owned(),
        schema_version: crate::TARGET_SCHEMA_VERSION,
        target_id: target_id.to_owned(),
        display_name: "Demo".to_owned(),
        enabled: true,
        target: TargetSource {
            kind: crate::model::TargetKind::Http,
            source_url: Some(Url::parse("https://example.com/page").expect("url")),
            file_path: None,
        },
        fetch: FetchConfig {
            engine: FetchEngine::Http,
            method: HttpMethod::GET,
            timeout_ms: 15_000,
            max_bytes: 2_000_000,
            user_agent: "ffhn/2.0.0".to_owned(),
            follow_redirects: true,
            accept: "text/html".to_owned(),
            headers: Default::default(),
            extensions: None,
        },
        selection: SelectionConfig {
            kind: SelectionKind::CssSelector,
            r#match: SelectionMatch::Single,
            index: None,
            output: OutputKind::OuterHtml,
            whitespace: WhitespaceMode::Normalize,
            rewrite_urls: false,
            selector: Some("main".to_owned()),
            start: None,
            end: None,
            mode: None,
            include_start: None,
            include_end: None,
            flags: Vec::new(),
        },
        compare: CompareConfig {
            basis: CompareBasis::CanonicalTextSha256,
            canonicalization: Vec::new(),
        },
        storage: Default::default(),
        notifications: Vec::new(),
        extensions: None,
    }
}

fn snapshot(slot: SnapshotSlot, name: &str, canonical: &str, outer: &str) -> SnapshotReference {
    SnapshotReference {
        slot,
        canonical_text_sha256: crate::stable_json::sha256_hex(canonical.as_bytes()),
        outer_html_sha256: crate::stable_json::sha256_hex(outer.as_bytes()),
        extraction_record_path: format!("snapshots/{name}/extraction.json"),
        canonical_text_path: format!("snapshots/{name}/canonical.txt"),
        outer_html_path: format!("snapshots/{name}/outer.html"),
        captured_at: "2026-04-05T10:15:30Z".to_owned(),
    }
}

fn write_target(paths: &TargetPaths, target: &TargetDocument) {
    write_text(
        paths.target_file(),
        &toml::to_string(target).expect("target toml"),
    )
    .expect("write target");
}

fn write_valid_state(paths: &TargetPaths) {
    let canonical = "hello";
    let outer = "<main>Hello</main>";
    let current = snapshot(SnapshotSlot::Current, "current", canonical, outer);
    write_exact_text(
        paths.target_dir().join(&current.canonical_text_path),
        canonical,
    )
    .expect("write canonical");
    write_exact_text(paths.target_dir().join(&current.outer_html_path), outer)
        .expect("write outer html");
    write_json(
        paths.target_dir().join(&current.extraction_record_path),
        &ExtractionRecord {
            schema_name: EXTRACTION_RECORD_SCHEMA_NAME.to_owned(),
            schema_version: EXTRACTION_RECORD_SCHEMA_VERSION,
            interop_profile: HTMLCUT_INTEROP_PROFILE.to_owned(),
            htmlcut_plan_digest_sha256: DIGEST.to_owned(),
            htmlcut_result_digest_sha256: DIGEST.to_owned(),
            comparison_input_sha256: DIGEST.to_owned(),
            outer_html_sha256: current.outer_html_sha256.clone(),
            strategy_kind: SelectionKind::CssSelector,
            selection_mode: SelectionMatch::Single,
            output_kind: OutputKind::OuterHtml,
            candidate_count: 1,
            selected_candidate_index: 1,
            match_metadata: json!({"selector": "main"}),
            warning_codes: Vec::new(),
            created_at: "2026-04-05T10:15:30Z".to_owned(),
            extensions: None,
        },
    )
    .expect("write extraction record");
    write_json(
        paths.state_file(),
        &crate::StateDocument {
            schema_name: crate::STATE_SCHEMA_NAME.to_owned(),
            schema_version: crate::STATE_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            state_phase: StatePhase::HasBaseline,
            last_run_at: Some("2026-04-05T10:15:30Z".to_owned()),
            last_run_outcome: Some(RunOutcome::Initialized),
            last_reason_code: Some(ReasonCode::Ok),
            current_snapshot: Some(current),
            snapshot_history: Vec::new(),
            extensions: None,
        },
    )
    .expect("write state");
}

#[test]
fn validate_target_reads_toml_and_enforces_directory_identity() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");

    write_target(&paths, &target_document("demo"));
    let target = validate_target(&paths).expect("valid target");
    assert_eq!(target.target_id, "demo");

    assert!(validate_target_against_paths(&paths, target_document("demo")).is_ok());
    assert!(validate_target_against_paths(&paths, target_document("other")).is_err());
}

#[test]
fn status_covers_config_invalid_missing_invalid_integrity_and_ready_states() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");

    write_text(paths.target_file(), "not = [valid").expect("broken target");
    let report = status(&paths).expect("config invalid status");
    assert_eq!(report.reason_code, ReasonCode::ConfigInvalid);
    assert_eq!(report.target_status, TargetStatus::Invalid);
    assert!(report.state_phase.is_none());
    assert!(!paths.run_lock_file().exists());

    write_target(&paths, &target_document("demo"));
    let report = status(&paths).expect("pending status");
    assert_eq!(report.reason_code, ReasonCode::Ok);
    assert_eq!(report.target_status, TargetStatus::Pending);
    assert_eq!(report.state_phase, Some(StatePhase::NeverSucceeded));
    assert!(paths.lock_dir().is_dir());
    assert!(paths.run_lock_file().is_file());

    write_json(
        paths.state_file(),
        &crate::StateDocument {
            schema_name: crate::STATE_SCHEMA_NAME.to_owned(),
            schema_version: crate::STATE_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            state_phase: StatePhase::NeverSucceeded,
            last_run_at: None,
            last_run_outcome: None,
            last_reason_code: None,
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        },
    )
    .expect("write valid never-succeeded state");
    let report = status(&paths).expect("valid never-succeeded status");
    assert_eq!(report.target_status, TargetStatus::Pending);

    write_json(
        paths.state_file(),
        &crate::StateDocument {
            schema_name: "wrong".to_owned(),
            schema_version: crate::STATE_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
            state_phase: StatePhase::HasBaseline,
            last_run_at: None,
            last_run_outcome: None,
            last_reason_code: None,
            current_snapshot: None,
            snapshot_history: Vec::new(),
            extensions: None,
        },
    )
    .expect("write invalid schema state");
    let report = status(&paths).expect("invalid state status");
    assert_eq!(report.reason_code, ReasonCode::StateInvalid);
    assert_eq!(report.target_status, TargetStatus::Invalid);
    assert_eq!(report.state_phase, Some(StatePhase::HasBaseline));

    #[cfg(unix)]
    {
        write_valid_state(&paths);
        let metadata = std::fs::metadata(paths.state_file()).expect("state metadata");
        let original = metadata.permissions();
        let mut denied = original.clone();
        denied.set_mode(0o000);
        std::fs::set_permissions(paths.state_file(), denied).expect("deny state permissions");
        let report = status(&paths).expect("unreadable state status");
        std::fs::set_permissions(paths.state_file(), original).expect("restore state permissions");
        assert_eq!(report.reason_code, ReasonCode::StateInvalid);
        assert_eq!(report.target_status, TargetStatus::Invalid);
        assert_eq!(report.state_phase, Some(StatePhase::NeverSucceeded));
    }

    write_valid_state(&paths);
    write_exact_text(
        paths.target_dir().join("snapshots/current/outer.html"),
        "<main>Tampered</main>",
    )
    .expect("tamper snapshot");
    let report = status(&paths).expect("integrity mismatch status");
    assert_eq!(report.reason_code, ReasonCode::IntegrityMismatch);
    assert_eq!(report.target_status, TargetStatus::Invalid);

    write_valid_state(&paths);
    let report = status(&paths).expect("ready status");
    assert_eq!(report.reason_code, ReasonCode::Ok);
    assert_eq!(report.target_status, TargetStatus::Ready);
    assert_eq!(report.state_phase, Some(StatePhase::HasBaseline));
    assert!(report.current_snapshot.is_some());

    #[cfg(unix)]
    {
        write_text(paths.target_file(), "bad = [").expect("break target");
        let metadata = std::fs::metadata(paths.state_file()).expect("state metadata");
        let original = metadata.permissions();
        let mut denied = original.clone();
        denied.set_mode(0o000);
        std::fs::set_permissions(paths.state_file(), denied).expect("deny state permissions");
        let report = status(&paths).expect("config invalid wins over unreadable state");
        std::fs::set_permissions(paths.state_file(), original).expect("restore state permissions");
        assert_eq!(report.reason_code, ReasonCode::ConfigInvalid);
        assert_eq!(report.target_status, TargetStatus::Invalid);
        assert_eq!(report.state_phase, None);
    }
}

#[test]
fn status_surfaces_target_load_io_failures_as_fatal_core_errors() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    std::fs::create_dir_all(paths.target_file()).expect("target file directory");

    let error = status(&paths).expect_err("target load io error");
    assert!(matches!(error, CoreError::Io { .. }));
}

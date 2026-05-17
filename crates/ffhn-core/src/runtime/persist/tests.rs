use super::super::state::SnapshotArtifacts;
use super::super::state::{StateLoad, load_state};
use super::super::storage::{read_json, read_text, with_write_error_injected, write_exact_text};
use super::snapshot_store::unique_snapshot_work_dir;
use super::*;
use crate::runtime::domain::PersistedState;
use crate::stable_json::{sha256_hex, stable_json};
use crate::{
    BaselinePhase, CompareBasis, CompareConfig, CoreError, EXTRACTION_RECORD_SCHEMA_NAME,
    EXTRACTION_RECORD_SCHEMA_VERSION, ExtractionRecord, FetchConfig, HttpMethod, LastRunRecord,
    NetworkFetchConfig, RelativeArtifactPath, RunFailureCause, RunOutcome, RunReport, RunResult,
    STATE_SCHEMA_NAME, STATE_SCHEMA_VERSION, SelectionConfig, SelectionEvidence, SelectionKind,
    SelectionMatch, SelectionModeConfig, SnapshotReference, SnapshotSlot, StateDocument,
    StoredBaseline, TargetDocument, TargetId, TargetPaths, TargetSource, WhitespaceMode,
};
use std::io;
use tempfile::tempdir;
use url::Url;

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn target_id(value: &str) -> TargetId {
    TargetId::new(value).expect("target id")
}

fn artifact_path(path: impl Into<String>) -> RelativeArtifactPath {
    RelativeArtifactPath::new(path).expect("relative artifact path")
}

fn target() -> TargetDocument {
    TargetDocument {
        schema_name: crate::TARGET_SCHEMA_NAME.to_owned(),
        schema_version: crate::TARGET_SCHEMA_VERSION,
        target_id: target_id("demo"),
        display_name: "Demo".to_owned(),
        enabled: true,
        target: TargetSource::Http {
            source_url: Url::parse("https://example.com/page").expect("url"),
        },
        fetch: FetchConfig::Http(NetworkFetchConfig {
            method: HttpMethod::GET,
            timeout_ms: 15_000,
            max_bytes: 2_000_000,
            user_agent: "ffhn/example".to_owned(),
            follow_redirects: true,
            accept: "text/html".to_owned(),
            headers: Default::default(),
            extensions: None,
        }),
        selection: SelectionConfig::CssSelector {
            selection_mode: SelectionModeConfig::Single,
            selector: "main".to_owned(),
        },
        compare: CompareConfig {
            basis: CompareBasis::Text,
            whitespace: Some(WhitespaceMode::Normalize),
            rewrite_urls: false,
            canonicalization: Vec::new(),
        },
        storage: Default::default(),
        notification_endpoints: Vec::new(),
        notification_routes: Vec::new(),
        extensions: None,
    }
}

fn extraction_record(outer_html_sha256: &str) -> ExtractionRecord {
    ExtractionRecord {
        schema_name: EXTRACTION_RECORD_SCHEMA_NAME.to_owned(),
        schema_version: EXTRACTION_RECORD_SCHEMA_VERSION,
        compare_source_sha256: DIGEST.to_owned(),
        outer_html_sha256: outer_html_sha256.to_owned(),
        selection_kind: SelectionKind::CssSelector,
        selection_match: SelectionMatch::Single,
        compare_basis: CompareBasis::Text,
        candidate_count: 1,
        selected_candidate_index: 1,
        selection_evidence: SelectionEvidence::CssSelector {
            path: "html > body > main".to_owned(),
            tag_name: "main".to_owned(),
        },
        warning_codes: Vec::new(),
        created_at: "2026-04-05T10:15:30Z".to_owned(),
        monitoring_contract_digest_sha256: DIGEST.to_owned(),
        extensions: None,
    }
}

fn snapshot(slot: SnapshotSlot, name: &str, canonical: &str, outer: &str) -> SnapshotArtifacts {
    let reference = SnapshotReference {
        slot,
        compare_digest_sha256: sha256_hex(canonical.as_bytes()),
        outer_html_sha256: sha256_hex(outer.as_bytes()),
        extraction_record_path: artifact_path(format!("snapshots/{name}/extraction.json")),
        compare_path: artifact_path(format!("snapshots/{name}/compare.txt")),
        outer_html_path: artifact_path(format!("snapshots/{name}/outer.html")),
        captured_at: "2026-04-05T10:15:30Z".to_owned(),
    };
    SnapshotArtifacts {
        extraction_json: stable_json(&extraction_record(&reference.outer_html_sha256))
            .expect("stable extraction record"),
        reference,
        compare_value: canonical.to_owned(),
        outer_html: outer.to_owned(),
    }
}

fn prior_state_with(
    current: Option<SnapshotArtifacts>,
    history: Vec<SnapshotArtifacts>,
) -> StateLoad {
    StateLoad::Valid(Box::new(super::super::state::LoadedState {
        state: PersistedState::from_document(StateDocument {
            schema_name: STATE_SCHEMA_NAME.to_owned(),
            schema_version: STATE_SCHEMA_VERSION,
            target_id: target_id("demo"),
            monitoring_contract_digest_sha256: DIGEST.to_owned(),
            baseline: match &current {
                Some(current_snapshot) => StoredBaseline::Ready {
                    current_snapshot: current_snapshot.reference.clone(),
                    snapshot_history: history
                        .iter()
                        .map(|snapshot| snapshot.reference.clone())
                        .collect(),
                },
                None => StoredBaseline::Pending,
            },
            last_run: Some(LastRunRecord::new(
                "2026-04-05T10:15:30Z".to_owned(),
                RunOutcome::Initialized,
                None,
            )),
            extensions: None,
        }),
        current,
    }))
}

fn write_snapshot(paths: &TargetPaths, snapshot: &SnapshotArtifacts) {
    write_exact_text(
        paths.target_dir().join(&snapshot.reference.compare_path),
        &snapshot.compare_value,
    )
    .expect("write snapshot canonical");
    write_exact_text(
        paths.target_dir().join(&snapshot.reference.outer_html_path),
        &snapshot.outer_html,
    )
    .expect("write snapshot outer");
    write_exact_text(
        paths
            .target_dir()
            .join(&snapshot.reference.extraction_record_path),
        &snapshot.extraction_json,
    )
    .expect("write snapshot extraction");
}

fn write_state(paths: &TargetPaths, state: &StateDocument) {
    super::super::storage::write_json(paths.state_file(), state).expect("write state");
}

#[test]
fn persist_state_only_updates_existing_state_and_handles_empty_prior_state() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let existing = prior_state_with(
        Some(snapshot(
            SnapshotSlot::Current,
            "current",
            "hello",
            "<main>Hello</main>",
        )),
        Vec::new(),
    );

    let (wrote_state, state) = persist_state_only(
        &paths,
        &target(),
        &existing,
        RunOutcome::FailedTransient,
        Some(RunFailureCause::FetchHttpServerError),
        "2026-04-05T11:00:00Z",
    )
    .expect("persist existing state");
    assert!(wrote_state);
    let state = state.expect("state document");
    assert_eq!(
        state
            .last_run
            .as_ref()
            .and_then(LastRunRecord::failure_cause),
        Some(RunFailureCause::FetchHttpServerError)
    );

    let (wrote_state, state) = persist_state_only(
        &paths,
        &target(),
        &StateLoad::Missing,
        RunOutcome::SkippedDisabled,
        None,
        "2026-04-05T11:00:00Z",
    )
    .expect("persist disabled state");
    assert!(wrote_state);
    assert_eq!(
        state.expect("disabled state").baseline_phase(),
        BaselinePhase::NeverSucceeded
    );

    let (wrote_state, state) = persist_state_only(
        &paths,
        &target(),
        &StateLoad::Missing,
        RunOutcome::FailedTransient,
        Some(RunFailureCause::FetchHttpServerError),
        "2026-04-05T11:00:00Z",
    )
    .expect("missing prior state");
    assert!(!wrote_state);
    assert!(state.is_none());
}

#[test]
fn persist_state_only_rejects_failed_outcomes_without_failure_causes() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let prior_state = prior_state_with(
        Some(snapshot(
            SnapshotSlot::Current,
            "current",
            "hello",
            "<main>Hello</main>",
        )),
        Vec::new(),
    );

    let transient_error = persist_state_only(
        &paths,
        &target(),
        &prior_state,
        RunOutcome::FailedTransient,
        None,
        "2026-04-05T11:00:00Z",
    )
    .expect_err("failed transient without cause should be rejected");
    assert!(
        transient_error
            .to_string()
            .contains("failed_transient persisted last_run requires a failure cause")
    );

    let permanent_error = persist_state_only(
        &paths,
        &target(),
        &prior_state,
        RunOutcome::FailedPermanent,
        None,
        "2026-04-05T11:00:00Z",
    )
    .expect_err("failed permanent without cause should be rejected");
    assert!(
        permanent_error
            .to_string()
            .contains("failed_permanent persisted last_run requires a failure cause")
    );
}

#[test]
fn persist_successful_run_rotates_current_into_history_and_prunes_to_limit() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let mut target = target();
    target.storage.history_limit = 2;
    let current = snapshot(
        SnapshotSlot::Current,
        "current",
        "before",
        "<main>Before</main>",
    );
    let older = snapshot(
        SnapshotSlot::History,
        "history/older",
        "older",
        "<main>Older</main>",
    );
    let prior_state = prior_state_with(Some(current.clone()), vec![older.clone()]);

    let extraction = extraction_record(&sha256_hex("<main>After</main>".as_bytes()));
    let state = persist_successful_run(
        &paths,
        SuccessfulPersistInput {
            target: &target,
            prior_state: &prior_state,
            run_started_at: "2026-04-05T12:00:00Z",
            run_outcome: RunOutcome::Changed,
            compare_value: "after",
            outer_html: "<main>After</main>",
            extraction_record: &extraction,
        },
    )
    .expect("persist changed run")
    .expect("state");

    assert_eq!(state.baseline_phase(), BaselinePhase::HasBaseline);
    assert_eq!(
        read_text(&paths.current_snapshot_dir().join("compare.txt")).expect("current canonical"),
        "after"
    );
    assert_eq!(state.snapshot_history().len(), 1);
    assert_eq!(state.snapshot_history()[0].slot(), SnapshotSlot::History);
    assert!(
        state.snapshot_history()[0]
            .compare_path()
            .as_str()
            .starts_with("snapshots/history/")
    );
}

#[test]
fn persist_successful_run_handles_initialized_and_unchanged_runs() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    write_exact_text(
        paths
            .history_snapshots_dir()
            .join("stale")
            .join("compare.txt"),
        "stale",
    )
    .expect("write stale history");
    let initialized = persist_successful_run(
        &paths,
        SuccessfulPersistInput {
            target: &target(),
            prior_state: &StateLoad::Missing,
            run_started_at: "2026-04-05T12:30:00Z",
            run_outcome: RunOutcome::Initialized,
            compare_value: "fresh",
            outer_html: "<main>Fresh</main>",
            extraction_record: &extraction_record(&sha256_hex("<main>Fresh</main>".as_bytes())),
        },
    )
    .expect("persist initialized run")
    .expect("state");
    assert!(initialized.snapshot_history().is_empty());
    assert!(!paths.history_snapshots_dir().join("stale").exists());

    let current = snapshot(
        SnapshotSlot::Current,
        "current",
        "same",
        "<main>Same</main>",
    );
    let history = snapshot(
        SnapshotSlot::History,
        "history/older",
        "older",
        "<main>Older</main>",
    );
    let unchanged = persist_successful_run(
        &paths,
        SuccessfulPersistInput {
            target: &target(),
            prior_state: &prior_state_with(Some(current.clone()), vec![history.clone()]),
            run_started_at: "2026-04-05T13:00:00Z",
            run_outcome: RunOutcome::Unchanged,
            compare_value: "same",
            outer_html: "<main>Same</main>",
            extraction_record: &extraction_record(&sha256_hex("<main>Same</main>".as_bytes())),
        },
    )
    .expect("persist unchanged run")
    .expect("state");
    assert_eq!(
        unchanged
            .current_snapshot()
            .expect("current")
            .compare_digest_sha256(),
        current.reference.compare_digest_sha256
    );
    assert_eq!(unchanged.snapshot_history().len(), 1);
}

#[test]
fn persist_successful_run_changed_without_prior_current_keeps_history_empty() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let state = persist_successful_run(
        &paths,
        SuccessfulPersistInput {
            target: &target(),
            prior_state: &prior_state_with(None, Vec::new()),
            run_started_at: "2026-04-05T13:30:00Z",
            run_outcome: RunOutcome::Changed,
            compare_value: "after",
            outer_html: "<main>After</main>",
            extraction_record: &extraction_record(&sha256_hex("<main>After</main>".as_bytes())),
        },
    )
    .expect("persist changed run without prior current")
    .expect("state");

    assert_eq!(state.snapshot_history(), &[]);
}

#[test]
fn persist_successful_run_surfaces_current_snapshot_write_errors_for_initialized_and_changed() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    write_exact_text(paths.snapshots_dir(), "blocked snapshots dir").expect("block snapshots dir");

    let initialized_error = persist_successful_run(
        &paths,
        SuccessfulPersistInput {
            target: &target(),
            prior_state: &StateLoad::Missing,
            run_started_at: "2026-04-05T12:30:00Z",
            run_outcome: RunOutcome::Initialized,
            compare_value: "fresh",
            outer_html: "<main>Fresh</main>",
            extraction_record: &extraction_record(&sha256_hex("<main>Fresh</main>".as_bytes())),
        },
    )
    .expect_err("initialized run should surface snapshot write errors");
    assert!(matches!(initialized_error, CoreError::Io { .. }));

    let changed_error = persist_successful_run(
        &paths,
        SuccessfulPersistInput {
            target: &target(),
            prior_state: &prior_state_with(
                Some(snapshot(
                    SnapshotSlot::Current,
                    "current",
                    "before",
                    "<main>Before</main>",
                )),
                Vec::new(),
            ),
            run_started_at: "2026-04-05T13:30:00Z",
            run_outcome: RunOutcome::Changed,
            compare_value: "after",
            outer_html: "<main>After</main>",
            extraction_record: &extraction_record(&sha256_hex("<main>After</main>".as_bytes())),
        },
    )
    .expect_err("changed run should surface snapshot write errors");
    assert!(matches!(changed_error, CoreError::Io { .. }));
}

#[test]
fn unique_snapshot_work_dir_skips_existing_candidates() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let prefix = "current-stage";

    let first_candidate = unique_snapshot_work_dir(&paths, prefix);
    let file_name = first_candidate
        .file_name()
        .and_then(|name| name.to_str())
        .expect("candidate name");
    let base_suffix = file_name
        .rsplit('-')
        .next()
        .expect("candidate suffix")
        .parse::<usize>()
        .expect("numeric suffix");

    for suffix in (base_suffix + 1)..=(base_suffix + 128) {
        let blocked = paths
            .snapshots_dir()
            .join(format!(".{prefix}-{}-{suffix}", std::process::id()));
        std::fs::create_dir_all(&blocked).expect("block snapshot work dir");
    }

    let skipped_candidate = unique_snapshot_work_dir(&paths, prefix);
    let skipped_name = skipped_candidate
        .file_name()
        .and_then(|name| name.to_str())
        .expect("skipped candidate name");
    let skipped_suffix = skipped_name
        .rsplit('-')
        .next()
        .expect("skipped suffix")
        .parse::<usize>()
        .expect("numeric skipped suffix");

    assert!(skipped_suffix > base_suffix + 128);
}

#[test]
fn persist_successful_run_rejects_failed_outcomes() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");

    let error = persist_successful_run(
        &paths,
        SuccessfulPersistInput {
            target: &target(),
            prior_state: &StateLoad::Missing,
            run_started_at: "2026-04-05T12:30:00Z",
            run_outcome: RunOutcome::FailedTransient,
            compare_value: "fresh",
            outer_html: "<main>Fresh</main>",
            extraction_record: &extraction_record(&sha256_hex("<main>Fresh</main>".as_bytes())),
        },
    )
    .expect_err("failed outcome should be rejected");

    assert!(
        error
            .to_string()
            .contains("persist_successful_run only supports successful outcomes")
    );
}

#[test]
fn persist_successful_run_rolls_back_changed_snapshot_mutations_when_state_write_fails() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let mut target = target();
    target.storage.history_limit = 2;

    let current = snapshot(
        SnapshotSlot::Current,
        "current",
        "before",
        "<main>Before</main>",
    );
    let older = snapshot(
        SnapshotSlot::History,
        "history/older",
        "older",
        "<main>Older</main>",
    );
    let oldest = snapshot(
        SnapshotSlot::History,
        "history/oldest",
        "oldest",
        "<main>Oldest</main>",
    );
    write_snapshot(&paths, &current);
    write_snapshot(&paths, &older);
    write_snapshot(&paths, &oldest);
    write_state(
        &paths,
        &StateDocument {
            schema_name: STATE_SCHEMA_NAME.to_owned(),
            schema_version: STATE_SCHEMA_VERSION,
            target_id: target_id("demo"),
            monitoring_contract_digest_sha256: DIGEST.to_owned(),
            baseline: StoredBaseline::Ready {
                current_snapshot: current.reference.clone(),
                snapshot_history: vec![older.reference.clone(), oldest.reference.clone()],
            },
            last_run: Some(LastRunRecord::new(
                "2026-04-05T10:15:30Z".to_owned(),
                RunOutcome::Initialized,
                None,
            )),
            extensions: None,
        },
    );

    let error = with_write_error_injected("state.json", io::ErrorKind::PermissionDenied, || {
        persist_successful_run(
            &paths,
            SuccessfulPersistInput {
                target: &target,
                prior_state: &prior_state_with(
                    Some(current.clone()),
                    vec![older.clone(), oldest.clone()],
                ),
                run_started_at: "2026-04-05T12:00:00Z",
                run_outcome: RunOutcome::Changed,
                compare_value: "after",
                outer_html: "<main>After</main>",
                extraction_record: &extraction_record(&sha256_hex("<main>After</main>".as_bytes())),
            },
        )
    })
    .expect_err("state write should fail");
    assert!(matches!(error, CoreError::Io { .. }));

    assert_eq!(
        read_text(&paths.current_snapshot_dir().join("compare.txt")).expect("current canonical"),
        "before"
    );
    assert!(
        paths
            .target_dir()
            .join(&older.reference.compare_path)
            .exists()
    );
    assert!(
        paths
            .target_dir()
            .join(&oldest.reference.compare_path)
            .exists()
    );
    let history_entries = std::fs::read_dir(paths.history_snapshots_dir())
        .expect("read history snapshots")
        .count();
    assert_eq!(history_entries, 2);
    let snapshot_work_dirs = std::fs::read_dir(paths.snapshots_dir())
        .expect("read snapshots dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".current-") || name.starts_with(".history-"))
        .collect::<Vec<_>>();
    assert!(snapshot_work_dirs.is_empty(), "{snapshot_work_dirs:?}");

    let loaded = load_state(&paths);
    assert!(matches!(loaded, StateLoad::Valid(_)));
    let persisted_state = read_text(&paths.state_file()).expect("state text");
    assert!(persisted_state.contains("\"last_run\":{\"outcome\":\"initialized\""));
    assert!(!persisted_state.contains("\"last_reason_code\""));
}

#[test]
fn persist_successful_run_removes_staged_current_on_initialized_state_write_failure() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");

    let error = with_write_error_injected("state.json", io::ErrorKind::PermissionDenied, || {
        persist_successful_run(
            &paths,
            SuccessfulPersistInput {
                target: &target(),
                prior_state: &StateLoad::Missing,
                run_started_at: "2026-04-05T12:30:00Z",
                run_outcome: RunOutcome::Initialized,
                compare_value: "fresh",
                outer_html: "<main>Fresh</main>",
                extraction_record: &extraction_record(&sha256_hex("<main>Fresh</main>".as_bytes())),
            },
        )
    })
    .expect_err("initialized state write should fail");
    assert!(matches!(error, CoreError::Io { .. }));
    assert!(!paths.current_snapshot_dir().exists());
    assert!(matches!(load_state(&paths), StateLoad::Missing));
}

#[test]
fn write_last_run_persists_report_json() {
    let temp = tempdir().expect("tempdir");
    let paths = TargetPaths::new(temp.path(), "demo");
    let report = RunReport {
        schema_name: crate::RUN_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: crate::RUN_REPORT_SCHEMA_VERSION,
        run_report_digest_sha256: String::new(),
        target_id: target_id("demo"),
        display_name: Some("Demo".to_owned()),
        run_started_at: "2026-04-05T10:15:30Z".to_owned(),
        run_finished_at: "2026-04-05T10:15:31Z".to_owned(),
        run_mode: crate::RunMode::Live,
        result: RunResult::Initialized,
        compare_basis: CompareBasis::Text,
        previous_compare_digest_sha256: None,
        current_compare_digest_sha256: Some(DIGEST.to_owned()),
        baseline_phase_before_run: BaselinePhase::NeverSucceeded,
        baseline_phase_after_run: BaselinePhase::HasBaseline,
        fetch: Some(crate::RunFetchSection {
            engine: crate::FetchEngine::File,
            final_url: Some("file:///tmp/source.html".to_owned()),
            http_status: None,
            content_type: None,
            bytes_read: Some(42),
            duration_ms: 1,
        }),
        extraction: Some(crate::RunExtractionSection {
            compare_source_sha256: DIGEST.to_owned(),
            outer_html_sha256: DIGEST.to_owned(),
            selection_kind: SelectionKind::CssSelector,
            selection_match: SelectionMatch::Single,
            candidate_count: 1,
            selected_candidate_index: 1,
            warning_codes: Vec::new(),
            duration_ms: 1,
        }),
        compare: Some(crate::RunCompareSection {
            canonicalizers: Vec::new(),
            duration_ms: 1,
        }),
        change: Some(crate::RunChangeSection {
            kind: crate::ChangeKind::Initialized,
            previous_compare_bytes: None,
            current_compare_bytes: 5,
            previous_compare_line_count: None,
            current_compare_line_count: 1,
            common_prefix_lines: 0,
            common_suffix_lines: 0,
            changed_region: None,
        }),
        persist: crate::RunPersistSection::from_writes(
            1,
            crate::PersistWriteStatus::Written,
            0,
            crate::PersistWriteStatus::NotAttempted,
        ),
        notifications: Vec::new(),
        extensions: None,
    }
    .with_digest()
    .expect("report digest");
    write_last_run(&paths, &report).expect("write last run");
    assert!(paths.last_run_file().is_file());
    let snapshot: crate::LastRunSnapshot = read_json(&paths.last_run_file()).expect("snapshot");
    assert_eq!(snapshot.run_report(), &report);
}

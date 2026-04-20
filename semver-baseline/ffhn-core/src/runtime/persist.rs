use std::fs;
use std::path::Path;

use crate::canonical::normalize_line_endings;
use crate::stable_json::{sha256_hex, stable_json};
use crate::{
    CoreError, ExtractionRecord, ReasonCode, RunOutcome, RunReport, STATE_SCHEMA_NAME,
    STATE_SCHEMA_VERSION, SnapshotReference, SnapshotSlot, StateDocument, StatePhase,
    TargetDocument, TargetPaths,
};

use super::state::{StateLoad, prior_valid_state};
use super::storage::{now_utc, write_exact_text, write_json};

pub(crate) struct SuccessfulPersistInput<'a> {
    pub(crate) target: &'a TargetDocument,
    pub(crate) prior_state: &'a StateLoad,
    pub(crate) run_started_at: &'a str,
    pub(crate) run_outcome: RunOutcome,
    pub(crate) canonical_text: &'a str,
    pub(crate) outer_html: &'a str,
    pub(crate) extraction_record: &'a ExtractionRecord,
}

pub(crate) fn persist_state_only(
    paths: &TargetPaths,
    target: &TargetDocument,
    prior_state: &StateLoad,
    run_outcome: RunOutcome,
    reason_code: ReasonCode,
    run_started_at: &str,
) -> Result<(bool, Option<StateDocument>), CoreError> {
    #[rustfmt::skip]
    let state = build_state_update(target, prior_state, run_outcome, reason_code, run_started_at)?;
    let Some(state) = state else {
        return Ok((false, None));
    };
    write_json(paths.state_file(), &state)?;
    let persisted_state = Some(state);
    Ok((true, persisted_state))
}

pub(crate) fn persist_successful_run(
    paths: &TargetPaths,
    input: SuccessfulPersistInput<'_>,
) -> Result<Option<StateDocument>, CoreError> {
    let extraction_json = stable_json(input.extraction_record)?;
    let prior = prior_valid_state(input.prior_state);

    let (current_snapshot, snapshot_history) = match input.run_outcome {
        RunOutcome::Initialized => {
            clear_dir_if_exists(&paths.history_snapshots_dir())?;
            let current_reference = write_new_current_snapshot(
                paths,
                input.canonical_text,
                input.outer_html,
                &extraction_json,
            )?;
            (Some(current_reference), Vec::new())
        }
        RunOutcome::Changed => {
            let current_reference = write_new_current_snapshot(
                paths,
                input.canonical_text,
                input.outer_html,
                &extraction_json,
            )?;
            let mut snapshot_history = prior
                .map(|state| state.document.snapshot_history.clone())
                .unwrap_or_default();
            if let Some(previous_current) = prior.and_then(|state| state.current.as_ref()) {
                let archived = archive_current_snapshot(paths, previous_current)?;
                snapshot_history.insert(0, archived);
            }
            prune_history(paths, &mut snapshot_history, input.target.storage.history_limit)?;
            (Some(current_reference), snapshot_history)
        }
        RunOutcome::Unchanged => (
            prior.and_then(|state| state.document.current_snapshot.clone()),
            prior
                .map(|state| state.document.snapshot_history.clone())
                .unwrap_or_default(),
        ),
        RunOutcome::FailedTransient
        | RunOutcome::FailedPermanent
        | RunOutcome::SkippedDisabled => {
            return Err(CoreError::htmlcut(
                "persist_successful_run only supports successful outcomes",
            ));
        }
    };

    let state = StateDocument {
        schema_name: STATE_SCHEMA_NAME.to_owned(),
        schema_version: STATE_SCHEMA_VERSION,
        target_id: input.target.target_id.clone(),
        state_phase: StatePhase::HasBaseline,
        last_run_at: Some(input.run_started_at.to_owned()),
        last_run_outcome: Some(input.run_outcome),
        last_reason_code: Some(ReasonCode::Ok),
        current_snapshot,
        snapshot_history,
        extensions: None,
    };
    state.validate()?;
    write_json(paths.state_file(), &state)?;
    Ok(Some(state))
}

pub(crate) fn write_last_run(paths: &TargetPaths, report: &RunReport) -> Result<(), CoreError> {
    write_json(paths.last_run_file(), report)
}

fn build_state_without_snapshot_changes(
    target: &TargetDocument,
    prior_state: &StateLoad,
    run_outcome: RunOutcome,
    reason_code: ReasonCode,
    run_started_at: &str,
) -> Result<Option<StateDocument>, CoreError> {
    match prior_valid_state(prior_state) {
        Some(prior) => {
            let mut state = prior.document.clone();
            state.last_run_at = Some(run_started_at.to_owned());
            state.last_run_outcome = Some(run_outcome);
            state.last_reason_code = Some(reason_code);
            state.validate()?;
            Ok(Some(state))
        }
        None if run_outcome == RunOutcome::SkippedDisabled => {
            let state = StateDocument {
                schema_name: STATE_SCHEMA_NAME.to_owned(),
                schema_version: STATE_SCHEMA_VERSION,
                target_id: target.target_id.clone(),
                state_phase: StatePhase::NeverSucceeded,
                last_run_at: Some(run_started_at.to_owned()),
                last_run_outcome: Some(run_outcome),
                last_reason_code: Some(reason_code),
                current_snapshot: None,
                snapshot_history: Vec::new(),
                extensions: None,
            };
            state.validate()?;
            Ok(Some(state))
        }
        None => Ok(None),
    }
}

fn build_state_update(
    target: &TargetDocument,
    prior_state: &StateLoad,
    outcome: RunOutcome,
    reason: ReasonCode,
    started_at: &str,
) -> Result<Option<StateDocument>, CoreError> {
    build_state_without_snapshot_changes(target, prior_state, outcome, reason, started_at)
}

fn write_snapshot_dir(
    dir: &Path,
    canonical_text: &str,
    outer_html: &str,
    extraction_json: &str,
) -> Result<(), CoreError> {
    fs::create_dir_all(dir).map_err(|error| CoreError::io(dir, error))?;
    let canonical_text = normalize_line_endings(canonical_text);
    let outer_html = normalize_line_endings(outer_html);
    write_exact_text(dir.join("canonical.txt"), &canonical_text)?;
    write_exact_text(dir.join("outer.html"), &outer_html)?;
    write_exact_text(dir.join("extraction.json"), extraction_json)?;
    Ok(())
}

fn write_current_snapshot(
    dir: &Path,
    canonical_text: &str,
    outer_html: &str,
    extraction_json: &str,
) -> Result<(), CoreError> {
    write_snapshot_dir(dir, canonical_text, outer_html, extraction_json)
}

fn write_snapshot_artifacts(
    dir: &Path,
    snapshot: &super::state::SnapshotArtifacts,
) -> Result<(), CoreError> {
    write_snapshot_dir(
        dir,
        &snapshot.canonical_text,
        &snapshot.outer_html,
        &snapshot.extraction_json,
    )
}

fn clear_dir_if_exists(path: &Path) -> Result<(), CoreError> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|error| CoreError::io(path, error))?;
    }
    Ok(())
}

fn write_new_current_snapshot(
    paths: &TargetPaths,
    canonical_text: &str,
    outer_html: &str,
    extraction_json: &str,
) -> Result<SnapshotReference, CoreError> {
    let captured_at = now_utc()?;
    let current_snapshot_dir = paths.current_snapshot_dir();
    write_current_snapshot(&current_snapshot_dir, canonical_text, outer_html, extraction_json)?;
    Ok(SnapshotReference {
        slot: SnapshotSlot::Current,
        canonical_text_sha256: sha256_hex(canonical_text.as_bytes()),
        outer_html_sha256: sha256_hex(outer_html.as_bytes()),
        extraction_record_path: "snapshots/current/extraction.json".to_owned(),
        canonical_text_path: "snapshots/current/canonical.txt".to_owned(),
        outer_html_path: "snapshots/current/outer.html".to_owned(),
        captured_at,
    })
}

fn archive_current_snapshot(
    paths: &TargetPaths,
    current: &super::state::SnapshotArtifacts,
) -> Result<SnapshotReference, CoreError> {
    let snapshot_key = history_snapshot_key(&current.reference);
    let history_dir = paths.history_snapshot_dir(&snapshot_key);
    write_snapshot_artifacts(&history_dir, current)?;
    Ok(SnapshotReference {
        slot: SnapshotSlot::History,
        canonical_text_sha256: current.reference.canonical_text_sha256.clone(),
        outer_html_sha256: current.reference.outer_html_sha256.clone(),
        extraction_record_path: format!("snapshots/history/{snapshot_key}/extraction.json"),
        canonical_text_path: format!("snapshots/history/{snapshot_key}/canonical.txt"),
        outer_html_path: format!("snapshots/history/{snapshot_key}/outer.html"),
        captured_at: current.reference.captured_at.clone(),
    })
}

fn history_snapshot_key(reference: &SnapshotReference) -> String {
    let compact_time = reference
        .captured_at
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    format!(
        "{compact_time}-{}",
        &reference.canonical_text_sha256[..12]
    )
}

fn prune_history(
    paths: &TargetPaths,
    snapshot_history: &mut Vec<SnapshotReference>,
    history_limit: usize,
) -> Result<(), CoreError> {
    let max_history_entries = history_limit.saturating_sub(1);
    while snapshot_history.len() > max_history_entries {
        if let Some(removed) = snapshot_history.pop() {
            let history_path = paths.target_dir().join(
                removed
                    .canonical_text_path
                    .split('/')
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("/"),
            );
            clear_dir_if_exists(&history_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::state::SnapshotArtifacts;
    use super::super::storage::read_text;
    use super::*;
    use crate::{
        CompareBasis, CompareConfig, EXTRACTION_RECORD_SCHEMA_NAME,
        EXTRACTION_RECORD_SCHEMA_VERSION, FetchConfig, FetchEngine, HTMLCUT_INTEROP_PROFILE,
        HttpMethod, OutputKind, SelectionConfig, SelectionKind, SelectionMatch, SnapshotSlot,
        TargetDocument, TargetSource, WhitespaceMode,
    };
    use serde_json::json;
    use tempfile::tempdir;
    use url::Url;

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn target() -> TargetDocument {
        TargetDocument {
            schema_name: crate::TARGET_SCHEMA_NAME.to_owned(),
            schema_version: crate::TARGET_SCHEMA_VERSION,
            target_id: "demo".to_owned(),
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

    fn extraction_record(outer_html_sha256: &str) -> ExtractionRecord {
        ExtractionRecord {
            schema_name: EXTRACTION_RECORD_SCHEMA_NAME.to_owned(),
            schema_version: EXTRACTION_RECORD_SCHEMA_VERSION,
            interop_profile: HTMLCUT_INTEROP_PROFILE.to_owned(),
            htmlcut_plan_digest_sha256: DIGEST.to_owned(),
            htmlcut_result_digest_sha256: DIGEST.to_owned(),
            comparison_input_sha256: DIGEST.to_owned(),
            outer_html_sha256: outer_html_sha256.to_owned(),
            strategy_kind: SelectionKind::CssSelector,
            selection_mode: SelectionMatch::Single,
            output_kind: OutputKind::OuterHtml,
            candidate_count: 1,
            selected_candidate_index: 1,
            match_metadata: json!({"selector": "main"}),
            warning_codes: Vec::new(),
            created_at: "2026-04-05T10:15:30Z".to_owned(),
            extensions: None,
        }
    }

    fn snapshot(slot: SnapshotSlot, name: &str, canonical: &str, outer: &str) -> SnapshotArtifacts {
        let reference = SnapshotReference {
            slot,
            canonical_text_sha256: sha256_hex(canonical.as_bytes()),
            outer_html_sha256: sha256_hex(outer.as_bytes()),
            extraction_record_path: format!("snapshots/{name}/extraction.json"),
            canonical_text_path: format!("snapshots/{name}/canonical.txt"),
            outer_html_path: format!("snapshots/{name}/outer.html"),
            captured_at: "2026-04-05T10:15:30Z".to_owned(),
        };
        SnapshotArtifacts {
            extraction_json: stable_json(&extraction_record(&reference.outer_html_sha256))
                .expect("stable extraction record"),
            reference,
            canonical_text: canonical.to_owned(),
            outer_html: outer.to_owned(),
        }
    }

    fn prior_state_with(current: Option<SnapshotArtifacts>, history: Vec<SnapshotArtifacts>) -> StateLoad {
        StateLoad::Valid(Box::new(super::super::state::LoadedState {
            document: StateDocument {
                schema_name: STATE_SCHEMA_NAME.to_owned(),
                schema_version: STATE_SCHEMA_VERSION,
                target_id: "demo".to_owned(),
                state_phase: StatePhase::HasBaseline,
                last_run_at: Some("2026-04-05T10:15:30Z".to_owned()),
                last_run_outcome: Some(RunOutcome::Initialized),
                last_reason_code: Some(ReasonCode::Ok),
                current_snapshot: current.as_ref().map(|snapshot| snapshot.reference.clone()),
                snapshot_history: history
                    .iter()
                    .map(|snapshot| snapshot.reference.clone())
                    .collect(),
                extensions: None,
            },
            current,
        }))
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
            ReasonCode::FetchHttpServerError,
            "2026-04-05T11:00:00Z",
        )
        .expect("persist existing state");
        assert!(wrote_state);
        let state = state.expect("state document");
        assert_eq!(state.last_reason_code, Some(ReasonCode::FetchHttpServerError));
        assert_eq!(state.last_run_outcome, Some(RunOutcome::FailedTransient));

        let (wrote_state, state) = persist_state_only(
            &paths,
            &target(),
            &StateLoad::Missing,
            RunOutcome::SkippedDisabled,
            ReasonCode::Disabled,
            "2026-04-05T11:00:00Z",
        )
        .expect("persist disabled state");
        assert!(wrote_state);
        assert_eq!(
            state.expect("disabled state").state_phase,
            StatePhase::NeverSucceeded
        );

        let (wrote_state, state) = persist_state_only(
            &paths,
            &target(),
            &StateLoad::Missing,
            RunOutcome::FailedTransient,
            ReasonCode::FetchHttpServerError,
            "2026-04-05T11:00:00Z",
        )
        .expect("missing prior state");
        assert!(!wrote_state);
        assert!(state.is_none());
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
                canonical_text: "after",
                outer_html: "<main>After</main>",
                extraction_record: &extraction,
            },
        )
        .expect("persist changed run")
        .expect("state");

        assert_eq!(state.state_phase, StatePhase::HasBaseline);
        assert_eq!(
            read_text(&paths.current_snapshot_dir().join("canonical.txt"))
                .expect("current canonical"),
            "after"
        );
        assert_eq!(state.snapshot_history.len(), 1);
        assert_eq!(
            state.snapshot_history[0].slot,
            SnapshotSlot::History
        );
        assert!(state.snapshot_history[0]
            .canonical_text_path
            .starts_with("snapshots/history/"));
    }

    #[test]
    fn persist_successful_run_handles_initialized_and_unchanged_runs() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        let initialized = persist_successful_run(
            &paths,
            SuccessfulPersistInput {
                target: &target(),
                prior_state: &StateLoad::Missing,
                run_started_at: "2026-04-05T12:30:00Z",
                run_outcome: RunOutcome::Initialized,
                canonical_text: "fresh",
                outer_html: "<main>Fresh</main>",
                extraction_record: &extraction_record(&sha256_hex("<main>Fresh</main>".as_bytes())),
            },
        )
        .expect("persist initialized run")
        .expect("state");
        assert!(initialized.snapshot_history.is_empty());
        assert!(paths.history_snapshots_dir().exists() || !paths.history_snapshots_dir().exists());

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
                canonical_text: "same",
                outer_html: "<main>Same</main>",
                extraction_record: &extraction_record(&sha256_hex("<main>Same</main>".as_bytes())),
            },
        )
        .expect("persist unchanged run")
        .expect("state");
        assert_eq!(
            unchanged.current_snapshot.expect("current").canonical_text_sha256,
            current.reference.canonical_text_sha256
        );
        assert_eq!(unchanged.snapshot_history.len(), 1);
    }

    #[test]
    fn write_last_run_persists_report_json() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        let report = RunReport {
            schema_name: crate::RUN_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: crate::RUN_REPORT_SCHEMA_VERSION,
            run_report_digest_sha256: String::new(),
            target_id: "demo".to_owned(),
            run_started_at: "2026-04-05T10:15:30Z".to_owned(),
            run_finished_at: "2026-04-05T10:15:31Z".to_owned(),
            run_mode: crate::RunMode::Live,
            run_outcome: RunOutcome::Initialized,
            reason_code: ReasonCode::Ok,
            failure_class: None,
            target_status_after_run: crate::TargetStatus::Ready,
            compare_basis: CompareBasis::CanonicalTextSha256,
            previous_compare_digest_sha256: None,
            current_compare_digest_sha256: Some(DIGEST.to_owned()),
            state_phase_before_run: StatePhase::NeverSucceeded,
            state_phase_after_run: StatePhase::HasBaseline,
            fetch: None,
            extraction: None,
            compare: None,
            change: Some(crate::RunChangeSection {
                kind: crate::ChangeKind::Initialized,
                previous_text_bytes: None,
                current_text_bytes: 5,
                previous_line_count: None,
                current_line_count: 1,
                common_prefix_lines: 0,
                common_suffix_lines: 0,
                changed_region: None,
            }),
            persist: crate::RunPersistSection {
                duration_ms: 1,
                wrote_state: true,
                wrote_last_run: false,
            },
            notifications: Vec::new(),
            extensions: None,
        }
        .with_digest()
        .expect("report digest");
        write_last_run(&paths, &report).expect("write last run");
        assert!(paths.last_run_file().is_file());
    }
}

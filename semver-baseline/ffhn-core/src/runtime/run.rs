use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use htmlcut_core::DiagnosticLevel;
use htmlcut_core::interop::v1::{ErrorCode, HtmlInput, InteropResult, execute_plan};

use crate::canonical::{apply_canonicalizers, normalize_line_endings};
use crate::fetch::{FetchFailure, fetch_target};
use crate::stable_json::{sha256_hex, stable_json};
use crate::{
    BatchOutcomeCounts, BatchRunEntry, BatchRunReport, BATCH_RUN_REPORT_SCHEMA_NAME,
    BATCH_RUN_REPORT_SCHEMA_VERSION, ChangeKind, CompareBasis, CoreError, FailureClass,
    NotificationEvent, NotificationHook, RunChangeRegion, RunChangeSection, RunMode,
    RunNotificationDelivery, EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION,
    ExtractionRecord, RUN_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_VERSION, RunCompareSection,
    RunExtractionSection, RunFetchSection, RunOutcome, RunPersistSection, RunReport, TargetPaths,
    TargetStatus,
};

use super::interop::{build_htmlcut_plan, map_output_kind, map_selection_mode, map_strategy_kind};
use super::lock::try_lock_exclusive;
use super::persist::{
    SuccessfulPersistInput, persist_state_only, persist_successful_run, write_last_run,
};
use super::state::{
    StateLoad, load_state, prior_compare_digest, prior_valid_state, state_phase_or_default,
    status_from_loaded_state, status_from_state,
};
use super::status::validate_target;
use super::storage::now_utc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RunOptions {
    pub(crate) mode: RunMode,
}

impl RunOptions {
    pub(crate) const LIVE: Self = Self {
        mode: RunMode::Live,
    };
    pub(crate) const DRY_RUN: Self = Self {
        mode: RunMode::DryRun,
    };
}

pub(crate) fn run_once(paths: &TargetPaths) -> Result<RunReport, CoreError> {
    run_once_with_options(paths, RunOptions::LIVE)
}

pub(crate) fn run_batch(
    watch_root: &Path,
    targets: &[String],
    options: RunOptions,
    jobs: usize,
) -> Result<BatchRunReport, CoreError> {
    let run_started_at = now_utc()?;
    let started = Instant::now();
    let max_concurrency = jobs.max(1);
    let mut entries = Vec::with_capacity(targets.len());

    for chunk in targets.chunks(max_concurrency) {
        let mut handles = Vec::with_capacity(chunk.len());
        for target_id in chunk {
            let watch_root = watch_root.to_path_buf();
            let target_id = target_id.clone();
            handles.push(thread::spawn(move || {
                let paths = TargetPaths::new(watch_root, target_id.clone());
                let entry = match run_once_with_options(&paths, options) {
                    Ok(run_report) => BatchRunEntry {
                        target_id,
                        run_report: Some(run_report),
                        fatal_error: None,
                    },
                    Err(error) => BatchRunEntry {
                        target_id,
                        run_report: None,
                        fatal_error: Some(error.to_string()),
                    },
                };
                (entry.target_id.clone(), entry)
            }));
        }

        let mut completed = handles
            .into_iter()
            .map(|handle| {
                handle.join().map_err(|_| {
                    CoreError::htmlcut("batch worker panicked before emitting a target result")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        completed.sort_by(|left, right| left.0.cmp(&right.0));
        entries.extend(completed.into_iter().map(|(_, entry)| entry));
    }

    entries.sort_by_key(|entry| {
        targets
            .iter()
            .position(|target_id| target_id == &entry.target_id)
            .unwrap_or(usize::MAX)
    });

    let mut outcome_counts = BatchOutcomeCounts {
        initialized: 0,
        changed: 0,
        unchanged: 0,
        failed_transient: 0,
        failed_permanent: 0,
        skipped_disabled: 0,
        fatal_error: 0,
    };
    for entry in &entries {
        match entry.run_report.as_ref().map(|report| report.run_outcome) {
            Some(RunOutcome::Initialized) => outcome_counts.initialized += 1,
            Some(RunOutcome::Changed) => outcome_counts.changed += 1,
            Some(RunOutcome::Unchanged) => outcome_counts.unchanged += 1,
            Some(RunOutcome::FailedTransient) => outcome_counts.failed_transient += 1,
            Some(RunOutcome::FailedPermanent) => outcome_counts.failed_permanent += 1,
            Some(RunOutcome::SkippedDisabled) => outcome_counts.skipped_disabled += 1,
            None => outcome_counts.fatal_error += 1,
        }
    }

    let report = BatchRunReport {
        schema_name: BATCH_RUN_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: BATCH_RUN_REPORT_SCHEMA_VERSION,
        run_mode: options.mode,
        watch_root: watch_root.to_string_lossy().into_owned(),
        requested_targets: targets.to_vec(),
        run_started_at,
        run_finished_at: now_utc()?,
        max_concurrency,
        entries,
        outcome_counts,
        extensions: None,
    };
    let _ = started;
    report.validate()?;
    Ok(report)
}

pub(crate) fn run_once_with_options(
    paths: &TargetPaths,
    options: RunOptions,
) -> Result<RunReport, CoreError> {
    let run_started_at = now_utc()?;
    let started = Instant::now();

    let target_result = validate_target(paths);
    let lock = if options.mode == RunMode::Live {
        Some(try_lock_exclusive(paths))
    } else {
        None
    };
    let state = match load_state(paths) {
        Ok(state) => state,
        Err(_) if target_result.is_err() => StateLoad::Missing,
        Err(error) => return Err(error),
    };

    if target_result.is_err() {
        let report = RunReport {
            schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: RUN_REPORT_SCHEMA_VERSION,
            run_report_digest_sha256: String::new(),
            target_id: paths.target_id().to_owned(),
            run_started_at: run_started_at.clone(),
            run_finished_at: now_utc()?,
            run_mode: options.mode,
            run_outcome: RunOutcome::FailedPermanent,
            reason_code: crate::ReasonCode::ConfigInvalid,
            failure_class: Some(FailureClass::Permanent),
            target_status_after_run: TargetStatus::Invalid,
            compare_basis: CompareBasis::CanonicalTextSha256,
            previous_compare_digest_sha256: prior_compare_digest(&state),
            current_compare_digest_sha256: None,
            state_phase_before_run: state_phase_or_default(&state),
            state_phase_after_run: state_phase_or_default(&state),
            fetch: None,
            extraction: None,
            compare: None,
            change: None,
            persist: RunPersistSection {
                duration_ms: started.elapsed().as_millis() as u64,
                wrote_state: false,
                wrote_last_run: false,
            },
            notifications: Vec::new(),
            extensions: None,
        };
        let _ = lock;
        return finalize_run_report(report);
    }
    let target = target_result.expect("checked above");

    if let Some(lock) = lock
        && lock.is_err()
    {
        return finish_report(
            paths,
            Some(&target),
            RunReport {
                schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
                schema_version: RUN_REPORT_SCHEMA_VERSION,
                run_report_digest_sha256: String::new(),
                target_id: target.target_id.clone(),
                run_started_at: run_started_at.clone(),
                run_finished_at: now_utc()?,
                run_mode: options.mode,
                run_outcome: RunOutcome::FailedTransient,
                reason_code: crate::ReasonCode::LockUnavailable,
                failure_class: Some(FailureClass::Transient),
                target_status_after_run: status_from_state(&state),
                compare_basis: target.compare.basis,
                previous_compare_digest_sha256: prior_compare_digest(&state),
                current_compare_digest_sha256: None,
                state_phase_before_run: state_phase_or_default(&state),
                state_phase_after_run: state_phase_or_default(&state),
                fetch: None,
                extraction: None,
                compare: None,
                change: None,
                persist: RunPersistSection {
                    duration_ms: 0,
                    wrote_state: false,
                    wrote_last_run: false,
                },
                notifications: Vec::new(),
                extensions: None,
            },
        );
    }

    match &state {
        StateLoad::InvalidSchema(_) => {
            if options.mode == RunMode::Live {
                return finish_report(
                    paths,
                    Some(&target),
                    RunReport {
                        schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
                        schema_version: RUN_REPORT_SCHEMA_VERSION,
                        run_report_digest_sha256: String::new(),
                        target_id: target.target_id.clone(),
                        run_started_at: run_started_at.clone(),
                        run_finished_at: now_utc()?,
                        run_mode: options.mode,
                        run_outcome: RunOutcome::FailedPermanent,
                        reason_code: crate::ReasonCode::StateInvalid,
                        failure_class: Some(FailureClass::Permanent),
                        target_status_after_run: TargetStatus::Invalid,
                        compare_basis: target.compare.basis,
                        previous_compare_digest_sha256: prior_compare_digest(&state),
                        current_compare_digest_sha256: None,
                        state_phase_before_run: state_phase_or_default(&state),
                        state_phase_after_run: state_phase_or_default(&state),
                        fetch: None,
                        extraction: None,
                        compare: None,
                        change: None,
                        persist: RunPersistSection {
                            duration_ms: 0,
                            wrote_state: false,
                            wrote_last_run: false,
                        },
                        notifications: Vec::new(),
                        extensions: None,
                    },
                );
            }
        }
        StateLoad::IntegrityMismatch(_) => {
            if options.mode == RunMode::Live {
                return finish_report(
                    paths,
                    Some(&target),
                    RunReport {
                        schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
                        schema_version: RUN_REPORT_SCHEMA_VERSION,
                        run_report_digest_sha256: String::new(),
                        target_id: target.target_id.clone(),
                        run_started_at: run_started_at.clone(),
                        run_finished_at: now_utc()?,
                        run_mode: options.mode,
                        run_outcome: RunOutcome::FailedPermanent,
                        reason_code: crate::ReasonCode::IntegrityMismatch,
                        failure_class: Some(FailureClass::Permanent),
                        target_status_after_run: TargetStatus::Invalid,
                        compare_basis: target.compare.basis,
                        previous_compare_digest_sha256: prior_compare_digest(&state),
                        current_compare_digest_sha256: None,
                        state_phase_before_run: state_phase_or_default(&state),
                        state_phase_after_run: state_phase_or_default(&state),
                        fetch: None,
                        extraction: None,
                        compare: None,
                        change: None,
                        persist: RunPersistSection {
                            duration_ms: 0,
                            wrote_state: false,
                            wrote_last_run: false,
                        },
                        notifications: Vec::new(),
                        extensions: None,
                    },
                );
            }
        }
        StateLoad::Missing | StateLoad::Valid(_) => {}
    }

    if options.mode == RunMode::Live && !target.enabled {
        let persist_started = Instant::now();
        let (wrote_state, state_after_run) =
            persist_disabled_state(paths, &target, &state, &run_started_at)?;
        return finish_report(
            paths,
            Some(&target),
            RunReport {
                schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
                schema_version: RUN_REPORT_SCHEMA_VERSION,
                run_report_digest_sha256: String::new(),
                target_id: target.target_id.clone(),
                run_started_at: run_started_at.clone(),
                run_finished_at: now_utc()?,
                run_mode: options.mode,
                run_outcome: RunOutcome::SkippedDisabled,
                reason_code: crate::ReasonCode::Disabled,
                failure_class: None,
                target_status_after_run: status_from_loaded_state(state_after_run.as_ref()),
                compare_basis: target.compare.basis,
                previous_compare_digest_sha256: prior_compare_digest(&state),
                current_compare_digest_sha256: None,
                state_phase_before_run: state_phase_or_default(&state),
                state_phase_after_run: state_after_run
                    .as_ref()
                    .map(|state| state.state_phase)
                    .unwrap_or_else(|| state_phase_or_default(&state)),
                fetch: None,
                extraction: None,
                compare: None,
                change: None,
                persist: RunPersistSection {
                    duration_ms: persist_started.elapsed().as_millis() as u64,
                    wrote_state,
                    wrote_last_run: false,
                },
                notifications: Vec::new(),
                extensions: None,
            },
        );
    }

    let fetch = match fetch_target(&target) {
        Ok(fetch) => fetch,
        Err(FetchFailure {
            reason_code,
            report,
        }) => {
            let persist_started = Instant::now();
            let (wrote_state, state_after_run) = if options.mode == RunMode::Live {
                persist_failed_state(paths, &target, &state, reason_code, &run_started_at)?
            } else {
                (false, None)
            };
            return finish_report(
                paths,
                Some(&target),
                RunReport {
                    schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
                    schema_version: RUN_REPORT_SCHEMA_VERSION,
                    run_report_digest_sha256: String::new(),
                    target_id: target.target_id.clone(),
                    run_started_at: run_started_at.clone(),
                    run_finished_at: now_utc()?,
                    run_mode: options.mode,
                    run_outcome: failure_run_outcome(reason_code),
                    reason_code,
                    failure_class: reason_code.failure_class(),
                    target_status_after_run: if options.mode == RunMode::Live {
                        status_from_loaded_state(state_after_run.as_ref())
                    } else {
                        status_from_state(&state)
                    },
                    compare_basis: target.compare.basis,
                    previous_compare_digest_sha256: prior_compare_digest(&state),
                    current_compare_digest_sha256: None,
                    state_phase_before_run: state_phase_or_default(&state),
                    state_phase_after_run: state_after_run
                        .as_ref()
                        .map(|state| state.state_phase)
                        .unwrap_or_else(|| state_phase_or_default(&state)),
                    fetch: Some(report),
                    extraction: None,
                    compare: None,
                    change: None,
                    persist: RunPersistSection {
                        duration_ms: persist_started.elapsed().as_millis() as u64,
                        wrote_state,
                        wrote_last_run: false,
                    },
                    notifications: Vec::new(),
                    extensions: None,
                },
            );
        }
    };

    let extraction_started = Instant::now();
    let plan = build_htmlcut_plan(&target)?;
    let source = HtmlInput::new(target.target_id.clone(), fetch.html.clone())
        .map_err(|error| CoreError::htmlcut(error.to_string()))?
        .with_input_base_url(fetch.final_url.clone());
    let htmlcut_result = match execute_plan(&source, &plan) {
        Ok(result) => {
            result
                .validate()
                .map_err(|error| CoreError::htmlcut(error.to_string()))?;
            result
        }
        Err(error) => {
            error
                .validate()
                .map_err(|validation_error| CoreError::htmlcut(validation_error.to_string()))?;
            return finalize_failed_run(
                paths,
                &target,
                &state,
                &run_started_at,
                reason_code_for_htmlcut_error(error.error_code),
                Some(fetch.report.clone()),
                options,
            );
        }
    };
    let extraction_duration_ms = extraction_started.elapsed().as_millis() as u64;

    let selected_outer_html = required_outer_html(&htmlcut_result)?;
    let comparison_input_text =
        normalize_line_endings(&htmlcut_result.selected_match.comparison_input_text);
    let comparison_input_sha256 = sha256_hex(comparison_input_text.as_bytes());
    let outer_html_sha256 = sha256_hex(selected_outer_html.as_bytes());
    let warning_codes = htmlcut_result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Warning)
        .map(|diagnostic| diagnostic.code.clone())
        .collect::<Vec<_>>();
    let match_metadata = serde_json::to_value(&htmlcut_result.selected_match.metadata)?;
    let extraction_record = ExtractionRecord {
        schema_name: EXTRACTION_RECORD_SCHEMA_NAME.to_owned(),
        schema_version: EXTRACTION_RECORD_SCHEMA_VERSION,
        interop_profile: htmlcut_result.interop_profile.clone(),
        htmlcut_plan_digest_sha256: htmlcut_result.plan_digest_sha256.clone(),
        htmlcut_result_digest_sha256: htmlcut_result.result_digest_sha256.clone(),
        comparison_input_sha256: comparison_input_sha256.clone(),
        outer_html_sha256: outer_html_sha256.clone(),
        strategy_kind: map_strategy_kind(&htmlcut_result)?,
        selection_mode: map_selection_mode(&htmlcut_result)?,
        output_kind: map_output_kind(htmlcut_result.selected_match.value_kind),
        candidate_count: htmlcut_result.candidate_count,
        selected_candidate_index: htmlcut_result.selected_match.candidate_index.get(),
        match_metadata,
        warning_codes: warning_codes.clone(),
        created_at: now_utc()?,
        extensions: None,
    };
    extraction_record.validate()?;

    let compare_started = Instant::now();
    let canonical_text =
        apply_canonicalizers(&comparison_input_text, &target.compare.canonicalization)
            .map_err(|_| CoreError::htmlcut("compare canonicalization failed"))?;
    let current_compare_digest_sha256 = sha256_hex(canonical_text.as_bytes());
    let compare_duration_ms = compare_started.elapsed().as_millis() as u64;

    let previous_compare_digest_sha256 = prior_compare_digest(&state);
    let run_outcome = run_outcome_from_digests(
        previous_compare_digest_sha256.as_deref(),
        &current_compare_digest_sha256,
    );
    let change_section = build_change_section(
        prior_valid_state(&state)
            .and_then(|loaded| loaded.current.as_ref())
            .map(|snapshot| snapshot.canonical_text.as_str()),
        &canonical_text,
        run_outcome,
    );

    let extraction_section = RunExtractionSection {
        interop_profile: htmlcut_result.interop_profile.clone(),
        htmlcut_plan_digest_sha256: htmlcut_result.plan_digest_sha256.clone(),
        htmlcut_result_digest_sha256: htmlcut_result.result_digest_sha256.clone(),
        comparison_input_sha256,
        outer_html_sha256,
        strategy_kind: extraction_record.strategy_kind,
        selection_mode: extraction_record.selection_mode,
        output_kind: extraction_record.output_kind,
        candidate_count: extraction_record.candidate_count,
        selected_candidate_index: extraction_record.selected_candidate_index,
        warning_codes,
        duration_ms: extraction_duration_ms,
    };
    let compare_section = RunCompareSection {
        canonicalizers: target
            .compare
            .canonicalization
            .iter()
            .map(|canonicalizer| canonicalizer.kind.as_str().to_owned())
            .collect(),
        duration_ms: compare_duration_ms,
    };

    let persist_started = Instant::now();
    let persist_result = if options.mode == RunMode::Live {
        persist_successful_run(
            paths,
            SuccessfulPersistInput {
                target: &target,
                prior_state: &state,
                run_started_at: &run_started_at,
                run_outcome,
                canonical_text: &canonical_text,
                outer_html: &selected_outer_html,
                extraction_record: &extraction_record,
            },
        )?
    } else {
        None
    };

    finish_report(
        paths,
        Some(&target),
        RunReport {
            schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
            schema_version: RUN_REPORT_SCHEMA_VERSION,
            run_report_digest_sha256: String::new(),
            target_id: target.target_id.clone(),
            run_started_at,
            run_finished_at: now_utc()?,
            run_mode: options.mode,
            run_outcome,
            reason_code: crate::ReasonCode::Ok,
            failure_class: None,
            target_status_after_run: if options.mode == RunMode::Live {
                status_from_loaded_state(persist_result.as_ref())
            } else {
                status_from_state(&state)
            },
            compare_basis: target.compare.basis,
            previous_compare_digest_sha256,
            current_compare_digest_sha256: Some(current_compare_digest_sha256),
            state_phase_before_run: state_phase_or_default(&state),
            state_phase_after_run: persist_result
                .as_ref()
                .map(|state| state.state_phase)
                .unwrap_or_else(|| state_phase_or_default(&state)),
            fetch: Some(fetch.report),
            extraction: Some(extraction_section),
            compare: Some(compare_section),
            change: Some(change_section),
            persist: RunPersistSection {
                duration_ms: persist_started.elapsed().as_millis() as u64,
                wrote_state: options.mode == RunMode::Live && persist_result.is_some(),
                wrote_last_run: false,
            },
            notifications: Vec::new(),
            extensions: None,
        },
    )
}

fn finalize_run_report(report: RunReport) -> Result<RunReport, CoreError> {
    let report = report.with_digest()?;
    report.validate()?;
    Ok(report)
}

fn required_outer_html(result: &InteropResult) -> Result<String, CoreError> {
    result
        .selected_match
        .outer_html
        .as_deref()
        .map(normalize_line_endings)
        .ok_or_else(|| {
            CoreError::htmlcut(
                "htmlcut.result selected_match.outer_html is required for persistence",
            )
        })
}

fn finalize_failed_run(
    paths: &TargetPaths,
    target: &crate::TargetDocument,
    state: &StateLoad,
    run_started_at: &str,
    reason_code: crate::ReasonCode,
    fetch: Option<RunFetchSection>,
    options: RunOptions,
) -> Result<RunReport, CoreError> {
    let persist_started = Instant::now();
    let (wrote_state, state_after_run) = if options.mode == RunMode::Live {
        persist_failed_state(paths, target, state, reason_code, run_started_at)?
    } else {
        (false, None)
    };
    finish_report(
        paths,
        Some(target),
        RunReport {
        schema_name: RUN_REPORT_SCHEMA_NAME.to_owned(),
        schema_version: RUN_REPORT_SCHEMA_VERSION,
        run_report_digest_sha256: String::new(),
        target_id: target.target_id.clone(),
        run_started_at: run_started_at.to_owned(),
        run_finished_at: now_utc()?,
        run_mode: options.mode,
        run_outcome: failure_run_outcome(reason_code),
        reason_code,
        failure_class: reason_code.failure_class(),
        target_status_after_run: if options.mode == RunMode::Live {
            status_from_loaded_state(state_after_run.as_ref())
        } else {
            status_from_state(state)
        },
        compare_basis: target.compare.basis,
        previous_compare_digest_sha256: prior_compare_digest(state),
        current_compare_digest_sha256: None,
        state_phase_before_run: state_phase_or_default(state),
        state_phase_after_run: state_after_run
            .as_ref()
            .map(|state| state.state_phase)
            .unwrap_or_else(|| state_phase_or_default(state)),
        fetch,
        extraction: None,
        compare: None,
        change: None,
        persist: RunPersistSection {
            duration_ms: persist_started.elapsed().as_millis() as u64,
            wrote_state,
            wrote_last_run: false,
        },
        notifications: Vec::new(),
        extensions: None,
    },
    )
}

fn persist_run_state(
    paths: &TargetPaths,
    target: &crate::TargetDocument,
    state: &StateLoad,
    run_outcome: RunOutcome,
    reason_code: crate::ReasonCode,
    run_started_at: &str,
) -> Result<(bool, Option<crate::StateDocument>), CoreError> {
    persist_state_only(
        paths,
        target,
        state,
        run_outcome,
        reason_code,
        run_started_at,
    )
}

fn persist_disabled_state(
    paths: &TargetPaths,
    target: &crate::TargetDocument,
    state: &StateLoad,
    run_started_at: &str,
) -> Result<(bool, Option<crate::StateDocument>), CoreError> {
    let disabled_outcome = RunOutcome::SkippedDisabled;
    let disabled_reason = crate::ReasonCode::Disabled;
    persist_run_state(
        paths,
        target,
        state,
        disabled_outcome,
        disabled_reason,
        run_started_at,
    )
}

fn persist_failed_state(
    paths: &TargetPaths,
    target: &crate::TargetDocument,
    state: &StateLoad,
    reason_code: crate::ReasonCode,
    run_started_at: &str,
) -> Result<(bool, Option<crate::StateDocument>), CoreError> {
    persist_run_state(
        paths,
        target,
        state,
        failure_run_outcome(reason_code),
        reason_code,
        run_started_at,
    )
}

fn reason_code_for_htmlcut_error(error_code: ErrorCode) -> crate::ReasonCode {
    match error_code {
        ErrorCode::PlanInvalid => crate::ReasonCode::ExtractionPlanInvalid,
        ErrorCode::NoMatch => crate::ReasonCode::ExtractionNoMatch,
        ErrorCode::AmbiguousMatch => crate::ReasonCode::ExtractionAmbiguousMatch,
        ErrorCode::InternalError => crate::ReasonCode::ExtractionInternalError,
    }
}

fn run_outcome_from_digests(previous: Option<&str>, current: &str) -> RunOutcome {
    match previous {
        None => RunOutcome::Initialized,
        Some(previous) => {
            if previous == current {
                RunOutcome::Unchanged
            } else {
                RunOutcome::Changed
            }
        }
    }
}

fn failure_run_outcome(reason_code: crate::ReasonCode) -> RunOutcome {
    match reason_code.failure_class() {
        Some(FailureClass::Transient) => RunOutcome::FailedTransient,
        Some(FailureClass::Permanent) => RunOutcome::FailedPermanent,
        None => RunOutcome::FailedPermanent,
    }
}

fn finish_report(
    paths: &TargetPaths,
    target: Option<&crate::TargetDocument>,
    report: RunReport,
) -> Result<RunReport, CoreError> {
    let mut report = finalize_run_report(report)?;
    if report.run_mode == RunMode::DryRun {
        return Ok(report);
    }

    if let Some(target) = target {
        report.notifications = dispatch_notifications(target, &report);
        report = finalize_run_report(report)?;
        let mut persisted_report = report.clone();
        persisted_report.persist.wrote_last_run = true;
        persisted_report = finalize_run_report(persisted_report)?;
        if write_last_run(paths, &persisted_report).is_ok() {
            return Ok(persisted_report);
        }
        return Ok(report);
    }

    Ok(report)
}

fn dispatch_notifications(
    target: &crate::TargetDocument,
    report: &RunReport,
) -> Vec<RunNotificationDelivery> {
    let Some(event) = notification_event(report.run_outcome) else {
        return Vec::new();
    };
    let payload = match stable_json(report) {
        Ok(payload) => payload,
        Err(error) => {
            return vec![RunNotificationDelivery {
                hook_name: "ffhn-internal".to_owned(),
                event,
                delivered: false,
                timed_out: false,
                exit_code: None,
                duration_ms: 0,
                error: Some(error.to_string()),
            }];
        }
    };

    target
        .notifications
        .iter()
        .filter(|hook| hook.on.contains(&event))
        .map(|hook| deliver_notification(hook, event, target, report, &payload))
        .collect()
}

fn notification_event(run_outcome: RunOutcome) -> Option<NotificationEvent> {
    match run_outcome {
        RunOutcome::Initialized => Some(NotificationEvent::Initialized),
        RunOutcome::Changed => Some(NotificationEvent::Changed),
        RunOutcome::Unchanged => Some(NotificationEvent::Unchanged),
        RunOutcome::FailedTransient => Some(NotificationEvent::FailedTransient),
        RunOutcome::FailedPermanent => Some(NotificationEvent::FailedPermanent),
        RunOutcome::SkippedDisabled => Some(NotificationEvent::SkippedDisabled),
    }
}

fn deliver_notification(
    hook: &NotificationHook,
    event: NotificationEvent,
    target: &crate::TargetDocument,
    report: &RunReport,
    payload: &str,
) -> RunNotificationDelivery {
    let started = Instant::now();
    let mut child = match Command::new(&hook.shell)
        .arg("-c")
        .arg(&hook.command)
        .env("FFHN_TARGET_ID", &target.target_id)
        .env("FFHN_RUN_OUTCOME", serde_variant_name(report.run_outcome))
        .env("FFHN_REASON_CODE", serde_variant_name(report.reason_code))
        .env("FFHN_RUN_MODE", serde_variant_name(report.run_mode))
        .env(
            "FFHN_FAILURE_CLASS",
            report
                .failure_class
                .map(serde_variant_name)
                .unwrap_or_default(),
        )
        .env("FFHN_NOTIFICATION_EVENT", serde_variant_name(event))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return RunNotificationDelivery {
                hook_name: hook.name.clone(),
                event,
                delivered: false,
                timed_out: false,
                exit_code: None,
                duration_ms: started.elapsed().as_millis() as u64,
                error: Some(error.to_string()),
            };
        }
    };

    if let Some(mut stdin) = child.stdin.take()
        && let Err(error) = stdin.write_all(payload.as_bytes())
    {
        let _ = child.kill();
        let _ = child.wait();
        return RunNotificationDelivery {
            hook_name: hook.name.clone(),
            event,
            delivered: false,
            timed_out: false,
            exit_code: None,
            duration_ms: started.elapsed().as_millis() as u64,
            error: Some(error.to_string()),
        };
    }

    let deadline = started + Duration::from_millis(hook.timeout_ms);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return RunNotificationDelivery {
                    hook_name: hook.name.clone(),
                    event,
                    delivered: status.success(),
                    timed_out: false,
                    exit_code: status.code(),
                    duration_ms: started.elapsed().as_millis() as u64,
                    error: if status.success() {
                        None
                    } else {
                        Some(format!("hook exited with status {status}"))
                    },
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return RunNotificationDelivery {
                        hook_name: hook.name.clone(),
                        event,
                        delivered: false,
                        timed_out: true,
                        exit_code: None,
                        duration_ms: started.elapsed().as_millis() as u64,
                        error: Some("hook timed out".to_owned()),
                    };
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                return RunNotificationDelivery {
                    hook_name: hook.name.clone(),
                    event,
                    delivered: false,
                    timed_out: false,
                    exit_code: None,
                    duration_ms: started.elapsed().as_millis() as u64,
                    error: Some(error.to_string()),
                };
            }
        }
    }
}

fn serde_variant_name<T: serde::Serialize>(value: T) -> String {
    // Every caller passes one C-like enum variant without payloads.
    let serialized = serde_json::to_value(value).expect("enum serialization");
    serialized
        .as_str()
        .expect("string enum serialization")
        .to_owned()
}

fn build_change_section(
    previous: Option<&str>,
    current: &str,
    run_outcome: RunOutcome,
) -> RunChangeSection {
    let previous_lines = previous.map(split_lines).unwrap_or_default();
    let current_lines = split_lines(current);
    let common_prefix_lines = common_prefix_len(&previous_lines, &current_lines);
    let common_suffix_lines = common_suffix_len(&previous_lines, &current_lines, common_prefix_lines);

    let changed_region = match run_outcome {
        RunOutcome::Changed | RunOutcome::Initialized => {
            let previous_region = &previous_lines
                [common_prefix_lines..previous_lines.len().saturating_sub(common_suffix_lines)];
            let current_region = &current_lines
                [common_prefix_lines..current_lines.len().saturating_sub(common_suffix_lines)];
            Some(RunChangeRegion {
                previous_start_line: common_prefix_lines + 1,
                previous_line_count: previous_region.len(),
                current_start_line: common_prefix_lines + 1,
                current_line_count: current_region.len(),
                previous_excerpt: excerpt_from_lines(previous_region),
                current_excerpt: excerpt_from_lines(current_region),
                previous_excerpt_sha256: excerpt_from_lines(previous_region)
                    .as_deref()
                    .map(|excerpt| sha256_hex(excerpt.as_bytes())),
                current_excerpt_sha256: excerpt_from_lines(current_region)
                    .as_deref()
                    .map(|excerpt| sha256_hex(excerpt.as_bytes())),
            })
        }
        RunOutcome::Unchanged
        | RunOutcome::FailedTransient
        | RunOutcome::FailedPermanent
        | RunOutcome::SkippedDisabled => None,
    };

    RunChangeSection {
        kind: match run_outcome {
            RunOutcome::Initialized => ChangeKind::Initialized,
            RunOutcome::Changed => ChangeKind::Changed,
            RunOutcome::Unchanged => ChangeKind::Unchanged,
            RunOutcome::FailedTransient
            | RunOutcome::FailedPermanent
            | RunOutcome::SkippedDisabled => ChangeKind::Unchanged,
        },
        previous_text_bytes: previous.map(str::len),
        current_text_bytes: current.len(),
        previous_line_count: previous.map(line_count),
        current_line_count: line_count(current),
        common_prefix_lines,
        common_suffix_lines,
        changed_region,
    }
}

fn split_lines(value: &str) -> Vec<&str> {
    if value.is_empty() {
        Vec::new()
    } else {
        value.split('\n').collect()
    }
}

fn line_count(value: &str) -> usize {
    split_lines(value).len()
}

fn common_prefix_len(previous: &[&str], current: &[&str]) -> usize {
    previous
        .iter()
        .zip(current.iter())
        .take_while(|(left, right)| left == right)
        .count()
}

fn common_suffix_len(previous: &[&str], current: &[&str], common_prefix_len: usize) -> usize {
    let max_suffix = previous
        .len()
        .min(current.len())
        .saturating_sub(common_prefix_len);
    let mut suffix = 0;
    while suffix < max_suffix
        && previous[previous.len() - 1 - suffix] == current[current.len() - 1 - suffix]
    {
        suffix += 1;
    }
    suffix
}

fn excerpt_from_lines(lines: &[&str]) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    let mut excerpt = lines.iter().take(4).copied().collect::<Vec<_>>().join("\n");
    if lines.len() > 4 {
        excerpt.push_str("\n...");
    }
    if excerpt.len() > 240 {
        excerpt.truncate(240);
        excerpt.push_str("...");
    }
    Some(excerpt)
}

#[cfg(test)]
mod tests {
    use super::super::lock::try_lock_exclusive;
    use super::super::storage::{read_json, write_exact_text, write_json, write_text};
    use super::*;
    use crate::{
        CompareConfig, EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION,
        ExtractionRecord, FetchConfig, FetchEngine, HTMLCUT_INTEROP_PROFILE, HttpMethod,
        OutputKind, ReasonCode, SelectionConfig, SelectionKind, SelectionMatch, SnapshotReference,
        SnapshotSlot, StatePhase, TargetDocument, TargetSource, WhitespaceMode,
    };
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use tempfile::tempdir;
    use url::Url;

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    struct TestResponse {
        status_line: &'static str,
        content_type: &'static str,
        body: &'static str,
    }

    fn serve_once(response: TestResponse) -> (Url, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind server");
        let address = listener.local_addr().expect("server addr");
        let url = Url::parse(&format!("http://{address}")).expect("server url");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept connection");
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            let raw = format!(
                "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n{}",
                response.status_line,
                response.content_type,
                response.body.len(),
                response.body
            );
            let _ = stream.write_all(raw.as_bytes());
        });
        (url, handle)
    }

    fn target_document(
        target_id: &str,
        enabled: bool,
        source_url: Url,
        selector: &str,
        selection_match: SelectionMatch,
    ) -> TargetDocument {
        TargetDocument {
            schema_name: crate::TARGET_SCHEMA_NAME.to_owned(),
            schema_version: crate::TARGET_SCHEMA_VERSION,
            target_id: target_id.to_owned(),
            display_name: "Demo".to_owned(),
            enabled,
            target: TargetSource {
                kind: crate::model::TargetKind::Http,
                source_url: Some(source_url),
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
                r#match: selection_match,
                index: None,
                output: OutputKind::OuterHtml,
                whitespace: WhitespaceMode::Normalize,
                rewrite_urls: false,
                selector: Some(selector.to_owned()),
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

    fn write_target(paths: &TargetPaths, target: &TargetDocument) {
        write_text(
            paths.target_file(),
            &toml::to_string(target).expect("target toml"),
        )
        .expect("write target");
    }

    fn snapshot_reference(
        slot: SnapshotSlot,
        name: &str,
        canonical: &str,
        outer: &str,
    ) -> SnapshotReference {
        SnapshotReference {
            slot,
            canonical_text_sha256: sha256_hex(canonical.as_bytes()),
            outer_html_sha256: sha256_hex(outer.as_bytes()),
            extraction_record_path: format!("snapshots/{name}/extraction.json"),
            canonical_text_path: format!("snapshots/{name}/canonical.txt"),
            outer_html_path: format!("snapshots/{name}/outer.html"),
            captured_at: "2026-04-05T10:15:30Z".to_owned(),
        }
    }

    fn write_snapshot_state(paths: &TargetPaths, canonical: &str, outer: &str) {
        let reference = snapshot_reference(SnapshotSlot::Current, "current", canonical, outer);
        write_exact_text(
            paths.target_dir().join(&reference.canonical_text_path),
            canonical,
        )
        .expect("write canonical");
        write_exact_text(paths.target_dir().join(&reference.outer_html_path), outer)
            .expect("write outer");
        write_json(
            paths.target_dir().join(&reference.extraction_record_path),
            &ExtractionRecord {
                schema_name: EXTRACTION_RECORD_SCHEMA_NAME.to_owned(),
                schema_version: EXTRACTION_RECORD_SCHEMA_VERSION,
                interop_profile: HTMLCUT_INTEROP_PROFILE.to_owned(),
                htmlcut_plan_digest_sha256: DIGEST.to_owned(),
                htmlcut_result_digest_sha256: DIGEST.to_owned(),
                comparison_input_sha256: DIGEST.to_owned(),
                outer_html_sha256: reference.outer_html_sha256.clone(),
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
        .expect("write extraction");
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
                current_snapshot: Some(reference),
                snapshot_history: Vec::new(),
                extensions: None,
            },
        )
        .expect("write state");
    }

    #[test]
    fn helper_functions_cover_error_mapping_and_compare_outcomes() {
        assert_eq!(
            reason_code_for_htmlcut_error(ErrorCode::PlanInvalid),
            ReasonCode::ExtractionPlanInvalid
        );
        assert_eq!(
            reason_code_for_htmlcut_error(ErrorCode::NoMatch),
            ReasonCode::ExtractionNoMatch
        );
        assert_eq!(
            reason_code_for_htmlcut_error(ErrorCode::AmbiguousMatch),
            ReasonCode::ExtractionAmbiguousMatch
        );
        assert_eq!(
            reason_code_for_htmlcut_error(ErrorCode::InternalError),
            ReasonCode::ExtractionInternalError
        );

        assert_eq!(
            run_outcome_from_digests(None, DIGEST),
            RunOutcome::Initialized
        );
        assert_eq!(
            run_outcome_from_digests(Some(DIGEST), DIGEST),
            RunOutcome::Unchanged
        );
        assert_eq!(
            run_outcome_from_digests(
                Some(DIGEST),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            ),
            RunOutcome::Changed
        );
    }

    #[test]
    fn required_outer_html_enforces_the_persisted_artifact_contract() {
        let url = Url::parse("https://example.com").expect("url");
        let target = target_document("demo", true, url.clone(), "main", SelectionMatch::Single);
        let plan = super::super::interop::build_htmlcut_plan(&target).expect("plan");
        let source = HtmlInput::new("demo".to_owned(), "<main>Hello</main>".to_owned())
            .expect("source")
            .with_input_base_url(url);

        let mut result = execute_plan(&source, &plan).expect("result");
        assert_eq!(
            required_outer_html(&result).expect("outer html"),
            "<main>Hello</main>"
        );

        result.selected_match.outer_html = None;
        assert!(required_outer_html(&result).is_err());
    }

    #[test]
    fn run_once_covers_config_invalid_lock_state_and_disabled_paths() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");
        let source_url = Url::parse("https://example.com").expect("url");

        write_target(
            &paths,
            &target_document(
                "other",
                true,
                source_url.clone(),
                "main",
                SelectionMatch::Single,
            ),
        );
        std::fs::write(paths.state_file(), [0xff]).expect("write unreadable state");
        let report = run_once(&paths).expect("config invalid run");
        assert_eq!(report.reason_code, ReasonCode::ConfigInvalid);
        assert_eq!(report.run_outcome, RunOutcome::FailedPermanent);

        write_target(
            &paths,
            &target_document(
                "demo",
                true,
                source_url.clone(),
                "main",
                SelectionMatch::Single,
            ),
        );
        let error = run_once(&paths).expect_err("state io error");
        assert!(error.to_string().contains("filesystem error"));
        std::fs::remove_file(paths.state_file()).expect("remove unreadable state");

        let _lock = try_lock_exclusive(&paths).expect("lock");
        let report = run_once(&paths).expect("lock unavailable run");
        assert_eq!(report.reason_code, ReasonCode::LockUnavailable);

        drop(_lock);
        write_exact_text(paths.state_file(), "{not json").expect("write malformed state");
        let report = run_once(&paths).expect("malformed state run");
        assert_eq!(report.reason_code, ReasonCode::StateInvalid);

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
        .expect("write invalid state");
        let report = run_once(&paths).expect("state invalid run");
        assert_eq!(report.reason_code, ReasonCode::StateInvalid);

        write_snapshot_state(&paths, "hello", "<main>Hello</main>");
        write_exact_text(
            paths.target_dir().join("snapshots/current/outer.html"),
            "<main>Tampered</main>",
        )
        .expect("tamper state");
        let report = run_once(&paths).expect("integrity mismatch run");
        assert_eq!(report.reason_code, ReasonCode::IntegrityMismatch);

        std::fs::remove_file(paths.state_file()).expect("remove state");
        write_target(
            &paths,
            &target_document("demo", false, source_url, "main", SelectionMatch::Single),
        );
        let report = run_once(&paths).expect("disabled run");
        assert_eq!(report.run_outcome, RunOutcome::SkippedDisabled);
        assert_eq!(report.reason_code, ReasonCode::Disabled);
        assert!(report.persist.wrote_state);
        assert!(paths.last_run_file().is_file());
    }

    #[test]
    fn run_once_covers_fetch_and_extraction_failures() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");

        let (url, handle) = serve_once(TestResponse {
            status_line: "500 Internal Server Error",
            content_type: "text/html",
            body: "boom",
        });
        write_target(
            &paths,
            &target_document("demo", true, url, "main", SelectionMatch::Single),
        );
        let report = run_once(&paths).expect("fetch failure run");
        handle.join().expect("server join");
        assert_eq!(report.reason_code, ReasonCode::FetchHttpServerError);
        assert_eq!(report.run_outcome, RunOutcome::FailedTransient);

        let (url, handle) = serve_once(TestResponse {
            status_line: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: "<html><body><main>Hello</main><main>Again</main></body></html>",
        });
        write_target(
            &paths,
            &target_document("demo", true, url, "main", SelectionMatch::Single),
        );
        let report = run_once(&paths).expect("ambiguous extraction run");
        handle.join().expect("server join");
        assert_eq!(report.reason_code, ReasonCode::ExtractionAmbiguousMatch);

        let (url, handle) = serve_once(TestResponse {
            status_line: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: "<html><body><aside>No match</aside></body></html>",
        });
        write_target(
            &paths,
            &target_document("demo", true, url, "main", SelectionMatch::Single),
        );
        let report = run_once(&paths).expect("no-match extraction run");
        handle.join().expect("server join");
        assert_eq!(report.reason_code, ReasonCode::ExtractionNoMatch);
        assert!(paths.last_run_file().is_file());
    }

    #[test]
    fn run_once_initializes_then_detects_unchanged_and_changed_content() {
        let temp = tempdir().expect("tempdir");
        let paths = TargetPaths::new(temp.path(), "demo");

        let (url, handle) = serve_once(TestResponse {
            status_line: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: "<html><body><main>Hello</main></body></html>",
        });
        write_target(
            &paths,
            &target_document("demo", true, url, "main", SelectionMatch::Single),
        );
        let report = run_once(&paths).expect("initialized run");
        handle.join().expect("server join");
        assert_eq!(report.run_outcome, RunOutcome::Initialized);
        assert_eq!(report.reason_code, ReasonCode::Ok);
        assert_eq!(report.target_status_after_run, TargetStatus::Ready);

        let (url, handle) = serve_once(TestResponse {
            status_line: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: "<html><body><main>Hello</main></body></html>",
        });
        write_target(
            &paths,
            &target_document("demo", true, url, "main", SelectionMatch::Single),
        );
        let report = run_once(&paths).expect("unchanged run");
        handle.join().expect("server join");
        assert_eq!(report.run_outcome, RunOutcome::Unchanged);

        let (url, handle) = serve_once(TestResponse {
            status_line: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: "<html><body><main>Changed</main></body></html>",
        });
        write_target(
            &paths,
            &target_document("demo", true, url, "main", SelectionMatch::Single),
        );
        let report = run_once(&paths).expect("changed run");
        handle.join().expect("server join");
        assert_eq!(report.run_outcome, RunOutcome::Changed);
        assert_eq!(report.reason_code, ReasonCode::Ok);

        let state: crate::StateDocument = read_json(&paths.state_file()).expect("read state");
        assert_eq!(state.state_phase, StatePhase::HasBaseline);
        assert_eq!(state.snapshot_history.len(), 1);
        assert_eq!(state.snapshot_history[0].slot, SnapshotSlot::History);
        assert!(state.snapshot_history[0]
            .canonical_text_path
            .starts_with("snapshots/history/"));
        assert!(paths.last_run_file().is_file());
        let last_run: crate::RunReport = read_json(&paths.last_run_file()).expect("last run");
        assert!(report.persist.wrote_last_run);
        assert!(last_run.persist.wrote_last_run);
        assert_eq!(last_run, report);
    }
}

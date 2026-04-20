use std::path::Path;
use std::thread;
use std::time::Instant;

use htmlcut_core::DiagnosticLevel;
use htmlcut_core::interop::v1::{ErrorCode, HtmlInput, InteropResult, execute_plan};

use crate::canonical::{apply_canonicalizers, normalize_line_endings};
use crate::fetch::{FetchFailure, fetch_target};
use crate::model::validate_batch_request_contract;
use crate::stable_json::sha256_hex;
use crate::{
    BATCH_RUN_REPORT_SCHEMA_NAME, BATCH_RUN_REPORT_SCHEMA_VERSION, BatchOutcomeCounts,
    BatchRunEntry, BatchRunReport, CoreError, EXTRACTION_RECORD_SCHEMA_NAME,
    EXTRACTION_RECORD_SCHEMA_VERSION, ExtractionRecord, FailureClass, RUN_REPORT_SCHEMA_NAME,
    RUN_REPORT_SCHEMA_VERSION, RunCompareSection, RunExtractionSection, RunMode, RunOutcome,
    RunPersistSection, RunReport, TargetPaths, TargetStatus,
};

use super::interop::{build_htmlcut_plan, map_output_kind, map_selection_mode, map_strategy_kind};
use super::lock::{ExclusiveLockError, lock_shared, try_lock_exclusive};
use super::persist::{SuccessfulPersistInput, persist_successful_run};
use super::state::{
    StateLoad, load_state, prior_compare_digest, prior_valid_state, state_phase_or_default,
    status_from_loaded_state, status_from_state,
};
use super::status::validate_target;
use super::storage::now_utc;

mod change;
mod notifications;
mod reporting;

use self::change::build_change_section;
use self::reporting::{
    PersistFailureContext, finalize_failed_run, finalize_run_report, finish_persist_failure_report,
    finish_report, invalid_target_run_report, persist_disabled_state, persist_failed_state,
};

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
    validate_batch_request_contract(targets, jobs)?;
    let run_started_at = now_utc()?;
    let started = Instant::now();
    let max_concurrency = jobs;
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
            .map(join_batch_handle)
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

    let target = match validate_target(paths) {
        Ok(target) => target,
        Err(_) => {
            let report = invalid_target_run_report(paths, &run_started_at, options.mode, started)?;
            return finalize_run_report(report);
        }
    };

    let _run_lock = match options.mode {
        RunMode::Live => match try_lock_exclusive(paths) {
            Ok(lock) => Some(lock),
            Err(ExclusiveLockError::Unavailable) => {
                let state = StateLoad::Missing;
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
            Err(ExclusiveLockError::Io(error)) => return Err(error),
        },
        RunMode::DryRun => Some(lock_shared(paths)?),
    };

    let state = load_state(paths);
    let live_state_failure_reason = if options.mode == RunMode::Live {
        match &state {
            StateLoad::Unreadable | StateLoad::InvalidSchema(_) => {
                Some(crate::ReasonCode::StateInvalid)
            }
            StateLoad::IntegrityMismatch(_) => Some(crate::ReasonCode::IntegrityMismatch),
            StateLoad::Missing | StateLoad::Valid(_) => None,
        }
    } else {
        None
    };

    if let Some(reason_code) = live_state_failure_reason {
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
                reason_code,
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

    if options.mode == RunMode::Live && !target.enabled {
        let persist_started = Instant::now();
        let (wrote_state, state_after_run) =
            match persist_disabled_state(paths, &target, &state, &run_started_at) {
                Ok(result) => result,
                Err(_) => {
                    return finish_persist_failure_report(
                        paths,
                        &target,
                        &state,
                        &run_started_at,
                        PersistFailureContext {
                            run_mode: options.mode,
                            fetch: None,
                            extraction: None,
                            compare: None,
                            change: None,
                            persist_duration_ms: persist_started.elapsed().as_millis() as u64,
                        },
                    );
                }
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
                match persist_failed_state(paths, &target, &state, reason_code, &run_started_at) {
                    Ok(result) => result,
                    Err(_) => {
                        return finish_persist_failure_report(
                            paths,
                            &target,
                            &state,
                            &run_started_at,
                            PersistFailureContext {
                                run_mode: options.mode,
                                fetch: Some(report.clone()),
                                extraction: None,
                                compare: None,
                                change: None,
                                persist_duration_ms: persist_started.elapsed().as_millis() as u64,
                            },
                        );
                    }
                }
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
        match persist_successful_run(
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
        ) {
            Ok(result) => result,
            Err(_) => {
                return finish_persist_failure_report(
                    paths,
                    &target,
                    &state,
                    &run_started_at,
                    PersistFailureContext {
                        run_mode: options.mode,
                        fetch: Some(fetch.report.clone()),
                        extraction: Some(extraction_section.clone()),
                        compare: Some(compare_section.clone()),
                        change: Some(change_section.clone()),
                        persist_duration_ms: persist_started.elapsed().as_millis() as u64,
                    },
                );
            }
        }
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

fn join_batch_handle(
    handle: thread::JoinHandle<(String, BatchRunEntry)>,
) -> Result<(String, BatchRunEntry), CoreError> {
    handle
        .join()
        .map_err(|_| CoreError::htmlcut("batch worker panicked before emitting a target result"))
}

#[cfg(test)]
mod tests;

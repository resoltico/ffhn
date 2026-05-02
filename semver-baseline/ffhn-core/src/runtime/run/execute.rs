use std::time::Instant;

use htmlcut_core::DiagnosticLevel;
use htmlcut_core::interop::v1::{HtmlInput, execute_plan};

use crate::canonical::{apply_canonicalizers, normalize_line_endings};
use crate::fetch::{FetchFailure, fetch_target};
use crate::stable_json::sha256_hex;
use crate::{
    CoreError, EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION, ExtractionRecord,
    RUN_REPORT_SCHEMA_NAME, RUN_REPORT_SCHEMA_VERSION, RunCompareSection, RunExtractionSection,
    RunMode, RunPersistSection, RunReport, TargetPaths,
};

use super::super::interop::{
    build_htmlcut_plan, map_output_kind, map_selection_mode, map_strategy_kind,
};
use super::super::lock::{ExclusiveLockError, lock_shared, try_lock_exclusive};
use super::super::persist::{SuccessfulPersistInput, persist_successful_run};
use super::super::state::{
    load_state, prior_compare_digest, prior_valid_state, state_phase_or_default,
    status_from_loaded_state, status_from_state,
};
use super::super::storage::now_utc;
use super::super::target_load::load_target_document;
use super::change::build_change_section;
use super::failures::{
    finish_disabled_target_report, finish_fetch_failure_report, finish_live_state_failure_report,
    finish_lock_unavailable_report, live_state_failure_reason,
};
use super::outcome::{
    reason_code_for_htmlcut_error, required_outer_html, run_outcome_from_digests,
};
use super::reporting::{
    PersistFailureContext, finalize_failed_run, finalize_run_report, finish_persist_failure_report,
    finish_report, invalid_target_run_report,
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

pub(crate) fn run_once_with_options(
    paths: &TargetPaths,
    options: RunOptions,
) -> Result<RunReport, CoreError> {
    let run_started_at = now_utc()?;
    let started = Instant::now();

    let target = match load_target_document(paths)? {
        Some(target) => target,
        None => {
            let report = invalid_target_run_report(paths, &run_started_at, options.mode, started)?;
            return finalize_run_report(report);
        }
    };

    let _run_lock = match options.mode {
        RunMode::Live => match try_lock_exclusive(paths) {
            Ok(lock) => Some(lock),
            Err(ExclusiveLockError::Unavailable) => {
                return finish_lock_unavailable_report(paths, &target, &run_started_at, options);
            }
            Err(ExclusiveLockError::Io(error)) => return Err(error),
        },
        RunMode::DryRun => Some(lock_shared(paths)?),
    };

    let state = load_state(paths);
    if let Some(reason_code) = live_state_failure_reason(options, &state) {
        return finish_live_state_failure_report(
            paths,
            &target,
            &state,
            &run_started_at,
            options,
            reason_code,
        );
    }

    if options.mode == RunMode::Live && !target.enabled {
        return finish_disabled_target_report(paths, &target, &state, &run_started_at, options);
    }

    let fetch = match fetch_target(&target) {
        Ok(fetch) => fetch,
        Err(FetchFailure {
            reason_code,
            report,
        }) => {
            return finish_fetch_failure_report(
                paths,
                &target,
                &state,
                &run_started_at,
                options,
                reason_code,
                report,
            );
        }
    };

    let extraction_started = Instant::now();
    let plan = build_htmlcut_plan(&target)?;
    let source = HtmlInput::new(target.target_id.clone(), fetch.html.clone())
        .map_err(|error| CoreError::htmlcut_interop(error.to_string()))?
        .with_input_base_url(fetch.final_url.clone());
    let htmlcut_result = match execute_plan(&source, &plan) {
        Ok(result) => {
            result
                .validate()
                .map_err(|error| CoreError::htmlcut_interop(error.to_string()))?;
            result
        }
        Err(error) => {
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
    // required_outer_html already confirmed selected_matches is non-empty via its own first() check.
    let selected_match = &htmlcut_result.selected_matches[0];
    let comparison_input_text = normalize_line_endings(&selected_match.comparison_input_text);
    let comparison_input_sha256 = sha256_hex(comparison_input_text.as_bytes());
    let outer_html_sha256 = sha256_hex(selected_outer_html.as_bytes());
    let warning_codes = htmlcut_result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Warning)
        .map(|diagnostic| diagnostic.code.to_string())
        .collect::<Vec<_>>();
    let match_metadata = serde_json::to_value(&selected_match.metadata)?;
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
        output_kind: map_output_kind(selected_match.value_kind),
        candidate_count: htmlcut_result.candidate_count,
        selected_candidate_index: selected_match.candidate_index.get(),
        match_metadata,
        warning_codes: warning_codes.clone(),
        created_at: now_utc()?,
        extensions: None,
    };
    extraction_record.validate()?;

    let compare_started = Instant::now();
    let canonical_text =
        apply_canonicalizers(&comparison_input_text, &target.compare.canonicalization)?;
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
            Err(error) => {
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
                        error: crate::ProcessErrorDetail::from(&error),
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
            run_finished_at: String::new(),
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
                error: None,
            },
            notifications: Vec::new(),
            extensions: None,
        },
    )
}

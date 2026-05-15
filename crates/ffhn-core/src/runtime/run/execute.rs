use std::time::Instant;

use htmlcut_core::interop::v1::{InteropDiagnosticLevel, execute_plan};

use crate::canonical::{apply_canonicalizers, normalize_line_endings};
use crate::fetch::{FetchFailure, fetch_target};
use crate::stable_json::sha256_hex;
use crate::time::elapsed_ms;
use crate::{
    CoreError, EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION, ExtractionRecord,
    PersistWriteStatus, RunCompareSection, RunExtractionSection, RunMode, RunPersistSection,
    RunReport, TargetPaths,
};

use super::super::interop::{
    build_htmlcut_input, build_htmlcut_plan, build_selection_evidence, map_output_kind,
    map_selection_mode, map_strategy_kind,
};
use super::super::lock::{ExclusiveLockError, lock_shared, try_lock_exclusive};
use super::super::persist::{SuccessfulPersistInput, persist_successful_run};
use super::super::state::{load_state, prior_compare_digest, prior_valid_state};
use super::super::storage::now_utc;
use super::super::target_load::{TargetLoad, load_target_document};
use super::change::build_change_section;
use super::failures::{
    finish_disabled_target_report, finish_live_state_failure_report,
    finish_lock_unavailable_report, live_state_failure_reason,
};
use super::outcome::{
    failure_cause_for_htmlcut_error, required_outer_html, required_selected_match,
    run_outcome_from_digests,
};
use super::report_builder::{
    RunReportDraft, RunReportLifecycle, RunReportSections, build_run_report, successful_result,
};
use super::reporting::{
    FailedRunContext, PersistFailureContext, finalize_failed_run, finalize_run_report,
    finish_persist_failure_report, finish_report, invalid_target_run_report,
    unavailable_target_run_report,
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
    paths.require_watch_root_directory()?;
    let run_started_at = now_utc()?;
    let target = match load_target_document(paths)? {
        TargetLoad::Valid(target) => target,
        TargetLoad::Invalid(error_detail) => {
            let report =
                invalid_target_run_report(paths, &run_started_at, options.mode, error_detail)?;
            return finalize_run_report(report);
        }
        TargetLoad::Unavailable(error_detail) => {
            let report =
                unavailable_target_run_report(paths, &run_started_at, options.mode, error_detail)?;
            return finalize_run_report(report);
        }
    };

    let _run_lock = match options.mode {
        RunMode::Live => match try_lock_exclusive(paths) {
            Ok(lock) => Some(lock),
            Err(ExclusiveLockError::Unavailable) => {
                let _stable_view = lock_shared(paths)?;
                let state = load_state(paths);
                return finish_lock_unavailable_report(
                    paths,
                    &target,
                    &state,
                    &run_started_at,
                    options,
                );
            }
            Err(ExclusiveLockError::Io(error)) => return Err(error),
        },
        RunMode::DryRun => Some(lock_shared(paths)?),
    };

    let state = load_state(paths);
    if let Some(failure_cause) = live_state_failure_reason(options, &state) {
        return finish_live_state_failure_report(
            paths,
            &target,
            &state,
            &run_started_at,
            options,
            failure_cause,
        );
    }

    if !target.enabled {
        return finish_disabled_target_report(paths, &target, &state, &run_started_at, options);
    }

    let fetch = match fetch_target(&target) {
        Ok(fetch) => fetch,
        Err(fetch_failure) => {
            let FetchFailure {
                failure_cause,
                error_detail,
                report,
            } = *fetch_failure;
            return finalize_failed_run(
                paths,
                &target,
                &state,
                &run_started_at,
                FailedRunContext {
                    run_mode: options.mode,
                    failure_cause,
                    error_detail,
                    fetch: Some(report),
                },
            );
        }
    };

    let extraction_started = Instant::now();
    let plan = build_htmlcut_plan(target.selection_config())?;
    let source = build_htmlcut_input(&target.target_id, fetch.html.clone(), &fetch.final_url)?;
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
                FailedRunContext {
                    run_mode: options.mode,
                    failure_cause: failure_cause_for_htmlcut_error(error.error_code),
                    error_detail: crate::ProcessErrorDetail::new(
                        crate::ProcessErrorKind::HtmlcutInterop,
                        format!("HTMLCut execution failed with {:?}", error.error_code),
                        None,
                    )
                    .expect("htmlcut execution failure detail"),
                    fetch: Some(fetch.report.clone()),
                },
            );
        }
    };
    let extraction_duration_ms = elapsed_ms(&extraction_started);

    let selected_match = required_selected_match(&htmlcut_result)?;
    let selected_outer_html = required_outer_html(selected_match)?;
    let comparison_input_text = normalize_line_endings(&selected_match.text_output);
    let comparison_input_sha256 = sha256_hex(comparison_input_text.as_bytes());
    let outer_html_sha256 = sha256_hex(selected_outer_html.as_bytes());
    let selection_kind = map_strategy_kind(&htmlcut_result)?;
    let selection_match = map_selection_mode(&htmlcut_result)?;
    let output_kind = map_output_kind(htmlcut_result.output.kind())?;
    let warning_codes = htmlcut_result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == InteropDiagnosticLevel::Warning)
        .map(|diagnostic| diagnostic.code.to_string())
        .collect::<Vec<_>>();
    let extraction_record = ExtractionRecord {
        schema_name: EXTRACTION_RECORD_SCHEMA_NAME.to_owned(),
        schema_version: EXTRACTION_RECORD_SCHEMA_VERSION,
        comparison_input_sha256: comparison_input_sha256.clone(),
        outer_html_sha256: outer_html_sha256.clone(),
        selection_kind,
        selection_match,
        output_kind,
        candidate_count: htmlcut_result.candidate_count,
        selected_candidate_index: selected_match.candidate_index.get(),
        selection_evidence: build_selection_evidence(&selected_match.metadata),
        warning_codes: warning_codes.clone(),
        created_at: now_utc()?,
        extensions: None,
    };
    extraction_record.validate()?;

    let compare_started = Instant::now();
    let canonical_text =
        apply_canonicalizers(&comparison_input_text, &target.compare.canonicalization)?;
    let current_compare_digest_sha256 = sha256_hex(canonical_text.as_bytes());
    let compare_duration_ms = elapsed_ms(&compare_started);

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
        comparison_input_sha256,
        outer_html_sha256,
        selection_kind: extraction_record.selection_kind,
        selection_match: extraction_record.selection_match,
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
                        current_compare_digest_sha256: Some(current_compare_digest_sha256.clone()),
                        fetch: Some(fetch.report.clone()),
                        extraction: Some(extraction_section.clone()),
                        compare: Some(compare_section.clone()),
                        change: Some(change_section.clone()),
                        state_commit_duration_ms: elapsed_ms(&persist_started),
                        error: crate::ProcessErrorDetail::from(&error),
                    },
                );
            }
        }
    } else {
        None
    };

    let lifecycle = if options.mode == RunMode::Live {
        RunReportLifecycle::from_live_state_transition(
            &state,
            persist_result.as_ref(),
            Some(current_compare_digest_sha256.clone()),
        )
    } else {
        RunReportLifecycle::from_state_snapshot(&state, Some(current_compare_digest_sha256.clone()))
    };
    let sections = RunReportSections {
        fetch: Some(fetch.report),
        extraction: Some(extraction_section),
        compare: Some(compare_section),
        change: Some(change_section),
    };

    finish_report(
        paths,
        Some(&target),
        build_run_report(RunReportDraft {
            target_id: target.target_id.clone(),
            display_name: Some(target.display_name.clone()),
            run_started_at,
            run_mode: options.mode,
            result: successful_result(run_outcome)?,
            compare_basis: target.compare.basis,
            lifecycle,
            sections,
            persist: RunPersistSection::from_writes(
                elapsed_ms(&persist_started),
                if options.mode == RunMode::Live {
                    PersistWriteStatus::Written
                } else {
                    PersistWriteStatus::NotAttempted
                },
                0,
                PersistWriteStatus::NotAttempted,
            ),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_result_rejects_non_success_outcomes() {
        assert!(successful_result(crate::RunOutcome::FailedTransient).is_err());
        assert!(successful_result(crate::RunOutcome::FailedPermanent).is_err());
        assert!(successful_result(crate::RunOutcome::SkippedDisabled).is_err());
    }
}

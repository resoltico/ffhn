use htmlcut_core::interop::v1::{ErrorCode, InteropResult, SelectedMatch};

use crate::canonical::normalize_line_endings;
use crate::{CoreError, FailureClass, RunOutcome};

pub(super) fn required_selected_match(result: &InteropResult) -> Result<&SelectedMatch, CoreError> {
    if result.selected_matches.len() != 1 {
        return Err(CoreError::htmlcut_interop(format!(
            "ffhn expects exactly one selected HTMLCut match, got {}",
            result.selected_matches.len()
        )));
    }

    result
        .selected_matches
        .first()
        .ok_or_else(|| CoreError::htmlcut_interop("htmlcut.result selected_matches is empty"))
}

pub(super) fn required_outer_html(selected_match: &SelectedMatch) -> Result<String, CoreError> {
    selected_match
        .outer_html
        .as_deref()
        .map(normalize_line_endings)
        .ok_or_else(|| {
            CoreError::htmlcut_interop(
                "htmlcut.selected_match.outer_html is required for persistence",
            )
        })
}

pub(super) fn reason_code_for_htmlcut_error(error_code: ErrorCode) -> crate::ReasonCode {
    match error_code {
        ErrorCode::PlanInvalid => crate::ReasonCode::ExtractionPlanInvalid,
        ErrorCode::NoMatch => crate::ReasonCode::ExtractionNoMatch,
        ErrorCode::AmbiguousMatch => crate::ReasonCode::ExtractionAmbiguousMatch,
        ErrorCode::InternalError => crate::ReasonCode::ExtractionInternalError,
    }
}

pub(super) fn run_outcome_from_digests(previous: Option<&str>, current: &str) -> RunOutcome {
    match previous {
        None => RunOutcome::Initialized,
        Some(previous) if previous == current => RunOutcome::Unchanged,
        Some(_) => RunOutcome::Changed,
    }
}

pub(super) fn failure_run_outcome(reason_code: crate::ReasonCode) -> RunOutcome {
    match reason_code.failure_class() {
        Some(FailureClass::Transient) => RunOutcome::FailedTransient,
        Some(FailureClass::Permanent) => RunOutcome::FailedPermanent,
        None => RunOutcome::FailedPermanent,
    }
}

use htmlcut_core::interop::v1::{ErrorCode, InteropResult, SelectedMatch};

use crate::canonical::normalize_line_endings;
use crate::{CompareBasis, CoreError, RunFailureCause, RunOutcome};

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
    Ok(normalize_line_endings(&selected_match.outer_html_output))
}

pub(super) fn compare_source_for_basis(
    selected_match: &SelectedMatch,
    compare_basis: CompareBasis,
) -> Result<String, CoreError> {
    match compare_basis {
        CompareBasis::Text => Ok(normalize_line_endings(&selected_match.text_output)),
        CompareBasis::InnerHtml => Ok(normalize_line_endings(&selected_match.inner_html_output)),
        CompareBasis::OuterHtml => required_outer_html(selected_match),
    }
}

pub(super) fn failure_cause_for_htmlcut_error(error_code: ErrorCode) -> RunFailureCause {
    match error_code {
        ErrorCode::PlanInvalid => RunFailureCause::SelectionContractInvalid,
        ErrorCode::NoMatch => RunFailureCause::SelectionNoMatch,
        ErrorCode::AmbiguousMatch => RunFailureCause::SelectionAmbiguousMatch,
        // FFHN's frozen HTMLCut contract never requests attribute output, so a missing-attribute
        // error indicates interop drift outside the supported profile rather than a user-facing
        // FFHN match mode.
        ErrorCode::MissingAttribute => RunFailureCause::SelectionInternalError,
        ErrorCode::InternalError => RunFailureCause::SelectionInternalError,
    }
}

pub(super) fn run_outcome_from_digests(previous: Option<&str>, current: &str) -> RunOutcome {
    match previous {
        None => RunOutcome::Initialized,
        Some(previous) if previous == current => RunOutcome::Unchanged,
        Some(_) => RunOutcome::Changed,
    }
}

pub(super) fn failure_run_outcome(failure_cause: RunFailureCause) -> RunOutcome {
    failure_cause.run_outcome()
}

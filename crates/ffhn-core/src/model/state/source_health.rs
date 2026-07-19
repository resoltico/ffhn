//! The source-health aggregate and the evidence it may own.

use serde::{Deserialize, Serialize};

use crate::{
    CoreError, DiagnosticDetail, SourceHealthSnapshot, SourceHealthState, SourceSuspectReason,
};

use super::require_timestamp;
use crate::model::validate_source_health_evidence;

/// The source-suspect lifecycle owned by one persisted target state document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceHealth {
    state: SourceHealthState,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason_class: Option<SourceSuspectReason>,
    consecutive_unresolved: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_unresolved_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_details: Option<DiagnosticDetail>,
}

impl SourceHealth {
    pub(super) const fn healthy() -> Self {
        Self {
            state: SourceHealthState::Healthy,
            reason_class: None,
            consecutive_unresolved: 0,
            first_unresolved_at: None,
            last_details: None,
        }
    }

    /// Stages source-suspect evidence and returns whether the escalation threshold was reached.
    pub(super) fn apply_suspect(
        &mut self,
        reason_class: SourceSuspectReason,
        details: DiagnosticDetail,
        now: &str,
        escalate_after: u32,
    ) -> Result<bool, CoreError> {
        require_timestamp("source-health timestamp", now)?;
        if escalate_after == 0 {
            return Err(CoreError::contract("escalate_after must be positive"));
        }
        validate_source_health_evidence(reason_class, &details)?;
        if self.reason_class == Some(reason_class) {
            self.consecutive_unresolved = self
                .consecutive_unresolved
                .checked_add(1)
                .ok_or_else(|| CoreError::contract("source-health unresolved count overflow"))?;
        } else {
            *self = Self {
                state: SourceHealthState::Suspect,
                reason_class: Some(reason_class),
                consecutive_unresolved: 1,
                first_unresolved_at: Some(now.to_owned()),
                last_details: None,
            };
        }
        self.state = SourceHealthState::Suspect;
        self.last_details = Some(details);
        Ok(self.consecutive_unresolved == escalate_after)
    }

    pub(super) fn episode_started_at(&self, reason: SourceSuspectReason) -> Option<&str> {
        (self.reason_class == Some(reason))
            .then_some(self.first_unresolved_at.as_deref())
            .flatten()
    }

    pub(super) fn snapshot(&self) -> Result<SourceHealthSnapshot, CoreError> {
        SourceHealthSnapshot::new(
            self.state,
            self.reason_class,
            self.consecutive_unresolved,
            self.first_unresolved_at.clone(),
            self.last_details.clone(),
        )
    }

    pub(super) fn validate(&self) -> Result<(), CoreError> {
        self.snapshot().map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{htmlcut_detail, io_detail, plain_detail};
    use crate::{
        HtmlcutDiagnosticCode, HtmlcutErrorClass, HtmlcutFailureDetails, IoErrorClass,
        SourceSuspectReason,
    };

    use crate::model::source_health_detail_matches_reason_for_test as source_health_detail_matches_reason;
    use crate::{DiagnosticKind, DiagnosticOperation};

    fn htmlcut(error_class: HtmlcutErrorClass) -> crate::DiagnosticDetail {
        htmlcut_detail(
            "HTMLCut could not produce a selected value",
            HtmlcutFailureDetails::new(error_class, None, "a".repeat(64), Vec::new()),
            None,
        )
    }

    fn htmlcut_match_index() -> crate::DiagnosticDetail {
        htmlcut_detail(
            "HTMLCut could not select the configured candidate index",
            HtmlcutFailureDetails::new(
                HtmlcutErrorClass::NoMatch,
                None,
                "a".repeat(64),
                Vec::new(),
            )
            .with_core_diagnostic_code(HtmlcutDiagnosticCode::MatchIndexOutOfRange),
            None,
        )
    }

    #[test]
    fn source_health_reason_matching_requires_each_closed_kind_operation_and_htmlcut_fact() {
        for reason in [
            SourceSuspectReason::FetchFailed,
            SourceSuspectReason::JsonMalformed,
            SourceSuspectReason::JsonMissingPointerTarget,
            SourceSuspectReason::JsonNonScalarPointerTarget,
            SourceSuspectReason::ValueUnparseable,
            SourceSuspectReason::HtmlcutNoMatch,
            SourceSuspectReason::HtmlcutAmbiguousMatch,
            SourceSuspectReason::HtmlcutMissingAttribute,
            SourceSuspectReason::HtmlcutMatchIndexOutOfRange,
        ] {
            assert!(!source_health_detail_matches_reason(
                reason,
                &plain_detail(
                    DiagnosticKind::Contract,
                    DiagnosticOperation::TargetLoad,
                    "unrelated diagnostic",
                    None,
                ),
            ));
        }

        assert!(source_health_detail_matches_reason(
            SourceSuspectReason::FetchFailed,
            &io_detail(
                IoErrorClass::NotFound,
                DiagnosticOperation::FileRead,
                "file source unavailable",
                None,
            ),
        ));
        assert!(source_health_detail_matches_reason(
            SourceSuspectReason::FetchFailed,
            &io_detail(
                IoErrorClass::ConnectionRefused,
                DiagnosticOperation::HttpFetch,
                "HTTP source unavailable",
                None,
            ),
        ));
        assert!(source_health_detail_matches_reason(
            SourceSuspectReason::JsonMalformed,
            &plain_detail(
                DiagnosticKind::Json,
                DiagnosticOperation::JsonPointerSelection,
                "JSON selection failed",
                None,
            ),
        ));
        assert!(source_health_detail_matches_reason(
            SourceSuspectReason::ValueUnparseable,
            &plain_detail(
                DiagnosticKind::ValueUnparseable,
                DiagnosticOperation::ValueParse,
                "value parsing failed",
                None,
            ),
        ));
        for reason in [
            SourceSuspectReason::HtmlcutNoMatch,
            SourceSuspectReason::HtmlcutAmbiguousMatch,
            SourceSuspectReason::HtmlcutMissingAttribute,
            SourceSuspectReason::HtmlcutMatchIndexOutOfRange,
        ] {
            assert!(!source_health_detail_matches_reason(
                reason,
                &crate::model::plain_detail(
                    DiagnosticKind::Htmlcut,
                    DiagnosticOperation::TargetLoad,
                    "HTMLCut operation did not run",
                    None,
                ),
            ));
        }
        assert!(source_health_detail_matches_reason(
            SourceSuspectReason::HtmlcutNoMatch,
            &htmlcut(HtmlcutErrorClass::NoMatch),
        ));
        assert!(source_health_detail_matches_reason(
            SourceSuspectReason::HtmlcutAmbiguousMatch,
            &htmlcut(HtmlcutErrorClass::AmbiguousMatch),
        ));
        assert!(source_health_detail_matches_reason(
            SourceSuspectReason::HtmlcutMissingAttribute,
            &htmlcut(HtmlcutErrorClass::MissingAttribute),
        ));
        assert!(source_health_detail_matches_reason(
            SourceSuspectReason::HtmlcutMatchIndexOutOfRange,
            &htmlcut_match_index(),
        ));
    }

    #[test]
    fn htmlcut_source_health_causality_is_exact_across_every_closed_failure_class() {
        let evidence = [
            (
                SourceSuspectReason::HtmlcutNoMatch,
                htmlcut(HtmlcutErrorClass::NoMatch),
            ),
            (
                SourceSuspectReason::HtmlcutAmbiguousMatch,
                htmlcut(HtmlcutErrorClass::AmbiguousMatch),
            ),
            (
                SourceSuspectReason::HtmlcutMissingAttribute,
                htmlcut(HtmlcutErrorClass::MissingAttribute),
            ),
            (
                SourceSuspectReason::HtmlcutMatchIndexOutOfRange,
                htmlcut_match_index(),
            ),
        ];
        for reason in [
            SourceSuspectReason::HtmlcutNoMatch,
            SourceSuspectReason::HtmlcutAmbiguousMatch,
            SourceSuspectReason::HtmlcutMissingAttribute,
            SourceSuspectReason::HtmlcutMatchIndexOutOfRange,
        ] {
            for (owned_reason, detail) in &evidence {
                assert_eq!(
                    source_health_detail_matches_reason(reason, detail),
                    reason == *owned_reason,
                    "{reason:?} must not accept evidence for {owned_reason:?}",
                );
            }
        }
    }
}

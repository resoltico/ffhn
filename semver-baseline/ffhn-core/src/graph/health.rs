//! Durable source-acquisition and measurement-extraction episode state.

use serde::{Deserialize, Serialize};

use crate::CoreError;

use super::{SourceFetchFailure, SourceFetchFailureKind};

/// Closed state of an acquisition or extraction health episode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    /// No unresolved episode is active.
    Healthy,
    /// An unresolved source or extraction failure episode is active.
    Suspect,
}

/// Source health is limited to typed acquisition failures.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAcquisitionHealth {
    state: HealthState,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_kind: Option<SourceFetchFailureKind>,
    consecutive_unresolved: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_unresolved_at_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_failure: Option<SourceFetchFailure>,
}

impl SourceAcquisitionHealth {
    /// Creates the absence of any source acquisition-health episode.
    pub const fn healthy() -> Self {
        Self {
            state: HealthState::Healthy,
            failure_kind: None,
            consecutive_unresolved: 0,
            first_unresolved_at_utc: None,
            last_failure: None,
        }
    }

    /// Records one failure and returns whether its episode just reached escalation.
    pub fn observe(
        &mut self,
        failure: SourceFetchFailure,
        now_utc: &str,
        escalate_after: u32,
    ) -> Result<bool, CoreError> {
        failure.validate()?;
        require_timestamp("source acquisition-health time", now_utc)?;
        if escalate_after == 0 {
            return Err(CoreError::contract(
                "source escalate_after must be positive",
            ));
        }
        if self.failure_kind == Some(failure.kind) {
            self.consecutive_unresolved =
                self.consecutive_unresolved.checked_add(1).ok_or_else(|| {
                    CoreError::contract("source acquisition-health unresolved count overflowed")
                })?;
        } else {
            *self = Self {
                state: HealthState::Suspect,
                failure_kind: Some(failure.kind),
                consecutive_unresolved: 1,
                first_unresolved_at_utc: Some(now_utc.to_owned()),
                last_failure: None,
            };
        }
        self.state = HealthState::Suspect;
        self.last_failure = Some(failure);
        self.validate()?;
        Ok(self.consecutive_unresolved == escalate_after)
    }

    /// Clears the source episode after a complete accepted representation or valid `304`.
    pub fn clear(&mut self) {
        *self = Self::healthy();
    }

    /// Returns the active failure kind, if any, for episode-key assignment.
    pub const fn failure_kind(&self) -> Option<SourceFetchFailureKind> {
        self.failure_kind
    }

    /// Validates the complete source-health episode shape.
    pub fn validate(&self) -> Result<(), CoreError> {
        match (
            self.state,
            self.failure_kind,
            self.consecutive_unresolved,
            self.first_unresolved_at_utc.as_deref(),
            self.last_failure.as_ref(),
        ) {
            (HealthState::Healthy, None, 0, None, None) => Ok(()),
            (HealthState::Suspect, Some(kind), count, Some(first), Some(last)) if count > 0 => {
                require_timestamp("source acquisition-health first time", first)?;
                last.validate()?;
                if last.kind == kind {
                    Ok(())
                } else {
                    Err(CoreError::contract(
                        "source acquisition-health failure kind differs from last failure",
                    ))
                }
            }
            _ => Err(CoreError::contract(
                "source acquisition-health episode fields disagree",
            )),
        }
    }
}

/// Closed measurement-local extraction or parse failure vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionFailureReason {
    /// The source body was not valid JSON for this measurement's JSON projection.
    JsonMalformed,
    /// The JSON pointer selected no value.
    JsonMissingPointerTarget,
    /// The JSON pointer selected an array or object rather than a scalar.
    JsonNonScalarPointerTarget,
    /// HTMLCut selected no candidate.
    HtmlcutNoMatch,
    /// HTMLCut selected more than one candidate for an exact-one projection.
    HtmlcutAmbiguousMatch,
    /// The selected HTML candidate lacked the requested attribute.
    HtmlcutMissingAttribute,
    /// The requested selected-match index did not exist.
    HtmlcutMatchIndexOutOfRange,
    /// The selected scalar did not match the declared typed-value grammar.
    ValueUnparseable,
}

impl ExtractionFailureReason {
    /// Returns the stable event-key and report spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JsonMalformed => "json_malformed",
            Self::JsonMissingPointerTarget => "json_missing_pointer_target",
            Self::JsonNonScalarPointerTarget => "json_non_scalar_pointer_target",
            Self::HtmlcutNoMatch => "htmlcut_no_match",
            Self::HtmlcutAmbiguousMatch => "htmlcut_ambiguous_match",
            Self::HtmlcutMissingAttribute => "htmlcut_missing_attribute",
            Self::HtmlcutMatchIndexOutOfRange => "htmlcut_match_index_out_of_range",
            Self::ValueUnparseable => "value_unparseable",
        }
    }
}

/// Measurement-local extraction health, intentionally independent of source acquisition health.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeasurementExtractionHealth {
    state: HealthState,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<ExtractionFailureReason>,
    consecutive_unresolved: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_unresolved_at_utc: Option<String>,
}

impl MeasurementExtractionHealth {
    /// Creates the absence of an extraction-health episode.
    pub const fn healthy() -> Self {
        Self {
            state: HealthState::Healthy,
            reason: None,
            consecutive_unresolved: 0,
            first_unresolved_at_utc: None,
        }
    }

    /// Records one extraction failure and returns whether the episode just reached escalation.
    pub fn observe(
        &mut self,
        reason: ExtractionFailureReason,
        now_utc: &str,
        escalate_after: u32,
    ) -> Result<bool, CoreError> {
        require_timestamp("measurement extraction-health time", now_utc)?;
        if escalate_after == 0 {
            return Err(CoreError::contract(
                "measurement escalate_after must be positive",
            ));
        }
        if self.reason == Some(reason) {
            self.consecutive_unresolved =
                self.consecutive_unresolved.checked_add(1).ok_or_else(|| {
                    CoreError::contract("measurement extraction-health unresolved count overflowed")
                })?;
        } else {
            *self = Self {
                state: HealthState::Suspect,
                reason: Some(reason),
                consecutive_unresolved: 1,
                first_unresolved_at_utc: Some(now_utc.to_owned()),
            };
        }
        self.state = HealthState::Suspect;
        self.validate()?;
        Ok(self.consecutive_unresolved == escalate_after)
    }

    /// Clears the extraction episode after an accepted typed observation.
    pub fn clear(&mut self) {
        *self = Self::healthy();
    }

    /// Returns the active extraction reason, if any, for episode-key assignment.
    pub const fn reason(&self) -> Option<ExtractionFailureReason> {
        self.reason
    }

    /// Validates the complete extraction-health episode shape.
    pub fn validate(&self) -> Result<(), CoreError> {
        match (
            self.state,
            self.reason,
            self.consecutive_unresolved,
            self.first_unresolved_at_utc.as_deref(),
        ) {
            (HealthState::Healthy, None, 0, None) => Ok(()),
            (HealthState::Suspect, Some(_), count, Some(first)) if count > 0 => {
                require_timestamp("measurement extraction-health first time", first)
            }
            _ => Err(CoreError::contract(
                "measurement extraction-health episode fields disagree",
            )),
        }
    }
}

/// Closed FFHN/HTMLCut integration-fault vocabulary, independent of health episodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphIntegrationFaultCode {
    /// A source header secret environment variable was unavailable.
    SecretUnavailable,
    /// HTMLCut reported its closed internal-error category.
    HtmlcutInternalError,
    /// FFHN observed a violated integration-boundary invariant.
    FfhnBoundaryInvariantViolation,
    /// FFHN could not uphold the exact-policy fixed-width proof.
    FfhnPolicyInvariantViolation,
}

impl GraphIntegrationFaultCode {
    /// Returns the stable event-key and report spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SecretUnavailable => "secret_unavailable",
            Self::HtmlcutInternalError => "htmlcut_internal_error",
            Self::FfhnBoundaryInvariantViolation => "ffhn_boundary_invariant_violation",
            Self::FfhnPolicyInvariantViolation => "ffhn_policy_invariant_violation",
        }
    }
}

/// One code-keyed durable integration-fault episode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationFaultEpisode {
    code: GraphIntegrationFaultCode,
    first_seen_at_utc: String,
}

impl IntegrationFaultEpisode {
    /// Creates one code-keyed episode with validated UTC evidence time.
    pub fn new(
        code: GraphIntegrationFaultCode,
        first_seen_at_utc: String,
    ) -> Result<Self, CoreError> {
        let episode = Self {
            code,
            first_seen_at_utc,
        };
        episode.validate()?;
        Ok(episode)
    }

    /// Returns the stable fault code.
    pub const fn code(&self) -> GraphIntegrationFaultCode {
        self.code
    }

    /// Validates immutable code-keyed episode evidence.
    pub fn validate(&self) -> Result<(), CoreError> {
        require_timestamp("integration-fault first-seen time", &self.first_seen_at_utc)
    }
}

fn require_timestamp(field: &str, value: &str) -> Result<(), CoreError> {
    crate::model::require_canonical_utc_rfc3339(field, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_and_measurement_health_are_scoped_and_restart_only_on_reason_change() {
        let mut source = SourceAcquisitionHealth::healthy();
        let fetch = SourceFetchFailure {
            kind: SourceFetchFailureKind::HttpStatus,
            status: Some(500),
            raw_platform_error: None,
        };
        assert!(
            !source
                .observe(fetch.clone(), "2026-08-25T00:00:00Z", 2)
                .expect("first failure")
        );
        assert!(
            source
                .observe(fetch, "2026-08-25T00:01:00Z", 2)
                .expect("second failure")
        );

        let mut extraction = MeasurementExtractionHealth::healthy();
        assert!(
            !extraction
                .observe(
                    ExtractionFailureReason::JsonMalformed,
                    "2026-08-25T00:00:00Z",
                    2,
                )
                .expect("first extraction failure")
        );
        assert!(
            !extraction
                .observe(
                    ExtractionFailureReason::ValueUnparseable,
                    "2026-08-25T00:01:00Z",
                    2,
                )
                .expect("changed extraction reason")
        );
    }

    #[test]
    fn health_value_objects_cover_closed_vocabularies_clears_and_invalid_episode_shapes() {
        for (reason, spelling) in [
            (ExtractionFailureReason::JsonMalformed, "json_malformed"),
            (
                ExtractionFailureReason::JsonMissingPointerTarget,
                "json_missing_pointer_target",
            ),
            (
                ExtractionFailureReason::JsonNonScalarPointerTarget,
                "json_non_scalar_pointer_target",
            ),
            (ExtractionFailureReason::HtmlcutNoMatch, "htmlcut_no_match"),
            (
                ExtractionFailureReason::HtmlcutAmbiguousMatch,
                "htmlcut_ambiguous_match",
            ),
            (
                ExtractionFailureReason::HtmlcutMissingAttribute,
                "htmlcut_missing_attribute",
            ),
            (
                ExtractionFailureReason::HtmlcutMatchIndexOutOfRange,
                "htmlcut_match_index_out_of_range",
            ),
            (
                ExtractionFailureReason::ValueUnparseable,
                "value_unparseable",
            ),
        ] {
            assert_eq!(reason.as_str(), spelling);
        }
        for (code, spelling) in [
            (
                GraphIntegrationFaultCode::SecretUnavailable,
                "secret_unavailable",
            ),
            (
                GraphIntegrationFaultCode::HtmlcutInternalError,
                "htmlcut_internal_error",
            ),
            (
                GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation,
                "ffhn_boundary_invariant_violation",
            ),
            (
                GraphIntegrationFaultCode::FfhnPolicyInvariantViolation,
                "ffhn_policy_invariant_violation",
            ),
        ] {
            assert_eq!(code.as_str(), spelling);
            let episode = IntegrationFaultEpisode::new(code, "2026-08-25T00:00:00Z".to_owned())
                .expect("episode");
            assert_eq!(episode.code(), code);
            episode.validate().expect("episode validation");
        }
        assert!(
            IntegrationFaultEpisode::new(
                GraphIntegrationFaultCode::SecretUnavailable,
                "bad".to_owned(),
            )
            .is_err()
        );

        let failure = SourceFetchFailure {
            kind: SourceFetchFailureKind::HttpStatus,
            status: Some(500),
            raw_platform_error: None,
        };
        let mut source = SourceAcquisitionHealth::healthy();
        assert_eq!(source.failure_kind(), None);
        source.validate().expect("healthy source");
        assert!(source.observe(failure.clone(), "bad", 1).is_err());
        assert!(
            source
                .observe(failure.clone(), "2026-08-25T00:00:00Z", 0)
                .is_err()
        );
        source
            .observe(failure, "2026-08-25T00:00:00Z", 2)
            .expect("failure");
        assert_eq!(
            source.failure_kind(),
            Some(SourceFetchFailureKind::HttpStatus)
        );
        source.clear();
        assert_eq!(source, SourceAcquisitionHealth::healthy());

        let mut overflowed_source = SourceAcquisitionHealth {
            state: HealthState::Suspect,
            failure_kind: Some(SourceFetchFailureKind::HttpStatus),
            consecutive_unresolved: u32::MAX,
            first_unresolved_at_utc: Some("2026-08-25T00:00:00Z".to_owned()),
            last_failure: Some(SourceFetchFailure {
                kind: SourceFetchFailureKind::HttpStatus,
                status: Some(500),
                raw_platform_error: None,
            }),
        };
        assert!(
            overflowed_source
                .observe(
                    SourceFetchFailure {
                        kind: SourceFetchFailureKind::HttpStatus,
                        status: Some(500),
                        raw_platform_error: None,
                    },
                    "2026-08-25T00:01:00Z",
                    1,
                )
                .is_err()
        );
        let crossed_source = SourceAcquisitionHealth {
            state: HealthState::Suspect,
            failure_kind: Some(SourceFetchFailureKind::InvalidUtf8),
            consecutive_unresolved: 1,
            first_unresolved_at_utc: Some("2026-08-25T00:00:00Z".to_owned()),
            last_failure: Some(SourceFetchFailure {
                kind: SourceFetchFailureKind::HttpStatus,
                status: Some(500),
                raw_platform_error: None,
            }),
        };
        assert!(crossed_source.validate().is_err());
        let incomplete_source = SourceAcquisitionHealth {
            state: HealthState::Healthy,
            failure_kind: Some(SourceFetchFailureKind::InvalidUtf8),
            consecutive_unresolved: 0,
            first_unresolved_at_utc: None,
            last_failure: None,
        };
        assert!(incomplete_source.validate().is_err());
        let zero_source = SourceAcquisitionHealth {
            state: HealthState::Suspect,
            failure_kind: Some(SourceFetchFailureKind::HttpStatus),
            consecutive_unresolved: 0,
            first_unresolved_at_utc: Some("2026-08-25T00:00:00Z".to_owned()),
            last_failure: Some(SourceFetchFailure {
                kind: SourceFetchFailureKind::HttpStatus,
                status: Some(500),
                raw_platform_error: None,
            }),
        };
        assert!(zero_source.validate().is_err());

        let mut extraction = MeasurementExtractionHealth::healthy();
        assert_eq!(extraction.reason(), None);
        extraction.validate().expect("healthy extraction");
        assert!(
            extraction
                .observe(ExtractionFailureReason::JsonMalformed, "bad", 1)
                .is_err()
        );
        assert!(
            extraction
                .observe(
                    ExtractionFailureReason::JsonMalformed,
                    "2026-08-25T00:00:00Z",
                    0
                )
                .is_err()
        );
        extraction
            .observe(
                ExtractionFailureReason::JsonMalformed,
                "2026-08-25T00:00:00Z",
                2,
            )
            .expect("failure");
        assert_eq!(
            extraction.reason(),
            Some(ExtractionFailureReason::JsonMalformed)
        );
        extraction.clear();
        assert_eq!(extraction, MeasurementExtractionHealth::healthy());
        let mut overflowed_extraction = MeasurementExtractionHealth {
            state: HealthState::Suspect,
            reason: Some(ExtractionFailureReason::JsonMalformed),
            consecutive_unresolved: u32::MAX,
            first_unresolved_at_utc: Some("2026-08-25T00:00:00Z".to_owned()),
        };
        assert!(
            overflowed_extraction
                .observe(
                    ExtractionFailureReason::JsonMalformed,
                    "2026-08-25T00:01:00Z",
                    1,
                )
                .is_err()
        );
        let invalid_extraction = MeasurementExtractionHealth {
            state: HealthState::Suspect,
            reason: None,
            consecutive_unresolved: 1,
            first_unresolved_at_utc: Some("2026-08-25T00:00:00Z".to_owned()),
        };
        assert!(invalid_extraction.validate().is_err());
        let zero_extraction = MeasurementExtractionHealth {
            state: HealthState::Suspect,
            reason: Some(ExtractionFailureReason::JsonMalformed),
            consecutive_unresolved: 0,
            first_unresolved_at_utc: Some("2026-08-25T00:00:00Z".to_owned()),
        };
        assert!(zero_extraction.validate().is_err());
    }
}

//! Closed lifecycle snapshots for report consumers.

use serde::{Deserialize, Deserializer, Serialize};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

use crate::{
    CoreError, DiagnosticDetail, DiagnosticKind, DiagnosticOperation, HtmlcutDiagnosticCode,
    HtmlcutErrorClass, IntegrationFaultCode, PermanentErrorCode, SourceSuspectReason,
};

/// The durable health state of one target source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceHealthState {
    /// The source has no unresolved source-suspect episode.
    Healthy,
    /// The source has an unresolved source-suspect episode.
    Suspect,
}

impl SourceHealthState {
    /// Returns the stable serialized spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Suspect => "suspect",
        }
    }
}

/// The source-health axis of one lifecycle snapshot.
///
/// Optional facts serialize as `null` when this axis is healthy, so every snapshot exposes the
/// complete source-health vocabulary rather than omitting facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceHealthSnapshot {
    state: SourceHealthState,
    reason_class: Option<SourceSuspectReason>,
    consecutive_unresolved: u32,
    first_unresolved_at: Option<String>,
    last_details: Option<DiagnosticDetail>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceHealthSnapshotWire {
    state: SourceHealthState,
    reason_class: Option<SourceSuspectReason>,
    consecutive_unresolved: u32,
    first_unresolved_at: Option<String>,
    last_details: Option<DiagnosticDetail>,
}

impl<'de> Deserialize<'de> for SourceHealthSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SourceHealthSnapshotWire::deserialize(deserializer)?;
        Self::new(
            wire.state,
            wire.reason_class,
            wire.consecutive_unresolved,
            wire.first_unresolved_at,
            wire.last_details,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl SourceHealthSnapshot {
    pub(crate) fn new(
        state: SourceHealthState,
        reason_class: Option<SourceSuspectReason>,
        consecutive_unresolved: u32,
        first_unresolved_at: Option<String>,
        last_details: Option<DiagnosticDetail>,
    ) -> Result<Self, CoreError> {
        validate_source_health_facts(
            state,
            reason_class,
            consecutive_unresolved,
            first_unresolved_at.as_deref(),
            last_details.as_ref(),
        )?;
        Ok(Self {
            state,
            reason_class,
            consecutive_unresolved,
            first_unresolved_at,
            last_details,
        })
    }

    /// Returns the durable source-health classification.
    pub const fn state(&self) -> SourceHealthState {
        self.state
    }

    /// Returns the closed reason class when the source is suspect.
    pub const fn reason_class(&self) -> Option<SourceSuspectReason> {
        self.reason_class
    }

    /// Returns the count of consecutive unresolved failures in the current episode.
    pub const fn consecutive_unresolved(&self) -> u32 {
        self.consecutive_unresolved
    }

    /// Returns the first observed instant of the current source-suspect episode.
    pub fn first_unresolved_at(&self) -> Option<&str> {
        self.first_unresolved_at.as_deref()
    }

    /// Returns the latest diagnostic evidence retained for the current source-suspect episode.
    pub fn last_details(&self) -> Option<&DiagnosticDetail> {
        self.last_details.as_ref()
    }

    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        validate_source_health_facts(
            self.state,
            self.reason_class,
            self.consecutive_unresolved,
            self.first_unresolved_at.as_deref(),
            self.last_details.as_ref(),
        )
    }
}

/// The permanent configuration-error axis of one lifecycle snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PermanentErrorEpisodeSnapshot {
    error_code: PermanentErrorCode,
    first_seen_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PermanentErrorEpisodeSnapshotWire {
    error_code: PermanentErrorCode,
    first_seen_at: String,
}

impl<'de> Deserialize<'de> for PermanentErrorEpisodeSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = PermanentErrorEpisodeSnapshotWire::deserialize(deserializer)?;
        Self::new(wire.error_code, wire.first_seen_at).map_err(serde::de::Error::custom)
    }
}

impl PermanentErrorEpisodeSnapshot {
    pub(crate) fn new(
        error_code: PermanentErrorCode,
        first_seen_at: String,
    ) -> Result<Self, CoreError> {
        require_canonical_utc_rfc3339("permanent-error first-seen timestamp", &first_seen_at)?;
        Ok(Self {
            error_code,
            first_seen_at,
        })
    }

    /// Returns the closed permanent-error classification.
    pub const fn error_code(&self) -> PermanentErrorCode {
        self.error_code
    }

    /// Returns the first observed instant of this permanent-error episode.
    pub fn first_seen_at(&self) -> &str {
        &self.first_seen_at
    }

    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        require_canonical_utc_rfc3339("permanent-error first-seen timestamp", &self.first_seen_at)
    }
}

/// The FFHN/adapter integration-fault axis of one lifecycle snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationFaultEpisodeSnapshot {
    code: IntegrationFaultCode,
    first_seen_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntegrationFaultEpisodeSnapshotWire {
    code: IntegrationFaultCode,
    first_seen_at: String,
}

impl<'de> Deserialize<'de> for IntegrationFaultEpisodeSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = IntegrationFaultEpisodeSnapshotWire::deserialize(deserializer)?;
        Self::new(wire.code, wire.first_seen_at).map_err(serde::de::Error::custom)
    }
}

impl IntegrationFaultEpisodeSnapshot {
    pub(crate) fn new(
        code: IntegrationFaultCode,
        first_seen_at: String,
    ) -> Result<Self, CoreError> {
        require_canonical_utc_rfc3339("integration-fault first-seen timestamp", &first_seen_at)?;
        Ok(Self {
            code,
            first_seen_at,
        })
    }

    /// Returns the closed FFHN/adapter integration-fault classification.
    pub const fn code(&self) -> IntegrationFaultCode {
        self.code
    }

    /// Returns the first observed instant of this integration-fault episode.
    pub fn first_seen_at(&self) -> &str {
        &self.first_seen_at
    }

    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        require_canonical_utc_rfc3339(
            "integration-fault first-seen timestamp",
            &self.first_seen_at,
        )
    }
}

/// One complete durable lifecycle snapshot, with no event eligibility or delivery facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleSnapshot {
    source_health: SourceHealthSnapshot,
    permanent_error_episode: Option<PermanentErrorEpisodeSnapshot>,
    integration_fault_episode: Option<IntegrationFaultEpisodeSnapshot>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecycleSnapshotWire {
    source_health: SourceHealthSnapshot,
    permanent_error_episode: Option<PermanentErrorEpisodeSnapshot>,
    integration_fault_episode: Option<IntegrationFaultEpisodeSnapshot>,
}

impl<'de> Deserialize<'de> for LifecycleSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = LifecycleSnapshotWire::deserialize(deserializer)?;
        Self::new(
            wire.source_health,
            wire.permanent_error_episode,
            wire.integration_fault_episode,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl LifecycleSnapshot {
    pub(crate) fn new(
        source_health: SourceHealthSnapshot,
        permanent_error_episode: Option<PermanentErrorEpisodeSnapshot>,
        integration_fault_episode: Option<IntegrationFaultEpisodeSnapshot>,
    ) -> Result<Self, CoreError> {
        source_health.validate()?;
        if let Some(episode) = &permanent_error_episode {
            episode.validate()?;
        }
        if let Some(episode) = &integration_fault_episode {
            episode.validate()?;
        }
        Ok(Self {
            source_health,
            permanent_error_episode,
            integration_fault_episode,
        })
    }

    /// Returns the complete durable source-health axis.
    pub const fn source_health(&self) -> &SourceHealthSnapshot {
        &self.source_health
    }

    /// Returns the active permanent-error episode, when one exists.
    pub const fn permanent_error_episode(&self) -> Option<&PermanentErrorEpisodeSnapshot> {
        self.permanent_error_episode.as_ref()
    }

    /// Returns the active FFHN/adapter integration-fault episode, when one exists.
    pub const fn integration_fault_episode(&self) -> Option<&IntegrationFaultEpisodeSnapshot> {
        self.integration_fault_episode.as_ref()
    }

    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        self.source_health.validate()?;
        if let Some(episode) = &self.permanent_error_episode {
            episode.validate()?;
        }
        if let Some(episode) = &self.integration_fault_episode {
            episode.validate()?;
        }
        Ok(())
    }
}

/// The lifecycle facet of a run report.
///
/// `before` is a durable state read under the target lock. `after` is only a staged transition;
/// [`crate::RunReport::state_persisted`] says whether that transition committed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleFacet {
    before: Option<LifecycleSnapshot>,
    after: Option<LifecycleSnapshot>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecycleFacetWire {
    before: Option<LifecycleSnapshot>,
    after: Option<LifecycleSnapshot>,
}

impl<'de> Deserialize<'de> for LifecycleFacet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = LifecycleFacetWire::deserialize(deserializer)?;
        let facet = Self::new(wire.before, wire.after);
        facet.validate().map_err(serde::de::Error::custom)?;
        Ok(facet)
    }
}

impl LifecycleFacet {
    pub(crate) const fn new(
        before: Option<LifecycleSnapshot>,
        after: Option<LifecycleSnapshot>,
    ) -> Self {
        Self { before, after }
    }

    /// Returns the durable lifecycle snapshot read before this run, when safely available.
    pub const fn before(&self) -> Option<&LifecycleSnapshot> {
        self.before.as_ref()
    }

    /// Returns the lifecycle snapshot staged by this run, when it staged a state transition.
    pub const fn after(&self) -> Option<&LifecycleSnapshot> {
        self.after.as_ref()
    }

    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        if let Some(snapshot) = &self.before {
            snapshot.validate()?;
        }
        if let Some(snapshot) = &self.after {
            snapshot.validate()?;
        }
        Ok(())
    }
}

pub(crate) fn validate_source_health_evidence(
    reason: SourceSuspectReason,
    detail: &DiagnosticDetail,
) -> Result<(), CoreError> {
    detail.validate()?;
    if detail.integration_fault_code().is_some()
        || !source_health_detail_matches_reason(reason, detail)
    {
        return Err(CoreError::contract(
            "source-health reason and diagnostic evidence must describe the same source failure",
        ));
    }
    Ok(())
}

fn validate_source_health_facts(
    state: SourceHealthState,
    reason_class: Option<SourceSuspectReason>,
    consecutive_unresolved: u32,
    first_unresolved_at: Option<&str>,
    last_details: Option<&DiagnosticDetail>,
) -> Result<(), CoreError> {
    match state {
        SourceHealthState::Healthy
            if reason_class.is_none()
                && consecutive_unresolved == 0
                && first_unresolved_at.is_none()
                && last_details.is_none() =>
        {
            Ok(())
        }
        SourceHealthState::Suspect => {
            let (Some(reason), Some(first_seen), Some(details)) =
                (reason_class, first_unresolved_at, last_details)
            else {
                return Err(CoreError::contract(
                    "source-health facts do not match the declared health state",
                ));
            };
            if consecutive_unresolved == 0 {
                return Err(CoreError::contract(
                    "source-health facts do not match the declared health state",
                ));
            }
            require_canonical_utc_rfc3339("source-health first-unresolved timestamp", first_seen)?;
            validate_source_health_evidence(reason, details)
        }
        SourceHealthState::Healthy => Err(CoreError::contract(
            "source-health facts do not match the declared health state",
        )),
    }
}

fn source_health_detail_matches_reason(
    reason: SourceSuspectReason,
    detail: &DiagnosticDetail,
) -> bool {
    match reason {
        SourceSuspectReason::FetchFailed => {
            detail.kind() == DiagnosticKind::Io
                && matches!(
                    detail.operation(),
                    DiagnosticOperation::FileRead | DiagnosticOperation::HttpFetch
                )
        }
        SourceSuspectReason::JsonMalformed
        | SourceSuspectReason::JsonMissingPointerTarget
        | SourceSuspectReason::JsonNonScalarPointerTarget => {
            detail.kind() == DiagnosticKind::Json
                && detail.operation() == DiagnosticOperation::JsonPointerSelection
        }
        SourceSuspectReason::ValueUnparseable => {
            detail.kind() == DiagnosticKind::ValueUnparseable
                && detail.operation() == DiagnosticOperation::ValueParse
        }
        SourceSuspectReason::HtmlcutNoMatch => {
            detail.kind() == DiagnosticKind::Htmlcut
                && detail.operation() == DiagnosticOperation::HtmlExtraction
                && detail.htmlcut_failure().is_some_and(|failure| {
                    failure.error_class() == HtmlcutErrorClass::NoMatch
                        && failure.core_diagnostic_code()
                            != Some(HtmlcutDiagnosticCode::MatchIndexOutOfRange)
                })
        }
        SourceSuspectReason::HtmlcutAmbiguousMatch => {
            detail.kind() == DiagnosticKind::Htmlcut
                && detail.operation() == DiagnosticOperation::HtmlExtraction
                && detail.htmlcut_failure().is_some_and(|failure| {
                    failure.error_class() == HtmlcutErrorClass::AmbiguousMatch
                })
        }
        SourceSuspectReason::HtmlcutMissingAttribute => {
            detail.kind() == DiagnosticKind::Htmlcut
                && detail.operation() == DiagnosticOperation::HtmlExtraction
                && detail.htmlcut_failure().is_some_and(|failure| {
                    failure.error_class() == HtmlcutErrorClass::MissingAttribute
                })
        }
        SourceSuspectReason::HtmlcutMatchIndexOutOfRange => {
            detail.kind() == DiagnosticKind::Htmlcut
                && detail.operation() == DiagnosticOperation::HtmlExtraction
                && detail.htmlcut_failure().is_some_and(|failure| {
                    failure.error_class() == HtmlcutErrorClass::NoMatch
                        && failure.core_diagnostic_code()
                            == Some(HtmlcutDiagnosticCode::MatchIndexOutOfRange)
                })
        }
    }
}

#[cfg(test)]
pub(crate) fn source_health_detail_matches_reason_for_test(
    reason: SourceSuspectReason,
    detail: &DiagnosticDetail,
) -> bool {
    source_health_detail_matches_reason(reason, detail)
}

pub(crate) fn require_canonical_utc_rfc3339(field: &str, value: &str) -> Result<(), CoreError> {
    let timestamp = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| CoreError::contract(format!("{field} must be RFC 3339")))?;
    let canonical = timestamp
        .format(&Rfc3339)
        .map_err(|_| CoreError::internal("could not format timestamp"))?;
    if timestamp.offset() != UtcOffset::UTC || value != canonical {
        return Err(CoreError::contract(format!(
            "{field} must be canonical UTC RFC 3339"
        )));
    }
    Ok(())
}

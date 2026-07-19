//! Policy evaluation vocabulary: inputs, outputs, staged events, and transient context.

use serde::{Deserialize, Serialize};

use crate::{
    DeliveryEventKind, IntegrationFaultCode, Observation, PermanentErrorCode, RouteFamily,
    SourceSuspectReason,
};

use super::super::condition::{ConditionId, ConditionReference};

/// The result of evaluating one named condition against one valid observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOutcome {
    /// The predicate is true for the supplied pre-run context.
    Satisfied,
    /// The predicate is false for the supplied pre-run context.
    NotSatisfied,
    /// A required named reference is absent or cannot be compared without conversion.
    Unavailable,
    /// Exact policy arithmetic exceeded its supported representation.
    ArithmeticOverflow,
    /// A percentage predicate encountered a zero runtime reference.
    ZeroReference,
}

impl ConditionOutcome {
    /// Returns the stable report-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Satisfied => "satisfied",
            Self::NotSatisfied => "not_satisfied",
            Self::Unavailable => "unavailable",
            Self::ArithmeticOverflow => "arithmetic_overflow",
            Self::ZeroReference => "zero_reference",
        }
    }
}

/// The transient per-condition context supplied from the pre-run state.
#[derive(Clone, Copy, Debug)]
pub struct ConditionContext<'a> {
    pub(super) last_accepted_observation: Option<&'a Observation>,
    pub(super) fixed_initial_baseline: Option<&'a Observation>,
    pub(super) last_condition_transition: Option<&'a Observation>,
    pub(super) active: bool,
}

impl<'a> ConditionContext<'a> {
    /// Creates the full pre-run context for one currently evaluated condition.
    pub const fn new(
        last_accepted_observation: Option<&'a Observation>,
        fixed_initial_baseline: Option<&'a Observation>,
        last_condition_transition: Option<&'a Observation>,
        active: bool,
    ) -> Self {
        Self {
            last_accepted_observation,
            fixed_initial_baseline,
            last_condition_transition,
            active,
        }
    }

    pub(crate) const fn empty() -> Self {
        Self::new(None, None, None, false)
    }
}

/// One staged outcome with its trigger decision and next hysteresis state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionEvaluation {
    pub(super) condition_id: ConditionId,
    pub(super) outcome: ConditionOutcome,
    pub(super) trigger: bool,
    pub(super) active_before: bool,
    pub(super) active_after: bool,
    pub(super) reference_evidence: Option<ConditionReferenceEvidence>,
}

/// Evidence for the named pre-run reference used by one condition evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConditionReferenceEvidence {
    /// The configured reference existed and supplied this canonical value.
    Resolved {
        /// The configured pre-run reference selector.
        reference: ConditionReference,
        /// The exact canonical value used by the policy calculation.
        canonical_value: String,
    },
    /// The configured reference did not exist or could not be used under the target contract.
    Unavailable {
        /// The configured pre-run reference selector.
        reference: ConditionReference,
    },
}

impl ConditionEvaluation {
    pub(crate) const fn id(&self) -> &ConditionId {
        &self.condition_id
    }

    /// Returns the evaluated condition identifier.
    pub fn condition_id(&self) -> &str {
        self.condition_id.as_str()
    }

    /// Returns the exact condition outcome.
    pub const fn outcome(&self) -> ConditionOutcome {
        self.outcome
    }

    /// Returns whether this evaluation satisfies the predicate's trigger rule.
    pub const fn trigger(&self) -> bool {
        self.trigger
    }

    /// Returns whether this condition was active before evaluating the current observation.
    pub const fn active_before(&self) -> bool {
        self.active_before
    }

    /// Returns the staged hysteresis state after this evaluation.
    pub const fn active_after(&self) -> bool {
        self.active_after
    }

    /// Returns the configured-reference evidence when this predicate compares one pre-run value.
    pub fn reference_evidence(&self) -> Option<&ConditionReferenceEvidence> {
        self.reference_evidence.as_ref()
    }
}

/// A condition outcome that requires immediate `on_run` routing in a later delivery milestone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditionIssue {
    /// Exact policy arithmetic exceeded its supported representation.
    ArithmeticOverflow,
    /// A percentage predicate encountered a zero runtime reference.
    ZeroReference,
}

/// The cause that makes one immediate `on_run` event eligible for later durable delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OnRunEventCause {
    /// The operator completed a blind reset and began a fresh target-state lifecycle.
    Reset,
    /// The target accepted its first valid observation for the current clean state lifecycle.
    Initialized,
    /// One condition reached a reportable exact-arithmetic issue.
    ConditionIssue {
        /// Stable identifier of the condition that produced the issue.
        condition_id: ConditionId,
        /// The precise reportable issue.
        issue: ConditionIssue,
    },
    /// A source-suspect episode reached its configured escalation boundary.
    SourceSuspectEscalated {
        /// Stable reason classification for the source-suspect episode.
        reason_class: SourceSuspectReason,
    },
    /// A permanent contract-error episode began or changed identity.
    PermanentContractErrorEpisodeBegan {
        /// Stable classification for the permanent contract-error episode.
        error_code: PermanentErrorCode,
    },
    /// An FFHN/HTMLCut adapter-boundary fault episode began or changed identity.
    IntegrationFaultEpisodeBegan {
        /// Stable classification for the adapter-boundary fault episode.
        integration_fault_code: IntegrationFaultCode,
    },
}

/// A deterministic routing eligibility staged before M4 creates any durable outbox record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagedEventEligibility {
    /// A named condition satisfied its predicate-specific `on_condition` trigger rule.
    OnCondition {
        /// Stable identifier of the condition eligible for condition routing.
        condition_id: ConditionId,
    },
    /// An immediate run-level event is eligible for later `on_run` routing.
    OnRun {
        /// The policy or episode fact that made this run-level event eligible.
        cause: OnRunEventCause,
    },
}

/// A classified T0 input before any persistent mutation or delivery is allowed.
#[derive(Clone, Copy, Debug)]
pub enum PolicyRunInput<'a> {
    /// A permanent contract error, which must not advance accepted observations or conditions.
    PermanentContractError {
        /// Stable error classification for the permanent contract fault.
        error_code: PermanentErrorCode,
        /// Whether the staged permanent-error episode began during this run.
        episode_began: bool,
    },
    /// An adapter-boundary fault, which must not advance accepted observations or conditions.
    IntegrationFault {
        /// Stable integration-fault classification.
        integration_fault_code: IntegrationFaultCode,
        /// Whether the staged integration-fault episode began during this run.
        episode_began: bool,
    },
    /// A source-suspect failure, which must not advance accepted observations or conditions.
    SourceSuspect {
        /// Stable reason classification for the source-suspect fault.
        reason_class: SourceSuspectReason,
        /// Whether the staged source-health episode reached its escalation boundary.
        escalation_reached: bool,
    },
    /// A valid observation eligible for condition evaluation and later baseline advancement.
    ValidObservation {
        /// The valid typed observation eligible for condition evaluation.
        observation: &'a Observation,
    },
}

/// A side-effect-free T0 staging result for one classified run input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagedPolicyRun {
    /// The permanent branch intentionally stages no accepted-observation or condition mutation.
    PermanentContractError {
        /// Stable error classification copied from the staged input.
        error_code: PermanentErrorCode,
        /// Deterministic routing eligibility for the staged permanent-error episode.
        event_eligibilities: Vec<StagedEventEligibility>,
    },
    /// The integration branch intentionally stages no accepted-observation or condition mutation.
    IntegrationFault {
        /// Stable integration-fault classification copied from the staged input.
        integration_fault_code: IntegrationFaultCode,
        /// Deterministic routing eligibility for the staged integration-fault episode.
        event_eligibilities: Vec<StagedEventEligibility>,
    },
    /// The source-suspect branch intentionally stages no accepted-observation or condition mutation.
    SourceSuspect {
        /// Stable source-suspect reason copied from the staged input.
        reason_class: SourceSuspectReason,
        /// Deterministic routing eligibility for the staged source-suspect episode.
        event_eligibilities: Vec<StagedEventEligibility>,
    },
    /// The valid branch stages all policy outcomes before later state/outbox work.
    ValidObservation {
        /// One staged evaluation for each configured named condition.
        condition_evaluations: Vec<ConditionEvaluation>,
        /// Deterministic routing eligibility from the staged condition outcomes.
        event_eligibilities: Vec<StagedEventEligibility>,
    },
}

impl StagedPolicyRun {
    /// Returns the staged condition evaluations for a valid observation input.
    pub(crate) fn condition_evaluations(&self) -> Option<&[ConditionEvaluation]> {
        match self {
            Self::ValidObservation {
                condition_evaluations,
                ..
            } => Some(condition_evaluations),
            Self::PermanentContractError { .. }
            | Self::IntegrationFault { .. }
            | Self::SourceSuspect { .. } => None,
        }
    }

    /// Returns the deterministic routing eligibilities that M4 must materialize without re-evaluation.
    pub fn event_eligibilities(&self) -> &[StagedEventEligibility] {
        match self {
            Self::PermanentContractError {
                event_eligibilities,
                ..
            }
            | Self::IntegrationFault {
                event_eligibilities,
                ..
            }
            | Self::SourceSuspect {
                event_eligibilities,
                ..
            }
            | Self::ValidObservation {
                event_eligibilities,
                ..
            } => event_eligibilities,
        }
    }
}

impl StagedEventEligibility {
    /// Returns the route family that would receive this event when matching routes exist.
    pub(crate) const fn route_family(&self) -> RouteFamily {
        match self {
            Self::OnCondition { .. } => RouteFamily::OnCondition,
            Self::OnRun { .. } => RouteFamily::OnRun,
        }
    }

    /// Returns the stable delivery event kind represented by this policy fact.
    pub(crate) const fn event_kind(&self) -> DeliveryEventKind {
        match self {
            Self::OnCondition { .. } => DeliveryEventKind::ConditionSatisfied,
            Self::OnRun { cause } => match cause {
                OnRunEventCause::Reset => DeliveryEventKind::Reset,
                OnRunEventCause::Initialized => DeliveryEventKind::Initialized,
                OnRunEventCause::ConditionIssue { issue, .. } => match issue {
                    ConditionIssue::ArithmeticOverflow => DeliveryEventKind::ArithmeticOverflow,
                    ConditionIssue::ZeroReference => DeliveryEventKind::ZeroReference,
                },
                OnRunEventCause::SourceSuspectEscalated { .. } => {
                    DeliveryEventKind::SourceSuspectEscalated
                }
                OnRunEventCause::PermanentContractErrorEpisodeBegan { .. } => {
                    DeliveryEventKind::PermanentContractError
                }
                OnRunEventCause::IntegrationFaultEpisodeBegan { .. } => {
                    DeliveryEventKind::IntegrationFault
                }
            },
        }
    }
}

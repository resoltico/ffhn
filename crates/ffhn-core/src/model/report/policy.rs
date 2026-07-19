//! Public evidence for the policy work staged during one run.

use serde::{Deserialize, Serialize};

use crate::{
    ConditionEvaluation, ConditionOutcome, ConditionReference, ConditionReferenceEvidence,
    DeliveryEventKind, IntegrationFaultCode, OnRunEventCause, PermanentErrorCode, RouteFamily,
    SourceSuspectReason, StagedEventEligibility,
};

/// The condition-evaluation and route-independent event facts staged for one run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum PolicyEvaluation {
    /// A valid typed observation was evaluated against every configured named condition.
    Evaluated {
        /// One result for every configured named condition, in target configuration order.
        condition_results: Vec<PolicyConditionResult>,
        /// Domain events eligible for matching delivery routes, independent of route configuration.
        event_eligibilities: Vec<PolicyEventEligibility>,
    },
    /// The run did not reach valid-observation condition evaluation.
    NotEvaluated {
        /// Domain events eligible for matching delivery routes, independent of route configuration.
        event_eligibilities: Vec<PolicyEventEligibility>,
    },
}

impl PolicyEvaluation {
    pub(crate) fn not_evaluated() -> Self {
        Self::NotEvaluated {
            event_eligibilities: Vec::new(),
        }
    }

    pub(crate) fn from_staged(
        evaluations: Option<&[ConditionEvaluation]>,
        eligibilities: &[StagedEventEligibility],
    ) -> Self {
        let event_eligibilities = eligibilities
            .iter()
            .map(PolicyEventEligibility::from_staged)
            .collect();
        match evaluations {
            Some(evaluations) => Self::Evaluated {
                condition_results: evaluations
                    .iter()
                    .map(PolicyConditionResult::from_evaluation)
                    .collect(),
                event_eligibilities,
            },
            None => Self::NotEvaluated {
                event_eligibilities,
            },
        }
    }

    /// Returns whether the run reached named-condition evaluation.
    pub const fn is_evaluated(&self) -> bool {
        matches!(self, Self::Evaluated { .. })
    }

    /// Returns every named condition result when policy evaluation ran.
    pub fn condition_results(&self) -> Option<&[PolicyConditionResult]> {
        match self {
            Self::Evaluated {
                condition_results, ..
            } => Some(condition_results),
            Self::NotEvaluated { .. } => None,
        }
    }

    /// Returns every route-independent event fact staged by this run.
    pub fn event_eligibilities(&self) -> &[PolicyEventEligibility] {
        match self {
            Self::Evaluated {
                event_eligibilities,
                ..
            }
            | Self::NotEvaluated {
                event_eligibilities,
            } => event_eligibilities,
        }
    }
}

/// One named condition's decision against the run's pre-run policy context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyConditionResult {
    condition_id: String,
    outcome: ConditionOutcome,
    triggered: bool,
    active_before: bool,
    active_after: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference: Option<PolicyReferenceEvidence>,
}

impl PolicyConditionResult {
    fn from_evaluation(evaluation: &ConditionEvaluation) -> Self {
        Self {
            condition_id: evaluation.condition_id().to_owned(),
            outcome: evaluation.outcome(),
            triggered: evaluation.trigger(),
            active_before: evaluation.active_before(),
            active_after: evaluation.active_after(),
            reference: evaluation
                .reference_evidence()
                .map(PolicyReferenceEvidence::from_evidence),
        }
    }

    /// Returns the stable target-local condition identifier.
    pub fn condition_id(&self) -> &str {
        &self.condition_id
    }

    /// Returns the exact predicate outcome.
    pub const fn outcome(&self) -> ConditionOutcome {
        self.outcome
    }

    /// Returns whether this result made a condition event eligible.
    pub const fn triggered(&self) -> bool {
        self.triggered
    }

    /// Returns the condition's hysteresis state before this evaluation.
    pub const fn active_before(&self) -> bool {
        self.active_before
    }

    /// Returns the condition's staged hysteresis state after this evaluation.
    pub const fn active_after(&self) -> bool {
        self.active_after
    }

    /// Returns the named pre-run reference evidence when this predicate uses one.
    pub fn reference(&self) -> Option<&PolicyReferenceEvidence> {
        self.reference.as_ref()
    }
}

/// The named pre-run reference selected for a condition result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum PolicyReferenceEvidence {
    /// The named reference was available and supplied this canonical value.
    Resolved {
        /// The configured reference selector.
        reference: ConditionReference,
        /// The canonical value used by the condition evaluation.
        canonical_value: String,
    },
    /// The named reference was absent or incompatible with the target contract.
    Unavailable {
        /// The configured reference selector.
        reference: ConditionReference,
    },
}

impl PolicyReferenceEvidence {
    fn from_evidence(evidence: &ConditionReferenceEvidence) -> Self {
        match evidence {
            ConditionReferenceEvidence::Resolved {
                reference,
                canonical_value,
            } => Self::Resolved {
                reference: *reference,
                canonical_value: canonical_value.clone(),
            },
            ConditionReferenceEvidence::Unavailable { reference } => Self::Unavailable {
                reference: *reference,
            },
        }
    }

    /// Returns the configured pre-run reference selector.
    pub const fn reference(&self) -> ConditionReference {
        match self {
            Self::Resolved { reference, .. } | Self::Unavailable { reference } => *reference,
        }
    }

    /// Returns the canonical value used by the comparison when the reference was available.
    pub fn canonical_value(&self) -> Option<&str> {
        match self {
            Self::Resolved {
                canonical_value, ..
            } => Some(canonical_value),
            Self::Unavailable { .. } => None,
        }
    }
}

/// One domain event eligible for matching delivery routes after policy staging.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyEventEligibility {
    event_kind: DeliveryEventKind,
    route_family: RouteFamily,
    #[serde(skip_serializing_if = "Option::is_none")]
    condition_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason_class: Option<SourceSuspectReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<PermanentErrorCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_fault_code: Option<IntegrationFaultCode>,
}

impl PolicyEventEligibility {
    fn from_staged(eligibility: &StagedEventEligibility) -> Self {
        let (condition_id, reason_class, error_code, integration_fault_code) = match eligibility {
            StagedEventEligibility::OnCondition { condition_id } => {
                (Some(condition_id.to_string()), None, None, None)
            }
            StagedEventEligibility::OnRun { cause } => match cause {
                OnRunEventCause::Reset | OnRunEventCause::Initialized => (None, None, None, None),
                OnRunEventCause::ConditionIssue { condition_id, .. } => {
                    (Some(condition_id.to_string()), None, None, None)
                }
                OnRunEventCause::SourceSuspectEscalated { reason_class } => {
                    (None, Some(*reason_class), None, None)
                }
                OnRunEventCause::PermanentContractErrorEpisodeBegan { error_code } => {
                    (None, None, Some(*error_code), None)
                }
                OnRunEventCause::IntegrationFaultEpisodeBegan {
                    integration_fault_code,
                } => (None, None, None, Some(*integration_fault_code)),
            },
        };
        Self {
            event_kind: eligibility.event_kind(),
            route_family: eligibility.route_family(),
            condition_id,
            reason_class,
            error_code,
            integration_fault_code,
        }
    }

    /// Returns the closed domain event kind.
    pub const fn event_kind(&self) -> DeliveryEventKind {
        self.event_kind
    }

    /// Returns the route family that would receive this event when configured.
    pub const fn route_family(&self) -> RouteFamily {
        self.route_family
    }

    /// Returns the relevant named condition, when the event is condition-scoped.
    pub fn condition_id(&self) -> Option<&str> {
        self.condition_id.as_deref()
    }

    /// Returns the source-health reason for a source escalation event.
    pub const fn reason_class(&self) -> Option<SourceSuspectReason> {
        self.reason_class
    }

    /// Returns the permanent-error code for a permanent-error event.
    pub const fn error_code(&self) -> Option<PermanentErrorCode> {
        self.error_code
    }

    /// Returns the integration-fault code for an integration event.
    pub const fn integration_fault_code(&self) -> Option<IntegrationFaultCode> {
        self.integration_fault_code
    }
}

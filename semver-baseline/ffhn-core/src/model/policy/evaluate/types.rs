//! Exact policy-evaluation inputs, outcomes, and reference evidence.

use serde::{Deserialize, Serialize};

use crate::Observation;

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

/// One exact outcome with its trigger decision and next hysteresis state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConditionEvaluation {
    pub(super) condition_id: ConditionId,
    pub(super) outcome: ConditionOutcome,
    pub(super) trigger: bool,
    pub(super) active_before: bool,
    pub(super) active_after: bool,
    pub(super) reference_evidence: Option<ConditionReferenceEvidence>,
}

/// Evidence for the named pre-run reference used by one condition evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum ConditionReferenceEvidence {
    /// The configured reference existed and supplied this canonical value.
    Resolved {
        /// The configured pre-run reference selector.
        reference: ConditionReference,
        /// The exact canonical value used by the policy calculation.
        canonical_value: String,
    },
    /// The configured reference did not exist or could not be used under the measurement contract.
    Unavailable {
        /// The configured pre-run reference selector.
        reference: ConditionReference,
    },
}

impl ConditionEvaluation {
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

    /// Returns the resulting hysteresis state after this evaluation.
    pub const fn active_after(&self) -> bool {
        self.active_after
    }

    /// Returns the configured-reference evidence when this predicate compares one pre-run value.
    pub fn reference_evidence(&self) -> Option<&ConditionReferenceEvidence> {
        self.reference_evidence.as_ref()
    }
}

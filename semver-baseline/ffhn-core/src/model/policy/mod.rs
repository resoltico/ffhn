mod condition;
mod evaluate;
mod exact_numeric;
mod value;

#[cfg(test)]
mod tests;

/// Version of the policy-decision semantics bound into every target contract digest.
///
/// Increment this value whenever the same accepted observations could produce a different
/// condition decision. This is deliberately independent from crate and dependency versions:
/// persisted temporal state must be reset before it can be evaluated under changed semantics.
pub(crate) const POLICY_EVALUATION_SEMANTICS_VERSION: u32 = 1;

pub(in crate::model) use condition::validate_conditions;
pub use condition::{
    Condition, ConditionId, ConditionPredicate, ConditionReference, ThresholdDirection,
};
pub(in crate::model) use evaluate::stage_policy_run;
pub use evaluate::{
    ConditionContext, ConditionEvaluation, ConditionIssue, ConditionOutcome,
    ConditionReferenceEvidence, OnRunEventCause, PolicyRunInput, StagedEventEligibility,
    StagedPolicyRun,
};

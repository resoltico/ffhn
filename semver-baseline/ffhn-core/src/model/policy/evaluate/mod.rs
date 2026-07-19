//! Deterministic policy staging, predicate evaluation, and durable-event eligibility.

mod predicates;
mod stage;
mod types;

pub(in crate::model) use stage::stage_policy_run;
pub use types::{
    ConditionContext, ConditionEvaluation, ConditionIssue, ConditionOutcome,
    ConditionReferenceEvidence, OnRunEventCause, PolicyRunInput, StagedEventEligibility,
    StagedPolicyRun,
};

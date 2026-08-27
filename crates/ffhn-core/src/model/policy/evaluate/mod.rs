//! Deterministic exact policy evaluation.

mod predicates;
mod types;

pub(crate) use predicates::{PolicyContract, evaluate_conditions};
pub use types::{
    ConditionContext, ConditionEvaluation, ConditionOutcome, ConditionReferenceEvidence,
};

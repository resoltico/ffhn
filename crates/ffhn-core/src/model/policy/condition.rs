use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::CoreError;

use super::super::{DeclaredType, TypeParams};
use super::value::{PolicyValue, parse_config_value, parse_percentage};

/// A stable identifier for one target-local named condition.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ConditionId(String);

impl ConditionId {
    /// Parses a target-local condition identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        validate_condition_id(&value)?;
        Ok(Self(value))
    }

    /// Returns the canonical identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ConditionId {
    type Error = CoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ConditionId> for String {
    fn from(value: ConditionId) -> Self {
        value.0
    }
}

impl FromStr for ConditionId {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl AsRef<str> for ConditionId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ConditionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A complete named policy condition configured by a target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    condition_id: ConditionId,
    predicate: ConditionPredicate,
}

impl Condition {
    /// Returns the typed target-local identifier for internal policy lookup.
    pub(crate) const fn id(&self) -> &ConditionId {
        &self.condition_id
    }

    /// Returns the stable condition identifier.
    pub fn condition_id(&self) -> &str {
        self.condition_id.as_str()
    }

    /// Returns the configured predicate.
    pub const fn predicate(&self) -> &ConditionPredicate {
        &self.predicate
    }
}

/// A named accepted-observation reference used by reference predicates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionReference {
    /// The accepted observation immediately preceding the current run.
    LastAcceptedObservation,
    /// The first accepted observation for the current target contract.
    FixedInitialBaseline,
    /// The last transition recorded for the condition currently being evaluated.
    LastConditionTransition,
}

impl ConditionReference {
    /// Returns the stable report-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LastAcceptedObservation => "last_accepted_observation",
            Self::FixedInitialBaseline => "fixed_initial_baseline",
            Self::LastConditionTransition => "last_condition_transition",
        }
    }
}

/// Direction for threshold crossings and hysteresis bands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdDirection {
    /// A value moves upward through a threshold or enters above a band.
    Rising,
    /// A value moves downward through a threshold or enters below a band.
    Falling,
}

/// One typed condition predicate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConditionPredicate {
    /// Tests canonical-value identity against a named accepted-observation reference.
    Changed {
        /// The pre-run accepted observation selected for comparison.
        reference: ConditionReference,
    },
    /// Tests whether the exact absolute numeric delta reaches a non-negative threshold.
    DeltaAbs {
        /// The pre-run accepted observation selected for comparison.
        reference: ConditionReference,
        /// Non-negative literal in the target's declared type grammar.
        threshold: String,
    },
    /// Tests whether the exact percentage delta reaches a non-negative percentage threshold.
    DeltaPct {
        /// The pre-run accepted observation selected for comparison.
        reference: ConditionReference,
        /// Non-negative invariant decimal percentage literal.
        threshold: String,
    },
    /// Tests whether the last accepted value crossed one typed threshold in a direction.
    Crosses {
        /// Literal in the target's declared type grammar.
        threshold: String,
        /// The required direction of the crossing.
        direction: ThresholdDirection,
    },
    /// Tests whether the current typed value is strictly below one threshold.
    Lt {
        /// Literal in the target's declared type grammar.
        threshold: String,
    },
    /// Tests whether the current typed value is strictly above one threshold.
    Gt {
        /// Literal in the target's declared type grammar.
        threshold: String,
    },
    /// Tests a directional hysteresis band using enter and exit thresholds.
    Band {
        /// Literal that admits a currently inactive condition.
        enter_threshold: String,
        /// Literal that keeps a currently active condition satisfied.
        exit_threshold: String,
        /// The direction in which the band becomes active.
        direction: ThresholdDirection,
    },
}

impl ConditionPredicate {
    pub(crate) const fn is_event_predicate(&self) -> bool {
        matches!(
            self,
            Self::Changed { .. }
                | Self::DeltaAbs { .. }
                | Self::DeltaPct { .. }
                | Self::Crosses { .. }
        )
    }
}

pub(in crate::model) fn validate_conditions(
    declared_type: DeclaredType,
    params: &TypeParams,
    conditions: &[Condition],
) -> Result<(), CoreError> {
    let mut condition_ids = BTreeSet::new();
    for condition in conditions {
        if !condition_ids.insert(&condition.condition_id) {
            return Err(CoreError::contract("condition_id values must be unique"));
        }
        validate_predicate(declared_type, params, condition)?;
    }
    Ok(())
}

fn validate_predicate(
    declared_type: DeclaredType,
    params: &TypeParams,
    condition: &Condition,
) -> Result<(), CoreError> {
    match &condition.predicate {
        ConditionPredicate::Changed { .. } => Ok(()),
        ConditionPredicate::DeltaAbs { threshold, .. } => {
            require_numeric(declared_type, condition)?;
            let threshold =
                parse_threshold(declared_type, params, threshold, condition, "threshold")?;
            if threshold.is_negative_numeric() {
                return Err(CoreError::contract(format!(
                    "condition {} delta_abs.threshold must be non-negative",
                    condition.condition_id
                )));
            }
            Ok(())
        }
        ConditionPredicate::DeltaPct { threshold, .. } => {
            require_numeric(declared_type, condition)?;
            let percentage = parse_percentage(threshold).map_err(|message| {
                CoreError::contract(format!(
                    "condition {} delta_pct.threshold {message}",
                    condition.condition_id
                ))
            })?;
            if percentage < Decimal::ZERO {
                return Err(CoreError::contract(format!(
                    "condition {} delta_pct.threshold must be non-negative",
                    condition.condition_id
                )));
            }
            Ok(())
        }
        ConditionPredicate::Crosses { threshold, .. }
        | ConditionPredicate::Lt { threshold }
        | ConditionPredicate::Gt { threshold } => {
            require_ordered(declared_type, condition)?;
            parse_threshold(declared_type, params, threshold, condition, "threshold")?;
            Ok(())
        }
        ConditionPredicate::Band {
            enter_threshold,
            exit_threshold,
            direction,
        } => {
            require_ordered(declared_type, condition)?;
            let enter = parse_threshold(
                declared_type,
                params,
                enter_threshold,
                condition,
                "enter_threshold",
            )?;
            let exit = parse_threshold(
                declared_type,
                params,
                exit_threshold,
                condition,
                "exit_threshold",
            )?;
            let order = enter
                .compare(&exit)
                .expect("band thresholds parsed under one declared type must be comparable");
            let valid = match direction {
                ThresholdDirection::Rising => order != Ordering::Less,
                ThresholdDirection::Falling => order != Ordering::Greater,
            };
            if !valid {
                return Err(CoreError::contract(format!(
                    "condition {} band thresholds conflict with direction",
                    condition.condition_id
                )));
            }
            Ok(())
        }
    }
}

fn require_ordered(declared_type: DeclaredType, condition: &Condition) -> Result<(), CoreError> {
    if declared_type == DeclaredType::Text {
        return Err(CoreError::contract(format!(
            "condition {} uses an ordered predicate with text declared_type",
            condition.condition_id
        )));
    }
    Ok(())
}

fn require_numeric(declared_type: DeclaredType, condition: &Condition) -> Result<(), CoreError> {
    if matches!(
        declared_type,
        DeclaredType::Integer | DeclaredType::Decimal | DeclaredType::Money
    ) {
        Ok(())
    } else {
        Err(CoreError::contract(format!(
            "condition {} uses a numeric predicate with a non-numeric declared_type",
            condition.condition_id
        )))
    }
}

fn parse_threshold(
    declared_type: DeclaredType,
    params: &TypeParams,
    value: &str,
    condition: &Condition,
    field: &str,
) -> Result<PolicyValue, CoreError> {
    parse_config_value(declared_type, params, value).map_err(|message| {
        CoreError::contract(format!(
            "condition {} {field} {message}",
            condition.condition_id
        ))
    })
}

fn validate_condition_id(value: &str) -> Result<(), CoreError> {
    if value.is_empty()
        || value.len() > 64
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
        || !value.as_bytes()[0].is_ascii_lowercase() && !value.as_bytes()[0].is_ascii_digit()
        || value.ends_with(['-', '_'])
        || value.contains("--")
        || value.contains("__")
        || value.contains("-_")
        || value.contains("_-")
    {
        return Err(CoreError::contract(
            "condition_id must start with [a-z0-9], stay within 64 chars, and only use single internal '-' or '_' separators",
        ));
    }
    Ok(())
}

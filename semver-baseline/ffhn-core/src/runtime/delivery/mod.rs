//! Durable outbox work decomposed into event materialization, due-record draining, and process execution.

mod drain;
mod materialize;
#[cfg(test)]
mod tests;

use crate::DeliveryOutcome;

pub(crate) mod process;
pub(super) use drain::*;
pub(super) use materialize::*;

/// Results accumulated while draining all currently due outbox records.
#[derive(Debug)]
pub(super) struct DeliveryDrain {
    outcomes: Vec<DeliveryOutcome>,
}

impl DeliveryDrain {
    pub(super) fn outcomes(&self) -> Vec<DeliveryOutcome> {
        self.outcomes.clone()
    }
}

/// A post-commit outbox-state persistence failure that still retains attempt evidence.
pub(super) struct DeliveryDrainFailure {
    pub(super) drain: DeliveryDrain,
    pub(super) error: crate::CoreError,
}

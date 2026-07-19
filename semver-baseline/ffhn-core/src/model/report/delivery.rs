//! Post-commit delivery outcomes with executable causal relations.

use serde::{Deserialize, Deserializer, Serialize};

use crate::{CoreError, DeliveryEventKind};

use super::diagnostic::DiagnosticDetail;

/// The result of one post-commit durable outbox delivery attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    /// The process accepted the stored payload and the pending record was removed.
    Delivered,
    /// The process failed and the record remains pending for its deterministic next retry.
    RetryScheduled,
    /// The process exhausted `max_attempts`; the record was removed as terminal evidence.
    DeadLettered,
    /// The process accepted the payload but FFHN could not persist the record removal.
    DeliveredUncommitted,
    /// The process failed but FFHN could not persist its deterministic retry state.
    RetryUncommitted,
    /// FFHN could not persist the required terminal-record removal.
    DeadLetterUncommitted,
}

impl DeliveryStatus {
    /// Returns the stable report-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::RetryScheduled => "retry_scheduled",
            Self::DeadLettered => "dead_lettered",
            Self::DeliveredUncommitted => "delivered_uncommitted",
            Self::RetryUncommitted => "retry_uncommitted",
            Self::DeadLetterUncommitted => "dead_letter_uncommitted",
        }
    }

    const fn expects_error_detail(self) -> bool {
        matches!(
            self,
            Self::RetryScheduled
                | Self::DeadLettered
                | Self::RetryUncommitted
                | Self::DeadLetterUncommitted
        )
    }

    const fn expects_outbox_error_detail(self) -> bool {
        matches!(
            self,
            Self::DeliveredUncommitted | Self::RetryUncommitted | Self::DeadLetterUncommitted
        )
    }

    const fn permits_observability_detail(self) -> bool {
        matches!(self, Self::Delivered | Self::DeliveredUncommitted)
    }
}

/// Immutable evidence of one post-commit outbox delivery attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeliveryOutcome {
    event_id: String,
    route_id: String,
    event_kind: DeliveryEventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    condition_id: Option<String>,
    status: DeliveryStatus,
    attempt_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_detail: Option<DiagnosticDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outbox_error_detail: Option<DiagnosticDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_observability_detail: Option<DiagnosticDetail>,
}

/// Deserialization wire form that makes the public delivery matrix fail closed.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliveryOutcomeWire {
    event_id: String,
    route_id: String,
    event_kind: DeliveryEventKind,
    #[serde(default)]
    condition_id: Option<String>,
    status: DeliveryStatus,
    attempt_count: u32,
    #[serde(default)]
    error_detail: Option<DiagnosticDetail>,
    #[serde(default)]
    outbox_error_detail: Option<DiagnosticDetail>,
    #[serde(default)]
    delivery_observability_detail: Option<DiagnosticDetail>,
}

impl<'de> Deserialize<'de> for DeliveryOutcome {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DeliveryOutcomeWire::deserialize(deserializer)?;
        Self::new(
            wire.event_id,
            wire.route_id,
            wire.event_kind,
            wire.condition_id,
            wire.status,
            wire.attempt_count,
            wire.error_detail,
            wire.outbox_error_detail,
            wire.delivery_observability_detail,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl DeliveryOutcome {
    pub(crate) fn delivered(
        event_id: String,
        route_id: String,
        event_kind: DeliveryEventKind,
        condition_id: Option<String>,
        attempt_count: u32,
        observability: Option<DiagnosticDetail>,
    ) -> Self {
        Self::from_trusted_outbox_facts(
            event_id,
            route_id,
            event_kind,
            condition_id,
            DeliveryStatus::Delivered,
            attempt_count,
            None,
            None,
            observability,
        )
    }

    pub(crate) fn retry_scheduled(
        event_id: String,
        route_id: String,
        event_kind: DeliveryEventKind,
        condition_id: Option<String>,
        attempt_count: u32,
        error_detail: DiagnosticDetail,
    ) -> Self {
        Self::from_trusted_outbox_facts(
            event_id,
            route_id,
            event_kind,
            condition_id,
            DeliveryStatus::RetryScheduled,
            attempt_count,
            Some(error_detail),
            None,
            None,
        )
    }

    pub(crate) fn dead_lettered(
        event_id: String,
        route_id: String,
        event_kind: DeliveryEventKind,
        condition_id: Option<String>,
        attempt_count: u32,
        error_detail: DiagnosticDetail,
    ) -> Self {
        Self::from_trusted_outbox_facts(
            event_id,
            route_id,
            event_kind,
            condition_id,
            DeliveryStatus::DeadLettered,
            attempt_count,
            Some(error_detail),
            None,
            None,
        )
    }

    pub(crate) fn delivered_uncommitted(
        event_id: String,
        route_id: String,
        event_kind: DeliveryEventKind,
        condition_id: Option<String>,
        attempt_count: u32,
        outbox_error_detail: DiagnosticDetail,
        observability: Option<DiagnosticDetail>,
    ) -> Self {
        Self::from_trusted_outbox_facts(
            event_id,
            route_id,
            event_kind,
            condition_id,
            DeliveryStatus::DeliveredUncommitted,
            attempt_count,
            None,
            Some(outbox_error_detail),
            observability,
        )
    }

    pub(crate) fn retry_uncommitted(
        event_id: String,
        route_id: String,
        event_kind: DeliveryEventKind,
        condition_id: Option<String>,
        attempt_count: u32,
        error_detail: DiagnosticDetail,
        outbox_error_detail: DiagnosticDetail,
    ) -> Self {
        Self::from_trusted_outbox_facts(
            event_id,
            route_id,
            event_kind,
            condition_id,
            DeliveryStatus::RetryUncommitted,
            attempt_count,
            Some(error_detail),
            Some(outbox_error_detail),
            None,
        )
    }

    pub(crate) fn dead_letter_uncommitted(
        event_id: String,
        route_id: String,
        event_kind: DeliveryEventKind,
        condition_id: Option<String>,
        attempt_count: u32,
        error_detail: DiagnosticDetail,
        outbox_error_detail: DiagnosticDetail,
    ) -> Self {
        Self::from_trusted_outbox_facts(
            event_id,
            route_id,
            event_kind,
            condition_id,
            DeliveryStatus::DeadLetterUncommitted,
            attempt_count,
            Some(error_detail),
            Some(outbox_error_detail),
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        event_id: String,
        route_id: String,
        event_kind: DeliveryEventKind,
        condition_id: Option<String>,
        status: DeliveryStatus,
        attempt_count: u32,
        error_detail: Option<DiagnosticDetail>,
        outbox_error_detail: Option<DiagnosticDetail>,
        delivery_observability_detail: Option<DiagnosticDetail>,
    ) -> Result<Self, CoreError> {
        let outcome = Self::from_trusted_outbox_facts(
            event_id,
            route_id,
            event_kind,
            condition_id,
            status,
            attempt_count,
            error_detail,
            outbox_error_detail,
            delivery_observability_detail,
        );
        outcome.validate()?;
        Ok(outcome)
    }

    /// Builds an outcome from facts already owned by validated pending state and fixed-status
    /// constructors. The deserialization boundary remains [`Self::new`], which revalidates raw
    /// report data; drain-time facts cannot become malformed without violating their source
    /// aggregate's invariant first.
    #[allow(clippy::too_many_arguments)]
    fn from_trusted_outbox_facts(
        event_id: String,
        route_id: String,
        event_kind: DeliveryEventKind,
        condition_id: Option<String>,
        status: DeliveryStatus,
        attempt_count: u32,
        error_detail: Option<DiagnosticDetail>,
        outbox_error_detail: Option<DiagnosticDetail>,
        delivery_observability_detail: Option<DiagnosticDetail>,
    ) -> Self {
        Self {
            event_id,
            route_id,
            event_kind,
            condition_id,
            status,
            attempt_count,
            error_detail,
            outbox_error_detail,
            delivery_observability_detail,
        }
    }

    /// Validates every legal and illegal member of the delivery status/detail relation.
    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        if self.attempt_count == 0 {
            return Err(CoreError::contract(
                "delivery outcome attempt_count must be positive",
            ));
        }
        if let Some(detail) = &self.error_detail {
            detail.validate()?;
            if !detail.is_delivery_failure() {
                return Err(CoreError::contract(
                    "delivery error_detail must carry a delivery_process failure",
                ));
            }
        }
        if let Some(detail) = &self.outbox_error_detail {
            detail.validate()?;
            let operation_is_valid = match self.status {
                DeliveryStatus::DeliveredUncommitted | DeliveryStatus::DeadLetterUncommitted => {
                    detail.operation() == crate::DiagnosticOperation::OutboxStateCommit
                }
                DeliveryStatus::RetryUncommitted => {
                    let operation = detail.operation();
                    operation == crate::DiagnosticOperation::OutboxDrain
                        || operation == crate::DiagnosticOperation::OutboxStateCommit
                }
                DeliveryStatus::Delivered
                | DeliveryStatus::RetryScheduled
                | DeliveryStatus::DeadLettered => false,
            };
            if detail.kind() == crate::DiagnosticKind::Delivery || !operation_is_valid {
                return Err(CoreError::contract(
                    "outbox_error_detail must describe the status-compatible outbox operation",
                ));
            }
        }
        if let Some(detail) = &self.delivery_observability_detail {
            detail.validate()?;
            if !self.status.permits_observability_detail() || !detail.is_delivery_observability() {
                return Err(CoreError::contract(
                    "delivery observability detail is legal only for delivered statuses",
                ));
            }
        }
        if self.status.expects_error_detail() != self.error_detail.is_some()
            || self.status.expects_outbox_error_detail() != self.outbox_error_detail.is_some()
        {
            return Err(CoreError::contract(
                "delivery status and diagnostic-detail presence disagree",
            ));
        }
        Ok(())
    }

    /// Returns the deterministic event identity.
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    /// Returns the target-local delivery route identifier.
    pub fn route_id(&self) -> &str {
        &self.route_id
    }

    /// Returns the domain event kind attempted for this route.
    pub const fn event_kind(&self) -> DeliveryEventKind {
        self.event_kind
    }

    /// Returns the named condition for a condition-scoped event, when any.
    pub fn condition_id(&self) -> Option<&str> {
        self.condition_id.as_deref()
    }

    /// Returns the delivery result category.
    pub const fn status(&self) -> DeliveryStatus {
        self.status
    }

    /// Returns the completed attempt count for this record.
    pub const fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    /// Returns delivery-process failure detail for failed delivery statuses.
    pub fn error_detail(&self) -> Option<&DiagnosticDetail> {
        self.error_detail.as_ref()
    }

    /// Returns outbox persistence failure detail for uncommitted statuses.
    pub fn outbox_error_detail(&self) -> Option<&DiagnosticDetail> {
        self.outbox_error_detail.as_ref()
    }

    /// Returns stderr-capture observability evidence after a successful delivery.
    pub fn delivery_observability_detail(&self) -> Option<&DiagnosticDetail> {
        self.delivery_observability_detail.as_ref()
    }
}

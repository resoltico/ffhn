//! Source-local in-memory capability pacing state and closed withdrawal reasons.

use std::collections::BTreeMap;

use time::{Duration, OffsetDateTime};

use super::super::MeasurementId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeferralReason {
    LockContention,
    Unreadable,
    SourceDisabled,
    UnresolvableManifest,
    ConfigInvalid,
    LineageRefused,
    AcquisitionHold,
    DeliveryUnreachable,
}

impl DeferralReason {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::LockContention => "lock_contention",
            Self::Unreadable => "unreadable",
            Self::SourceDisabled => "source_disabled",
            Self::UnresolvableManifest => "unresolvable_manifest",
            Self::ConfigInvalid => "config_invalid",
            Self::LineageRefused => "lineage_refused",
            Self::AcquisitionHold => "acquisition_hold",
            Self::DeliveryUnreachable => "delivery_unreachable",
        }
    }
}

/// All mutable pacing facts owned by exactly one source.
#[derive(Clone, Debug, Default)]
pub(super) struct SourceDeferrals {
    pub(super) acquisition_until: Option<OffsetDateTime>,
    pub(super) acquisition_reason: Option<DeferralReason>,
    pub(super) drain_until: Option<OffsetDateTime>,
    pub(super) drain_reason: Option<DeferralReason>,
    pub(super) measurement_drain_until: BTreeMap<MeasurementId, OffsetDateTime>,
    pub(super) measurement_drain_reason: BTreeMap<MeasurementId, DeferralReason>,
}

impl SourceDeferrals {
    pub(super) fn expire_elapsed(&mut self, now: OffsetDateTime) {
        if self.acquisition_until.is_some_and(|until| until <= now) {
            self.acquisition_until = None;
            self.acquisition_reason = None;
        }
        if self.drain_until.is_some_and(|until| until <= now) {
            self.drain_until = None;
            self.drain_reason = None;
        }
        self.measurement_drain_until.retain(|_, until| *until > now);
        self.measurement_drain_reason
            .retain(|measurement_id, _| self.measurement_drain_until.contains_key(measurement_id));
    }

    pub(super) fn acquisition_permitted(&self, now: OffsetDateTime) -> bool {
        self.acquisition_until.is_none_or(|until| until <= now)
    }

    pub(super) fn drain_permitted(&self, now: OffsetDateTime) -> bool {
        self.drain_until.is_none_or(|until| until <= now)
    }

    pub(super) fn measurement_drain_permitted(
        &self,
        id: &MeasurementId,
        now: OffsetDateTime,
    ) -> bool {
        self.measurement_drain_until
            .get(id)
            .is_none_or(|until| *until <= now)
    }

    pub(super) fn defer_acquisition(
        &mut self,
        now: OffsetDateTime,
        milliseconds: i64,
        reason: DeferralReason,
    ) {
        self.acquisition_until = Some(now + Duration::milliseconds(milliseconds));
        self.acquisition_reason = Some(reason);
    }

    pub(super) fn defer_drain(
        &mut self,
        now: OffsetDateTime,
        milliseconds: i64,
        reason: DeferralReason,
    ) {
        self.drain_until = Some(now + Duration::milliseconds(milliseconds));
        self.drain_reason = Some(reason);
    }
}

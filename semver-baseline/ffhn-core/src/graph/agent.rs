//! Graph-lease-owning fixed-interval agent scheduling with independently paced capabilities.

use std::collections::BTreeMap;

use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::CoreError;

use super::locks::GraphLease;
use super::{
    DrainResult, GraphMeasureResult, GraphSourceStatus, SourceId, TrustedGraphRoot,
    drain_measurement_outbox_once, drain_source_outbox_once, measure_source_once,
};

#[path = "agent/bounded.rs"]
mod bounded;
#[path = "agent/deferral.rs"]
mod deferral;
#[path = "agent/drain_state.rs"]
mod drain_state;
#[path = "agent/wake.rs"]
mod wake;

use bounded::run_bounded;
use deferral::{DeferralReason, SourceDeferrals};
#[cfg(test)]
use drain_state::record_measurement_drain_result;
use drain_state::{drain_measurements, finish_source_drain};

const CONTENDED_DEFER_MS: i64 = 100;
const UNREADABLE_DEFER_MS: i64 = 1_000;

/// A singleton agent worker that retains the graph lease for its complete running lifetime.
pub struct AgentWorker {
    _lease: GraphLease,
    deferrals: BTreeMap<SourceId, SourceDeferrals>,
}

/// Aggregate result of one finite agent tick.
#[derive(Clone, Debug)]
pub struct AgentTickResult {
    sources: Vec<AgentSourceTurn>,
}

/// Source-local acquisition and drain facts from one agent tick.
#[derive(Clone, Debug)]
pub struct AgentSourceTurn {
    source_id: SourceId,
    measurement: Option<GraphMeasureResult>,
    acquisition_error: Option<String>,
    source_drain: Option<DrainResult>,
    drain_error: Option<String>,
    measurement_drains: Vec<(String, DrainResult)>,
    measurement_drain_deferrals: Vec<(String, String, String)>,
    acquisition_deferred_until: Option<String>,
    acquisition_deferred_reason: Option<String>,
    drain_deferred_until: Option<String>,
    drain_deferred_reason: Option<String>,
}

impl AgentWorker {
    /// Claims the graph-wide lease for a continuous worker; `None` means another agent owns it.
    pub fn try_start(graph: &TrustedGraphRoot) -> Result<Option<Self>, CoreError> {
        graph.validate_graph_documents()?;
        Ok(graph.try_acquire_agent_lease()?.map(|lease| Self {
            _lease: lease,
            deferrals: BTreeMap::new(),
        }))
    }

    /// Runs one finite source-order-stable tick while retaining this worker's graph lease.
    pub fn tick(
        &mut self,
        graph: &TrustedGraphRoot,
        now_utc: String,
    ) -> Result<AgentTickResult, CoreError> {
        self.tick_with_jobs(graph, now_utc, 1)
    }

    /// Runs one finite tick with at most `jobs` source turns executing concurrently.
    pub fn tick_with_jobs(
        &mut self,
        graph: &TrustedGraphRoot,
        now_utc: String,
        jobs: usize,
    ) -> Result<AgentTickResult, CoreError> {
        if jobs == 0 {
            return Err(CoreError::contract("agent jobs must be positive"));
        }
        let now = OffsetDateTime::parse(&now_utc, &Rfc3339)?;
        let mut ids = graph.source_ids()?;
        ids.sort_unstable();
        let source_count = ids.len();
        let graph_paths = graph.paths().clone();
        let pending = ids
            .into_iter()
            .map(|source_id| {
                let deferrals = self.deferrals.remove(&source_id).unwrap_or_default();
                (source_id, deferrals)
            })
            .collect::<Vec<_>>();
        let completed = run_bounded(pending, jobs, |(source_id, mut deferrals)| {
            let turn = match TrustedGraphRoot::open(graph_paths.clone()) {
                Ok(graph) => {
                    Self::tick_source(&graph, source_id.clone(), now, &now_utc, &mut deferrals)
                }
                Err(error) => {
                    deferrals.defer_acquisition(
                        now,
                        UNREADABLE_DEFER_MS,
                        DeferralReason::Unreadable,
                    );
                    deferrals.defer_drain(now, UNREADABLE_DEFER_MS, DeferralReason::Unreadable);
                    Self::turn(
                        &deferrals,
                        source_id.clone(),
                        None,
                        Some(error.to_string()),
                        None,
                        Some("graph root is unavailable to this source worker".to_owned()),
                    )
                }
            };
            Ok((source_id, deferrals, turn))
        })?;
        let mut turns = Vec::with_capacity(source_count);
        for (source_id, deferrals, turn) in completed {
            self.deferrals.insert(source_id, deferrals);
            turns.push(turn);
        }
        turns.sort_unstable_by(|left, right| left.source_id.cmp(&right.source_id));
        Ok(AgentTickResult { sources: turns })
    }

    /// Calculates the earliest UTC instant at which any source acquisition or reachable outbox
    /// retry can run without violating its source or measurement deferral clock.
    pub fn next_wake_at(
        &self,
        graph: &TrustedGraphRoot,
        now_utc: String,
    ) -> Result<String, CoreError> {
        let now = OffsetDateTime::parse(&now_utc, &Rfc3339)?;
        wake::next_wake_at(self, graph, now)?
            .format(&Rfc3339)
            .map_err(CoreError::from)
    }

    fn tick_source(
        graph: &TrustedGraphRoot,
        source_id: SourceId,
        now: OffsetDateTime,
        now_utc: &str,
        deferrals: &mut SourceDeferrals,
    ) -> AgentSourceTurn {
        deferrals.expire_elapsed(now);
        let source = match graph.open_source(source_id.clone()) {
            Ok(source) => source,
            Err(error) => {
                deferrals.defer_acquisition(now, UNREADABLE_DEFER_MS, DeferralReason::Unreadable);
                deferrals.defer_drain(now, UNREADABLE_DEFER_MS, DeferralReason::Unreadable);
                return Self::turn(
                    deferrals,
                    source_id,
                    None,
                    Some(error.to_string()),
                    None,
                    Some("source directory is unavailable".to_owned()),
                );
            }
        };
        let due = source
            .inspect_lineage([])
            .ok()
            .and_then(|inspection| inspection.source().as_ready_state().cloned())
            .is_none_or(|state| {
                state.next_due_utc().is_none_or(|due| {
                    OffsetDateTime::parse(due, &Rfc3339)
                        .is_ok_and(|due| acquisition_is_due(due, now))
                })
            });
        let acquisition_allowed = due && deferrals.acquisition_permitted(now);
        let (measurement, acquisition_error) = if acquisition_allowed {
            match measure_source_once(graph, source_id.clone()) {
                Ok(result) if result.status() == GraphSourceStatus::Locked => {
                    deferrals.defer_acquisition(
                        now,
                        CONTENDED_DEFER_MS,
                        DeferralReason::LockContention,
                    );
                    deferrals.defer_drain(now, CONTENDED_DEFER_MS, DeferralReason::LockContention);
                    return Self::locked_turn(deferrals, source_id);
                }
                Ok(result) => {
                    if let Some(reason) = acquisition_deferral_reason(result.status()) {
                        deferrals.defer_acquisition(
                            now,
                            source_interval_milliseconds(&source),
                            reason,
                        );
                    } else {
                        deferrals.acquisition_until = None;
                        deferrals.acquisition_reason = None;
                    }
                    (Some(result), None)
                }
                Err(error) => {
                    deferrals.defer_acquisition(
                        now,
                        UNREADABLE_DEFER_MS,
                        DeferralReason::Unreadable,
                    );
                    (None, Some(error.to_string()))
                }
            }
        } else {
            (None, None)
        };
        let drain_now = source
            .inspect_lineage([])
            .ok()
            .and_then(|inspection| inspection.source().as_ready_state().cloned())
            .and_then(|state| state.last_cycle_completed_utc().map(ToOwned::to_owned))
            .and_then(|completed| OffsetDateTime::parse(&completed, &Rfc3339).ok())
            .unwrap_or(now)
            .max(now);
        let drain_now_utc =
            Self::format_time(Some(drain_now)).unwrap_or_else(|| now_utc.to_owned());
        let (source_drain, drain_error, measurement_drains) = Self::drain_source(
            graph,
            &source,
            &source_id,
            drain_now,
            &drain_now_utc,
            deferrals,
        );
        Self::turn_with_measurements(
            deferrals,
            source_id,
            measurement,
            acquisition_error,
            source_drain,
            drain_error,
            measurement_drains,
        )
    }

    fn drain_source(
        graph: &TrustedGraphRoot,
        source: &super::TrustedSourceDir,
        source_id: &SourceId,
        now: OffsetDateTime,
        now_utc: &str,
        deferrals: &mut SourceDeferrals,
    ) -> (
        Option<DrainResult>,
        Option<String>,
        Vec<(String, DrainResult)>,
    ) {
        if !deferrals.drain_permitted(now) {
            return (None, None, Vec::new());
        }
        let source_drain =
            match drain_source_outbox_once(graph, source_id.clone(), now_utc.to_owned()) {
                Ok(DrainResult::Locked) => {
                    deferrals.defer_drain(now, CONTENDED_DEFER_MS, DeferralReason::LockContention);
                    return (
                        Some(DrainResult::Locked),
                        None,
                        Self::locked_measurements(source),
                    );
                }
                Ok(DrainResult::Unreachable) => {
                    deferrals.defer_drain(
                        now,
                        source_interval_milliseconds(source),
                        DeferralReason::DeliveryUnreachable,
                    );
                    DrainResult::Unreachable
                }
                Ok(result) => result,
                Err(error) => {
                    deferrals.defer_drain(now, UNREADABLE_DEFER_MS, DeferralReason::Unreadable);
                    return (None, Some(error.to_string()), Vec::new());
                }
            };
        let measurement_ids = source
            .inspect_lineage([])
            .map(|inspection| inspection.measurements().keys().cloned().collect())
            .unwrap_or_else(|_| source.measurement_ids().unwrap_or_default());
        let (measurement_drains, measurement_error) = drain_measurements(
            measurement_ids,
            deferrals,
            now,
            source_interval_milliseconds(source),
            |measurement_id| {
                drain_measurement_outbox_once(
                    graph,
                    source_id.clone(),
                    measurement_id.clone(),
                    now_utc.to_owned(),
                )
            },
        );
        finish_source_drain(
            deferrals,
            source_drain,
            measurement_drains,
            measurement_error,
        )
    }

    fn locked_measurements(source: &super::TrustedSourceDir) -> Vec<(String, DrainResult)> {
        source
            .inspect_lineage([])
            .map(|inspection| {
                inspection
                    .measurements()
                    .keys()
                    .map(|id| (id.as_str().to_owned(), DrainResult::Locked))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn locked_turn(deferrals: &SourceDeferrals, source_id: SourceId) -> AgentSourceTurn {
        Self::turn(
            deferrals,
            source_id,
            None,
            None,
            Some(DrainResult::Locked),
            None,
        )
    }

    fn turn(
        deferrals: &SourceDeferrals,
        source_id: SourceId,
        measurement: Option<GraphMeasureResult>,
        acquisition_error: Option<String>,
        source_drain: Option<DrainResult>,
        drain_error: Option<String>,
    ) -> AgentSourceTurn {
        Self::turn_with_measurements(
            deferrals,
            source_id,
            measurement,
            acquisition_error,
            source_drain,
            drain_error,
            Vec::new(),
        )
    }

    fn turn_with_measurements(
        deferrals: &SourceDeferrals,
        source_id: SourceId,
        measurement: Option<GraphMeasureResult>,
        acquisition_error: Option<String>,
        source_drain: Option<DrainResult>,
        drain_error: Option<String>,
        measurement_drains: Vec<(String, DrainResult)>,
    ) -> AgentSourceTurn {
        AgentSourceTurn {
            acquisition_deferred_until: Self::format_time(deferrals.acquisition_until),
            acquisition_deferred_reason: deferrals
                .acquisition_reason
                .map(|reason| reason.as_str().to_owned()),
            drain_deferred_until: Self::format_time(deferrals.drain_until),
            drain_deferred_reason: deferrals
                .drain_reason
                .map(|reason| reason.as_str().to_owned()),
            source_id,
            measurement,
            acquisition_error,
            source_drain,
            drain_error,
            measurement_drains,
            measurement_drain_deferrals: deferrals
                .measurement_drain_until
                .iter()
                .filter_map(|(id, until)| {
                    let reason = deferrals.measurement_drain_reason.get(id)?;
                    until
                        .format(&Rfc3339)
                        .ok()
                        .map(|until| (id.as_str().to_owned(), until, reason.as_str().to_owned()))
                })
                .collect(),
        }
    }

    fn format_time(until: Option<OffsetDateTime>) -> Option<String> {
        until.and_then(|until| until.format(&Rfc3339).ok())
    }
}

fn acquisition_is_due(due: OffsetDateTime, now: OffsetDateTime) -> bool {
    due <= now
}

fn source_interval_milliseconds(source: &super::TrustedSourceDir) -> i64 {
    source
        .read_source_document()
        .ok()
        .and_then(|document| i64::try_from(document.schedule().interval_ms()).ok())
        .unwrap_or(UNREADABLE_DEFER_MS)
}

fn acquisition_deferral_reason(status: GraphSourceStatus) -> Option<DeferralReason> {
    match status {
        GraphSourceStatus::Disabled => Some(DeferralReason::SourceDisabled),
        GraphSourceStatus::UnresolvableManifest => Some(DeferralReason::UnresolvableManifest),
        GraphSourceStatus::ConfigInvalid => Some(DeferralReason::ConfigInvalid),
        GraphSourceStatus::LineageRefused => Some(DeferralReason::LineageRefused),
        GraphSourceStatus::AcquisitionHold => Some(DeferralReason::AcquisitionHold),
        GraphSourceStatus::Locked
        | GraphSourceStatus::Document
        | GraphSourceStatus::NotModified
        | GraphSourceStatus::FetchFailed
        | GraphSourceStatus::IntegrationFault => None,
    }
}

impl AgentTickResult {
    /// Returns source turns in stable source-ID order.
    pub fn sources(&self) -> &[AgentSourceTurn] {
        &self.sources
    }
    /// Returns whether any source turn completed with a handled acquisition or delivery failure.
    pub fn has_handled_failure(&self) -> bool {
        self.sources
            .iter()
            .any(AgentSourceTurn::has_handled_failure)
    }
}

impl AgentSourceTurn {
    fn has_handled_failure(&self) -> bool {
        self.acquisition_error.is_some()
            || self.drain_error.is_some()
            || self
                .measurement
                .as_ref()
                .is_some_and(GraphMeasureResult::has_handled_failure)
            || self.source_drain.as_ref().is_some_and(drain_is_failure)
            || self
                .measurement_drains
                .iter()
                .any(|(_, result)| drain_is_failure(result))
    }
    /// Returns the source identity covered by this turn.
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }
    /// Returns an acquisition result when the source was due and acquisition was permitted.
    pub fn measurement(&self) -> Option<&GraphMeasureResult> {
        self.measurement.as_ref()
    }
    /// Returns a safe source-local acquisition failure while independent drains continue.
    pub fn acquisition_error(&self) -> Option<&str> {
        self.acquisition_error.as_deref()
    }
    /// Returns the one source-outbox drain disposition when attempted.
    pub fn source_drain(&self) -> Option<&DrainResult> {
        self.source_drain.as_ref()
    }
    /// Returns a safe source-local drain failure while later sources continue.
    pub fn drain_error(&self) -> Option<&str> {
        self.drain_error.as_deref()
    }
    /// Returns measurement drain dispositions keyed by measurement id.
    pub fn measurement_drains(&self) -> &[(String, DrainResult)] {
        &self.measurement_drains
    }
    /// Returns active measurement-local drain deferrals in measurement-ID order.
    pub fn measurement_drain_deferrals(&self) -> &[(String, String, String)] {
        &self.measurement_drain_deferrals
    }
    /// Returns the in-memory acquisition capability pacing boundary, if withdrawn.
    pub fn acquisition_deferred_until(&self) -> Option<&str> {
        self.acquisition_deferred_until.as_deref()
    }
    /// Returns the reason the acquisition capability remains withdrawn.
    pub fn acquisition_deferred_reason(&self) -> Option<&str> {
        self.acquisition_deferred_reason.as_deref()
    }
    /// Returns the in-memory drain capability pacing boundary, if withdrawn.
    pub fn drain_deferred_until(&self) -> Option<&str> {
        self.drain_deferred_until.as_deref()
    }
    /// Returns the reason the source drain capability remains withdrawn.
    pub fn drain_deferred_reason(&self) -> Option<&str> {
        self.drain_deferred_reason.as_deref()
    }
}

const fn drain_is_failure(result: &DrainResult) -> bool {
    matches!(
        result,
        DrainResult::Retrying | DrainResult::DeadLettered | DrainResult::Unreachable
    )
}

/// Runs one finite tick under a newly acquired singleton graph lease.
pub fn agent_tick(graph: &TrustedGraphRoot, now_utc: String) -> Result<AgentTickResult, CoreError> {
    agent_tick_with_jobs(graph, now_utc, 1)
}

/// Runs one finite tick with bounded source-level concurrency under a fresh graph lease.
pub fn agent_tick_with_jobs(
    graph: &TrustedGraphRoot,
    now_utc: String,
    jobs: usize,
) -> Result<AgentTickResult, CoreError> {
    let Some(mut worker) = AgentWorker::try_start(graph)? else {
        return Err(CoreError::contract("agent is already running"));
    };
    worker.tick_with_jobs(graph, now_utc, jobs)
}

#[cfg(test)]
#[path = "agent/tests.rs"]
mod tests;

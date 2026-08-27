//! One locked source cycle, from lineage authority through a generation-atomic state commit.

use std::collections::BTreeMap;

use crate::{ConditionEvaluation, CoreError};

use super::{
    MeasurementId, SourceAcquireError, SourceCycleDecision, SourceId, SourceTurnEntry,
    TrustedGraphRoot, TrustedSourceDir, decide_source_cycle,
};

#[path = "engine/commit.rs"]
mod commit;
#[path = "engine/dry.rs"]
mod dry;
#[path = "engine/eligibility.rs"]
mod eligibility;
#[path = "engine/load.rs"]
mod load;
#[path = "engine/process.rs"]
mod process;
#[path = "engine/results.rs"]
mod results;
#[path = "engine/selection.rs"]
mod selection;
#[path = "engine/timing.rs"]
mod timing;

use commit::commit_acquisition;
use dry::turn_entry as dry_turn_entry;
use eligibility::{
    Eligible, SourceContext, eligible, initialize, source_context, with_source_state,
};
use load::{disabled, load};
use process::document_commit;
use results::{measurement, measurement_with_events, result};
use selection::selected_measurements;
use timing::{now, scheduled};

/// Result of one source cycle before delivery is drained independently.
#[derive(Clone, Debug)]
pub struct GraphMeasureResult {
    source_id: SourceId,
    status: GraphSourceStatus,
    measurements: Vec<GraphMeasurementResult>,
    source_event_envelopes: Vec<super::EventEnvelope>,
    source_outbox_overflow: Vec<super::OutboxOverflowFact>,
    source_failure: Option<super::SourceFetchFailure>,
    source_config_error: Option<String>,
    unresolvable_manifest: Option<super::UnresolvableManifest>,
    source_lineage_refusal: Option<super::SourceLineageRefusal>,
    source_health: Option<super::SourceAcquisitionHealth>,
    source_integration_fault_episode: Option<super::IntegrationFaultEpisode>,
}

/// Source-level acquisition disposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphSourceStatus {
    /// No acquisition occurred because the source is disabled.
    Disabled,
    /// Another writer holds the source lock, so no acquisition was attempted.
    Locked,
    /// A manifest anomaly requires a source reset.
    UnresolvableManifest,
    /// Source configuration cannot be decoded or validated.
    ConfigInvalid,
    /// Authoritative source lineage was refused.
    LineageRefused,
    /// No declared measurement can project under its current MVD or lineage state.
    AcquisitionHold,
    /// A complete document was projected and committed.
    Document,
    /// A complete cycle obtained no document and committed source state.
    NotModified,
    /// Acquisition failed and committed source health.
    FetchFailed,
    /// Acquisition was withheld by source-scoped integration fault.
    IntegrationFault,
}

/// Measurement disposition from the same shared source document.
#[derive(Clone, Debug)]
pub struct GraphMeasurementResult {
    measurement_id: MeasurementId,
    status: GraphMeasurementStatus,
    policy_evaluations: Vec<ConditionEvaluation>,
    event_envelopes: Vec<super::EventEnvelope>,
    outbox_overflow: Vec<super::OutboxOverflowFact>,
    config_error: Option<String>,
    lineage_hold: Option<super::MeasurementLineageHold>,
    stored_measurement_value_digest: Option<String>,
    current_measurement_value_digest: Option<String>,
    extraction_health: Option<super::MeasurementExtractionHealth>,
    integration_fault_episode: Option<super::IntegrationFaultEpisode>,
    observation: Option<crate::Observation>,
}

type LoadedMeasurement = (
    MeasurementId,
    Result<
        (
            super::MeasurementDocument,
            super::PreparedMeasurementProjection,
        ),
        CoreError,
    >,
);

/// Measurement-local acquisition and policy result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphMeasurementStatus {
    /// The first typed observation for this measurement lineage was accepted.
    Initialized,
    /// A typed observation with a different canonical value was accepted.
    Changed,
    /// A typed observation with the same canonical value was accepted.
    Unchanged,
    /// Extraction or typed parsing failed.
    ExtractionFailed,
    /// An FFHN or HTMLCut boundary invariant failed.
    IntegrationFault,
    /// Value-contract mismatch quarantined this measurement.
    Quarantined,
    /// The lineage gate held this measurement only.
    LineageHeld,
    /// Configuration or preflight was invalid.
    ConfigInvalid,
    /// The configured measurement is disabled.
    Disabled,
    /// No source document required projection.
    NotModified,
}

impl GraphMeasureResult {
    /// Returns the measured source identity.
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }
    /// Returns the source disposition.
    pub const fn status(&self) -> GraphSourceStatus {
        self.status
    }
    /// Returns per-measurement results in stable measurement-ID order.
    pub fn measurements(&self) -> &[GraphMeasurementResult] {
        &self.measurements
    }
    /// Returns route-independent source events staged by this cycle.
    pub fn source_event_envelopes(&self) -> &[super::EventEnvelope] {
        &self.source_event_envelopes
    }
    /// Returns source-owned event/route admissions refused by the bounded outbox.
    pub fn source_outbox_overflow(&self) -> &[super::OutboxOverflowFact] {
        &self.source_outbox_overflow
    }
    /// Returns the typed acquisition failure for a failed source cycle.
    pub fn source_failure(&self) -> Option<&super::SourceFetchFailure> {
        self.source_failure.as_ref()
    }
    /// Returns whether this source cycle contains any handled source or measurement failure.
    pub fn has_handled_failure(&self) -> bool {
        matches!(
            self.status,
            GraphSourceStatus::UnresolvableManifest
                | GraphSourceStatus::ConfigInvalid
                | GraphSourceStatus::LineageRefused
                | GraphSourceStatus::AcquisitionHold
                | GraphSourceStatus::FetchFailed
                | GraphSourceStatus::IntegrationFault
        ) || self.measurements.iter().any(|measurement| {
            matches!(
                measurement.status,
                GraphMeasurementStatus::ExtractionFailed
                    | GraphMeasurementStatus::IntegrationFault
                    | GraphMeasurementStatus::Quarantined
                    | GraphMeasurementStatus::LineageHeld
                    | GraphMeasurementStatus::ConfigInvalid
            )
        })
    }
    /// Returns the source configuration failure that withheld acquisition.
    pub fn source_config_error(&self) -> Option<&str> {
        self.source_config_error.as_deref()
    }
    /// Returns the unresolved durable commit-point class that withheld the source.
    pub const fn unresolvable_manifest(&self) -> Option<super::UnresolvableManifest> {
        self.unresolvable_manifest
    }
    /// Returns the authoritative source-lineage refusal reason.
    pub const fn source_lineage_refusal(&self) -> Option<super::SourceLineageRefusal> {
        self.source_lineage_refusal
    }
    /// Returns route-independent acquisition-health evidence after this cycle.
    pub const fn source_health(&self) -> Option<&super::SourceAcquisitionHealth> {
        self.source_health.as_ref()
    }
    /// Returns route-independent source integration-fault evidence after this cycle.
    pub const fn source_integration_fault_episode(
        &self,
    ) -> Option<&super::IntegrationFaultEpisode> {
        self.source_integration_fault_episode.as_ref()
    }
}

impl GraphMeasurementResult {
    /// Returns the measurement identity.
    pub fn measurement_id(&self) -> &MeasurementId {
        &self.measurement_id
    }
    /// Returns the measurement disposition.
    pub const fn status(&self) -> GraphMeasurementStatus {
        self.status
    }
    /// Returns staged policy decisions, never delivery outcomes.
    pub fn policy_evaluations(&self) -> &[ConditionEvaluation] {
        &self.policy_evaluations
    }
    /// Returns route-independent envelopes staged from the measurement policy result.
    pub fn event_envelopes(&self) -> &[super::EventEnvelope] {
        &self.event_envelopes
    }
    /// Returns admissions refused by this measurement's bounded outbox during this cycle.
    pub fn outbox_overflow(&self) -> &[super::OutboxOverflowFact] {
        &self.outbox_overflow
    }
    /// Returns the configuration/preflight failure for this measurement.
    pub fn config_error(&self) -> Option<&str> {
        self.config_error.as_deref()
    }
    /// Returns the measurement-scoped lineage-hold reason.
    pub const fn lineage_hold(&self) -> Option<super::MeasurementLineageHold> {
        self.lineage_hold
    }
    /// Returns the stored MVD that caused quarantine, when available.
    pub fn stored_measurement_value_digest(&self) -> Option<&str> {
        self.stored_measurement_value_digest.as_deref()
    }
    /// Returns the current configured MVD used for this result.
    pub fn current_measurement_value_digest(&self) -> Option<&str> {
        self.current_measurement_value_digest.as_deref()
    }
    /// Returns route-independent extraction-health evidence after this cycle.
    pub const fn extraction_health(&self) -> Option<&super::MeasurementExtractionHealth> {
        self.extraction_health.as_ref()
    }
    /// Returns route-independent measurement integration-fault evidence after this cycle.
    pub const fn integration_fault_episode(&self) -> Option<&super::IntegrationFaultEpisode> {
        self.integration_fault_episode.as_ref()
    }
    /// Returns the accepted typed observation produced by this cycle.
    pub const fn observation(&self) -> Option<&crate::Observation> {
        self.observation.as_ref()
    }
}

/// Runs one non-blocking locked acquisition cycle. Delivery draining remains a separate capability.
pub fn measure_source_once(
    graph: &TrustedGraphRoot,
    source_id: SourceId,
) -> Result<GraphMeasureResult, CoreError> {
    measure_source(graph, source_id, None, true)
}

/// Runs one live source cycle for the selected configured measurements only.
pub fn measure_selected_source_once(
    graph: &TrustedGraphRoot,
    source_id: SourceId,
    measurement_ids: Vec<MeasurementId>,
) -> Result<GraphMeasureResult, CoreError> {
    measure_source(graph, source_id, Some(measurement_ids), true)
}

/// Fetches, projects, parses, and evaluates one source without changing lineage, state, or outboxes.
pub fn measure_source_dry_run(
    graph: &TrustedGraphRoot,
    source_id: SourceId,
) -> Result<GraphMeasureResult, CoreError> {
    measure_source(graph, source_id, None, false)
}

/// Dry-runs exactly the selected configured measurements without changing any durable artifact.
pub fn measure_selected_source_dry_run(
    graph: &TrustedGraphRoot,
    source_id: SourceId,
    measurement_ids: Vec<MeasurementId>,
) -> Result<GraphMeasureResult, CoreError> {
    measure_source(graph, source_id, Some(measurement_ids), false)
}

fn measure_source(
    graph: &TrustedGraphRoot,
    source_id: SourceId,
    selection: Option<Vec<MeasurementId>>,
    persist: bool,
) -> Result<GraphMeasureResult, CoreError> {
    graph.validate_graph_documents()?;
    let graph_id = graph
        .read_graph_identity()?
        .ok_or_else(|| CoreError::contract("graph measurement requires graph identity"))?
        .graph_id()
        .clone();
    let source_dir = graph.open_source(source_id.clone())?;
    if persist {
        let Some(_lease) = source_dir.try_acquire_write_lease()? else {
            return Ok(result(source_id, GraphSourceStatus::Locked, Vec::new()));
        };
        return measure_locked(&source_dir, graph_id, true, selection);
    }
    let Some(_lease) = source_dir.try_acquire_read_lease()? else {
        return Ok(result(source_id, GraphSourceStatus::Locked, Vec::new()));
    };
    measure_locked(&source_dir, graph_id, false, selection)
}

fn measure_locked(
    source_dir: &TrustedSourceDir,
    graph_id: super::GraphId,
    persist: bool,
    selection: Option<Vec<MeasurementId>>,
) -> Result<GraphMeasureResult, CoreError> {
    let source_id = source_dir.paths().source_id().clone();
    let turn_entry = if persist {
        source_dir.recover_turn_entry()?
    } else {
        dry_turn_entry(source_dir)
    };
    if let SourceTurnEntry::Unresolvable(kind) = turn_entry {
        let mut report = result(
            source_id,
            GraphSourceStatus::UnresolvableManifest,
            Vec::new(),
        );
        report.unresolvable_manifest = Some(kind);
        return Ok(report);
    }
    let source = match source_dir.read_source_document() {
        Ok(source) => source,
        Err(error) => {
            let mut report = result(source_id, GraphSourceStatus::ConfigInvalid, Vec::new());
            report.source_config_error = Some(error.to_string());
            return Ok(report);
        }
    };
    let configured_ids = match source_dir.measurement_ids() {
        Ok(ids) => ids,
        Err(error) => {
            let mut report = result(source_id, GraphSourceStatus::ConfigInvalid, Vec::new());
            report.source_config_error = Some(error.to_string());
            return Ok(report);
        }
    };
    let ids = selected_measurements(configured_ids, selection)?;
    let loaded = ids
        .iter()
        .map(|id| load(source_dir, id))
        .collect::<Vec<_>>();
    if !source.enabled() {
        return Ok(result(
            source_id,
            GraphSourceStatus::Disabled,
            disabled(loaded),
        ));
    }
    let inspection = if persist {
        initialize(source_dir, &ids)?
    } else {
        source_dir.inspect_lineage(ids.iter().cloned())?
    };
    let (identity, source_state, authoritative_lineage) =
        match source_context(&inspection, persist)? {
            SourceContext::Ready {
                identity,
                state,
                authoritative_lineage,
            } => (*identity, *state, authoritative_lineage),
            SourceContext::Refused(reason) => {
                let mut report = result(source_id, GraphSourceStatus::LineageRefused, Vec::new());
                report.source_lineage_refusal = Some(reason);
                return Ok(report);
            }
        };
    let storage = persist.then(|| source_dir.open_storage()).transpose()?;
    let (eligible, mut outcomes) = eligible(&source, &inspection, loaded)?;
    if eligible.is_empty() {
        return Ok(with_source_state(
            result(source_id, GraphSourceStatus::AcquisitionHold, outcomes),
            &source_state,
        ));
    }
    let required = eligible.iter().any(|item| {
        item.state
            .as_ref()
            .is_none_or(|state| state.accepted_observation().is_none())
    });
    let validators = (source.conditional_enabled() && !required)
        .then(|| source_state.validators())
        .flatten();
    let now = now()?;
    match super::acquire_source_with_validators(&source, validators) {
        Err(SourceAcquireError::Fetch(failure)) => {
            let mut state = source_state;
            let report_failure = failure.clone();
            let failure_code = failure.kind.as_str().to_owned();
            let escalated =
                state.apply_acquisition_failure(failure, &now, source.escalate_after())?;
            let source_events = if authoritative_lineage && escalated {
                vec![super::event_materialize::materialize_source_event(
                    super::event_materialize::SourceEventParts {
                        graph_id: graph_id.clone(),
                        source_id: source.source_id().clone(),
                        source_instance_id: identity.source_instance_id().clone(),
                        source: &source,
                        state: &state,
                        event_kind: super::EventKind::SourceEscalation,
                        code: failure_code,
                        committed_at_utc: now.clone(),
                    },
                )?]
            } else {
                Vec::new()
            };
            let mut source_overflow = Vec::new();
            if let Some(storage) = &storage {
                let (source_records, overflow) = if source_events.is_empty() {
                    (Vec::new(), Vec::new())
                } else {
                    let admission = super::OutboxAdmission::admit(
                        &storage.read_source_delivery_records()?,
                        source_events.clone(),
                        source.routes(),
                        source.outbox(),
                        &now,
                    )?;
                    (admission.records().to_vec(), admission.overflow().to_vec())
                };
                source_overflow = overflow;
                state.set_outbox_overflow(source_overflow.clone())?;
                commit_acquisition(
                    source_dir,
                    storage,
                    &scheduled(&state, &source, &now)?,
                    Vec::new(),
                    BTreeMap::new(),
                    source_records,
                    Vec::new(),
                )?;
            }
            Ok(with_source_state(
                results::result_with_source_events(
                    source_id,
                    GraphSourceStatus::FetchFailed,
                    outcomes,
                    source_events,
                    source_overflow,
                    Some(report_failure),
                ),
                &state,
            ))
        }
        Err(SourceAcquireError::SecretUnavailable) => {
            let mut state = source_state;
            let began = state.apply_source_integration_fault(
                super::GraphIntegrationFaultCode::SecretUnavailable,
                now.clone(),
            )?;
            let source_events = if authoritative_lineage && began {
                vec![super::event_materialize::materialize_source_event(
                    super::event_materialize::SourceEventParts {
                        graph_id: graph_id.clone(),
                        source_id: source.source_id().clone(),
                        source_instance_id: identity.source_instance_id().clone(),
                        source: &source,
                        state: &state,
                        event_kind: super::EventKind::SourceIntegrationFault,
                        code: super::GraphIntegrationFaultCode::SecretUnavailable
                            .as_str()
                            .to_owned(),
                        committed_at_utc: now.clone(),
                    },
                )?]
            } else {
                Vec::new()
            };
            let mut source_overflow = Vec::new();
            if let Some(storage) = &storage {
                let (source_records, overflow) = if source_events.is_empty() {
                    (Vec::new(), Vec::new())
                } else {
                    let admission = super::OutboxAdmission::admit(
                        &storage.read_source_delivery_records()?,
                        source_events.clone(),
                        source.routes(),
                        source.outbox(),
                        &now,
                    )?;
                    (admission.records().to_vec(), admission.overflow().to_vec())
                };
                source_overflow = overflow;
                state.set_outbox_overflow(source_overflow.clone())?;
                commit_acquisition(
                    source_dir,
                    storage,
                    &scheduled(&state, &source, &now)?,
                    Vec::new(),
                    BTreeMap::new(),
                    source_records,
                    Vec::new(),
                )?;
            }
            Ok(with_source_state(
                results::result_with_source_events(
                    source_id,
                    GraphSourceStatus::IntegrationFault,
                    outcomes,
                    source_events,
                    source_overflow,
                    None,
                ),
                &state,
            ))
        }
        Ok(acquisition) => {
            match decide_source_cycle(&source, Some(&source_state), acquisition, required)? {
                SourceCycleDecision::NotModified { validators } => {
                    let mut state = source_state;
                    state.clear_transient_episodes();
                    if let Some(validators) = validators {
                        state = state.with_representation_facts(
                            Some(validators),
                            state.source_url().cloned(),
                            state.effective_base_url().cloned(),
                            None,
                            source.source_representation_digest()?,
                        )?;
                    }
                    outcomes.extend(eligible.into_iter().map(not_modified_measurement));
                    if let Some(storage) = &storage {
                        state.set_outbox_overflow(Vec::new())?;
                        commit_acquisition(
                            source_dir,
                            storage,
                            &scheduled(&state, &source, &now)?,
                            Vec::new(),
                            BTreeMap::new(),
                            Vec::new(),
                            Vec::new(),
                        )?;
                    }
                    Ok(with_source_state(
                        result(source_id, GraphSourceStatus::NotModified, outcomes),
                        &state,
                    ))
                }
                SourceCycleDecision::Document(document) => document_commit(
                    source_dir,
                    storage.as_ref(),
                    &source,
                    identity,
                    source_state,
                    &now,
                    document,
                    eligible,
                    outcomes,
                    graph_id,
                    authoritative_lineage,
                ),
            }
        }
    }
}

fn not_modified_measurement(item: Eligible) -> GraphMeasurementResult {
    let mut outcome = measurement(item.id, GraphMeasurementStatus::NotModified, Vec::new());
    outcome.current_measurement_value_digest = Some(item.mvd);
    if let Some(state) = item.state {
        outcome.extraction_health = Some(state.extraction_health().clone());
        outcome.integration_fault_episode = state.integration_fault_episode().cloned();
    }
    outcome
}

#[cfg(test)]
#[path = "engine/tests.rs"]
mod tests;

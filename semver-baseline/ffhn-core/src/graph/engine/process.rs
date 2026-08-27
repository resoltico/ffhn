//! Projection, policy, event materialization, and optional durable installation for one document.

use std::collections::BTreeMap;

use crate::CoreError;

use super::super::{
    MeasurementInstanceId, MeasurementProjectionFailure, MeasurementState, OutboxAdmission,
    SourceDocument, SourceDocumentBytes, SourceIdentity, SourceState, TrustedSourceDir,
    TrustedStorageDir,
};
use super::{
    Eligible, GraphMeasureResult, GraphMeasurementResult, GraphMeasurementStatus,
    GraphSourceStatus, measurement_with_events, scheduled,
};

/// Evaluates every eligible measurement over one source document and commits only when storage is
/// supplied. The absent-storage path is the real dry-run path, not a simulated renderer.
#[allow(clippy::too_many_arguments)]
pub(super) fn document_commit(
    source_dir: &TrustedSourceDir,
    storage: Option<&TrustedStorageDir>,
    source: &SourceDocument,
    identity: SourceIdentity,
    mut source_state: SourceState,
    now: &str,
    document: Box<SourceDocumentBytes>,
    eligible: Vec<Eligible>,
    mut outcomes: Vec<GraphMeasurementResult>,
    graph_id: super::super::GraphId,
    authoritative_lineage: bool,
) -> Result<GraphMeasureResult, CoreError> {
    let source_was_uninitialized = source_state.last_source_representation_digest().is_none();
    let (source_url, effective_base_url) = source_provenance(source, &document)?;
    source_state.clear_transient_episodes();
    source_state = source_state.with_representation_facts(
        document.validators.clone(),
        source_url,
        effective_base_url,
        document.file_content_sha256.clone(),
        source.source_representation_digest()?,
    )?;
    let source_events = (authoritative_lineage && source_was_uninitialized)
        .then(|| {
            super::super::event_materialize::materialize_source_initialized(
                graph_id.clone(),
                source.source_id().clone(),
                identity.source_instance_id().clone(),
                source,
                now.to_owned(),
            )
        })
        .transpose()?
        .into_iter()
        .collect::<Vec<_>>();
    let mut states = Vec::new();
    let mut additions = BTreeMap::new();
    let mut records = Vec::new();
    for item in eligible {
        let current_mvd = item.mvd.clone();
        let first = item.state.is_none();
        let instance = item
            .state
            .as_ref()
            .map(|state| state.measurement_instance_id().clone())
            .unwrap_or_else(MeasurementInstanceId::mint);
        let mut state = item.state.unwrap_or_else(|| {
            MeasurementState::fresh(identity.source_instance_id().clone(), instance.clone())
        });
        let previous_canonical = state
            .accepted_observation()
            .map(|observation| observation.canonical_value().to_owned());
        let was_uninitialized = previous_canonical.is_none();
        let (status, evaluations, events) =
            match item.projection.execute(&item.document, source, &document) {
                Ok(observation) => {
                    let accepted_status = match previous_canonical.as_deref() {
                        None => GraphMeasurementStatus::Initialized,
                        Some(previous) if previous == observation.canonical_value() => {
                            GraphMeasurementStatus::Unchanged
                        }
                        Some(_) => GraphMeasurementStatus::Changed,
                    };
                    let measurement_value_digest = item.mvd.clone();
                    match state.apply_accepted_observation(
                        &item.document,
                        observation,
                        item.mvd.clone(),
                    ) {
                        Ok(evaluations) => {
                            let mut events = Vec::new();
                            if authoritative_lineage && was_uninitialized {
                                events.push(
                                    super::super::event_materialize::materialize_measurement_event(
                                        super::super::event_materialize::MeasurementEventParts {
                                            graph_id: graph_id.clone(),
                                            source_id: source.source_id().clone(),
                                            source_instance_id: identity
                                                .source_instance_id()
                                                .clone(),
                                            measurement_id: item.id.clone(),
                                            measurement: &item.document,
                                            state: &state,
                                            event_kind:
                                                super::super::EventKind::MeasurementInitialized,
                                            code: None,
                                            measurement_value_digest,
                                            committed_at_utc: now.to_owned(),
                                        },
                                    )?,
                                );
                            }
                            if authoritative_lineage {
                                events.extend(
                                    super::super::event_materialize::materialize_condition_events(
                                        graph_id.clone(),
                                        source.source_id().clone(),
                                        identity.source_instance_id().clone(),
                                        item.id.clone(),
                                        &item.document,
                                        &state,
                                        &evaluations,
                                        now.to_owned(),
                                    )?,
                                );
                            }
                            (accepted_status, evaluations, events)
                        }
                        Err(error) => stage_policy_failure(
                            &error,
                            authoritative_lineage,
                            &graph_id,
                            source,
                            &identity,
                            &item.id,
                            &item.document,
                            &item.mvd,
                            &mut state,
                            now,
                        )?,
                    }
                }
                Err(failure) => stage_projection_failure(
                    failure,
                    authoritative_lineage,
                    &graph_id,
                    source,
                    &identity,
                    &item.id,
                    &item.document,
                    &item.mvd,
                    &mut state,
                    now,
                )?,
            };
        let mut outbox_overflow = Vec::new();
        if let Some(storage) = storage {
            let admission = OutboxAdmission::admit(
                &storage.read_measurement_delivery_records(&item.id)?,
                events.clone(),
                item.document.routes(),
                item.document.outbox(),
                now,
            )?;
            records.extend(
                admission
                    .records()
                    .iter()
                    .cloned()
                    .map(|record| (item.id.clone(), record)),
            );
            outbox_overflow = admission.overflow().to_vec();
            state.set_outbox_overflow(outbox_overflow.clone())?;
        }
        if first {
            additions.insert(item.id.clone(), instance);
        }
        let mut outcome = measurement_with_events(
            item.id.clone(),
            status,
            evaluations,
            events,
            outbox_overflow,
        );
        outcome.current_measurement_value_digest = Some(current_mvd);
        outcome.extraction_health = Some(state.extraction_health().clone());
        outcome.integration_fault_episode = state.integration_fault_episode().cloned();
        if matches!(
            status,
            GraphMeasurementStatus::Initialized
                | GraphMeasurementStatus::Changed
                | GraphMeasurementStatus::Unchanged
        ) {
            outcome.observation = state.accepted_observation().cloned();
        }
        outcomes.push(outcome);
        states.push((item.id, state));
    }
    let mut source_overflow = Vec::new();
    if let Some(storage) = storage {
        let (source_records, overflow) = if source_events.is_empty() {
            (Vec::new(), Vec::new())
        } else {
            let admission = OutboxAdmission::admit(
                &storage.read_source_delivery_records()?,
                source_events.clone(),
                source.routes(),
                source.outbox(),
                now,
            )?;
            (admission.records().to_vec(), admission.overflow().to_vec())
        };
        source_overflow = overflow;
        source_state.set_outbox_overflow(source_overflow.clone())?;
        super::commit::commit_acquisition(
            source_dir,
            storage,
            &scheduled(&source_state, source, now)?,
            states,
            additions,
            source_records,
            records,
        )?;
    }
    outcomes.sort_unstable_by(|left, right| left.measurement_id.cmp(&right.measurement_id));
    Ok(super::with_source_state(
        super::results::result_with_source_events(
            source.source_id().clone(),
            GraphSourceStatus::Document,
            outcomes,
            source_events,
            source_overflow,
            None,
        ),
        &source_state,
    ))
}

pub(super) fn source_provenance(
    source: &SourceDocument,
    document: &SourceDocumentBytes,
) -> Result<(Option<url::Url>, Option<url::Url>), CoreError> {
    match (
        source.fetch(),
        document.effective_http_url.as_ref(),
        document.file_content_sha256.as_ref(),
    ) {
        (super::super::SourceFetch::Http { source_url, .. }, Some(effective), None) => {
            Ok((Some(source_url.clone()), Some(effective.clone())))
        }
        (super::super::SourceFetch::File { .. }, None, Some(_)) => Ok((None, None)),
        _ => Err(CoreError::internal(
            "accepted source document provenance disagrees with its fetch contract",
        )),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn stage_projection_failure(
    failure: MeasurementProjectionFailure,
    authoritative_lineage: bool,
    graph_id: &super::super::GraphId,
    source: &SourceDocument,
    identity: &SourceIdentity,
    measurement_id: &super::super::MeasurementId,
    measurement: &super::super::MeasurementDocument,
    measurement_value_digest: &str,
    state: &mut MeasurementState,
    now: &str,
) -> Result<
    (
        GraphMeasurementStatus,
        Vec<crate::ConditionEvaluation>,
        Vec<super::super::EventEnvelope>,
    ),
    CoreError,
> {
    let (status, event_kind, code, emit) = match failure {
        MeasurementProjectionFailure::Extraction(reason) => {
            let escalated =
                state.apply_extraction_failure(reason, now, measurement.escalate_after())?;
            (
                GraphMeasurementStatus::ExtractionFailed,
                super::super::EventKind::ExtractionEscalation,
                reason.as_str(),
                escalated,
            )
        }
        MeasurementProjectionFailure::Integration(code) => {
            let began = state.apply_measurement_integration_fault(code, now.to_owned())?;
            (
                GraphMeasurementStatus::IntegrationFault,
                super::super::EventKind::MeasurementIntegrationFault,
                code.as_str(),
                began,
            )
        }
    };
    let events = (authoritative_lineage && emit)
        .then(|| {
            super::super::event_materialize::materialize_measurement_event(
                super::super::event_materialize::MeasurementEventParts {
                    graph_id: graph_id.clone(),
                    source_id: source.source_id().clone(),
                    source_instance_id: identity.source_instance_id().clone(),
                    measurement_id: measurement_id.clone(),
                    measurement,
                    state,
                    event_kind,
                    code: Some(code.to_owned()),
                    measurement_value_digest: measurement_value_digest.to_owned(),
                    committed_at_utc: now.to_owned(),
                },
            )
        })
        .transpose()?
        .into_iter()
        .collect();
    Ok((status, Vec::new(), events))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn stage_policy_failure(
    error: &CoreError,
    authoritative_lineage: bool,
    graph_id: &super::super::GraphId,
    source: &SourceDocument,
    identity: &SourceIdentity,
    measurement_id: &super::super::MeasurementId,
    measurement: &super::super::MeasurementDocument,
    measurement_value_digest: &str,
    state: &mut MeasurementState,
    now: &str,
) -> Result<
    (
        GraphMeasurementStatus,
        Vec<crate::ConditionEvaluation>,
        Vec<super::super::EventEnvelope>,
    ),
    CoreError,
> {
    stage_projection_failure(
        MeasurementProjectionFailure::Integration(policy_error_code(error)),
        authoritative_lineage,
        graph_id,
        source,
        identity,
        measurement_id,
        measurement,
        measurement_value_digest,
        state,
        now,
    )
}

pub(super) const fn policy_error_code(
    error: &CoreError,
) -> super::super::GraphIntegrationFaultCode {
    if matches!(error, CoreError::PolicyInvariant(_)) {
        super::super::GraphIntegrationFaultCode::FfhnPolicyInvariantViolation
    } else {
        super::super::GraphIntegrationFaultCode::FfhnBoundaryInvariantViolation
    }
}

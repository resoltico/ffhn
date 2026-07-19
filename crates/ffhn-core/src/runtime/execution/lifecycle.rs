use std::collections::BTreeMap;

use crate::{
    ConditionEvaluation, CoreError, DiagnosticDetail, DiagnosticKind, DiagnosticOperation,
    IntegrationFaultCode, LifecycleSnapshot, OnRunEventCause, PermanentErrorCode, PolicyEvaluation,
    PolicyRunInput, RunMode, RunOutcome, RunReport, SourceSuspectReason, StagedEventEligibility,
    StagedPolicyRun, StateDocument, TargetDocument, TargetPaths,
};

use super::super::acquire::{MeasurementAcquisitionFailure, now_utc};
use super::super::delivery::{self, DeliveryDrainFailure};
use super::super::report::{
    DeliveryEvidence, FinishReport, detail_from_error_for_operation, finish_report,
};
use super::super::storage::write_state;
use crate::model::integration_detail;

pub(in crate::runtime) struct StatefulRun<'a> {
    pub(in crate::runtime) paths: &'a TargetPaths,
    pub(in crate::runtime) target: &'a TargetDocument,
    pub(in crate::runtime) mode: RunMode,
    pub(in crate::runtime) started: String,
    pub(in crate::runtime) digest: String,
    pub(in crate::runtime) previous: Option<String>,
    pub(in crate::runtime) lifecycle_before: Option<LifecycleSnapshot>,
}

/// One complete in-memory T0 transaction plan.
///
/// The policy decision and its resulting state transition are inseparable: M4 must extend this
/// exact plan with immutable outbox records rather than re-evaluating policy after state changes.
pub(in crate::runtime) struct StagedRun {
    pub(in crate::runtime) next_state: StateDocument,
    condition_evaluations: Option<Vec<ConditionEvaluation>>,
    pub(in crate::runtime) event_eligibilities: Vec<StagedEventEligibility>,
}

impl StagedRun {
    pub(in crate::runtime) fn new(
        next_state: StateDocument,
        policy: StagedPolicyRun,
        initialized: bool,
    ) -> Self {
        let condition_evaluations = policy.condition_evaluations().map(ToOwned::to_owned);
        let mut event_eligibilities = policy.event_eligibilities().to_vec();
        if initialized {
            event_eligibilities.push(StagedEventEligibility::OnRun {
                cause: OnRunEventCause::Initialized,
            });
        }
        Self {
            next_state,
            condition_evaluations,
            event_eligibilities,
        }
    }

    pub(in crate::runtime) fn from_eligibilities(
        next_state: StateDocument,
        event_eligibilities: Vec<StagedEventEligibility>,
    ) -> Self {
        Self {
            next_state,
            condition_evaluations: None,
            event_eligibilities,
        }
    }

    pub(in crate::runtime) fn next_state(&self) -> &StateDocument {
        &self.next_state
    }

    pub(in crate::runtime) fn event_eligibilities(&self) -> &[StagedEventEligibility] {
        &self.event_eligibilities
    }

    pub(in crate::runtime) fn policy_evaluation(&self) -> PolicyEvaluation {
        PolicyEvaluation::from_staged(
            self.condition_evaluations.as_deref(),
            &self.event_eligibilities,
        )
    }
}

pub(in crate::runtime) fn finish_source_suspect(
    run: StatefulRun<'_>,
    state: &StateDocument,
    outcome: RunOutcome,
    reason: SourceSuspectReason,
    detail: DiagnosticDetail,
) -> Result<RunReport, CoreError> {
    let mut next_state = state.clone();
    let escalation_reached = next_state.apply_source_suspect(
        reason,
        detail.clone(),
        &run.started,
        run.target.escalate_after(),
    )?;
    let staged = run.target.stage_policy_run(
        PolicyRunInput::SourceSuspect {
            reason_class: reason,
            escalation_reached,
        },
        &BTreeMap::new(),
    )?;
    let staged_run = StagedRun::new(next_state, staged, false);
    ensure_failure_event_staging(
        &staged_run,
        escalation_reached.then_some(OnRunEventCause::SourceSuspectEscalated {
            reason_class: reason,
        }),
    )?;
    let policy_evaluation = staged_run.policy_evaluation();
    let lifecycle_after = staged_run.next_state().lifecycle_snapshot()?;
    let commit = match commit_staged_run(run.paths, run.target, run.mode, &staged_run, &run.digest)
    {
        Ok(commit) => commit,
        Err(failure) => {
            return finish_report(FinishReport {
                target: run.target,
                mode: run.mode,
                outcome: RunOutcome::PersistFailed,
                started: run.started,
                digest: Some(run.digest),
                observation: None,
                previous: run.previous,
                error: Some(detail_from_error_for_operation(
                    &failure.error,
                    DiagnosticOperation::StateCommit,
                    Some(run.paths.state_file()),
                )),
                policy_evaluation: policy_evaluation.clone(),
                lifecycle_before: run.lifecycle_before.clone(),
                lifecycle_after: Some(lifecycle_after.clone()),
                persisted: failure.persisted,
                delivery: failure.delivery,
            });
        }
    };
    finish_report(FinishReport {
        target: run.target,
        mode: run.mode,
        outcome,
        started: run.started,
        digest: Some(run.digest),
        observation: None,
        previous: run.previous,
        error: Some(detail),
        policy_evaluation,
        lifecycle_before: run.lifecycle_before,
        lifecycle_after: Some(lifecycle_after),
        persisted: commit.persisted,
        delivery: commit.delivery,
    })
}

pub(in crate::runtime) fn finish_measurement_acquisition_failure(
    run: StatefulRun<'_>,
    state: &StateDocument,
    failure: MeasurementAcquisitionFailure,
) -> Result<RunReport, CoreError> {
    match failure {
        MeasurementAcquisitionFailure::SourceSuspect(failure) => finish_source_suspect(
            run,
            state,
            RunOutcome::AcquisitionFailed,
            failure.reason,
            failure.detail,
        ),
        MeasurementAcquisitionFailure::Permanent { code, detail } => {
            finish_permanent_error(run, state, code, detail)
        }
        MeasurementAcquisitionFailure::Integration { detail } => {
            finish_integration_fault(run, state, detail)
        }
    }
}

pub(in crate::runtime) fn finish_permanent_error(
    run: StatefulRun<'_>,
    state: &StateDocument,
    error_code: PermanentErrorCode,
    detail: DiagnosticDetail,
) -> Result<RunReport, CoreError> {
    let mut next_state = state.clone();
    let episode_began = next_state.apply_permanent_error(error_code, &run.started)?;
    let staged = run.target.stage_policy_run(
        PolicyRunInput::PermanentContractError {
            error_code,
            episode_began,
        },
        &BTreeMap::new(),
    )?;
    let staged_run = StagedRun::new(next_state, staged, false);
    ensure_failure_event_staging(
        &staged_run,
        episode_began.then_some(OnRunEventCause::PermanentContractErrorEpisodeBegan { error_code }),
    )?;
    let policy_evaluation = staged_run.policy_evaluation();
    let lifecycle_after = staged_run.next_state().lifecycle_snapshot()?;
    let commit = match commit_staged_run(run.paths, run.target, run.mode, &staged_run, &run.digest)
    {
        Ok(commit) => commit,
        Err(failure) => {
            return finish_report(FinishReport {
                target: run.target,
                mode: run.mode,
                outcome: RunOutcome::PersistFailed,
                started: run.started,
                digest: Some(run.digest),
                observation: None,
                previous: run.previous,
                error: Some(detail_from_error_for_operation(
                    &failure.error,
                    DiagnosticOperation::StateCommit,
                    Some(run.paths.state_file()),
                )),
                policy_evaluation: policy_evaluation.clone(),
                lifecycle_before: run.lifecycle_before.clone(),
                lifecycle_after: Some(lifecycle_after.clone()),
                persisted: failure.persisted,
                delivery: failure.delivery,
            });
        }
    };
    finish_report(FinishReport {
        target: run.target,
        mode: run.mode,
        outcome: RunOutcome::ConfigInvalid,
        started: run.started,
        digest: Some(run.digest),
        observation: None,
        previous: run.previous,
        error: Some(detail),
        policy_evaluation,
        lifecycle_before: run.lifecycle_before,
        lifecycle_after: Some(lifecycle_after),
        persisted: commit.persisted,
        delivery: commit.delivery,
    })
}

pub(in crate::runtime) fn finish_integration_fault(
    run: StatefulRun<'_>,
    state: &StateDocument,
    detail: DiagnosticDetail,
) -> Result<RunReport, CoreError> {
    finish_integration_fault_with_observation(run, state, detail, None)
}

/// Records an FFHN exact-policy invariant failure without accepting the observed value.
///
/// The report retains the parsed observation for diagnosis, while the state keeps its prior
/// accepted observation and temporal condition facts. A batch therefore loses only this target.
pub(in crate::runtime) fn finish_policy_invariant(
    run: StatefulRun<'_>,
    state: &StateDocument,
    observation: crate::Observation,
    message: String,
) -> Result<RunReport, CoreError> {
    finish_integration_fault_with_observation(
        run,
        state,
        integration_detail(
            DiagnosticKind::PolicyInvariant,
            DiagnosticOperation::PolicyEvaluation,
            message,
            IntegrationFaultCode::FfhnPolicyInvariantViolation,
        ),
        Some(observation),
    )
}

fn finish_integration_fault_with_observation(
    run: StatefulRun<'_>,
    state: &StateDocument,
    detail: DiagnosticDetail,
    observation: Option<crate::Observation>,
) -> Result<RunReport, CoreError> {
    let integration_fault_code = detail.integration_fault_code().ok_or_else(|| {
        CoreError::contract(
            "integration-fault acquisition failure must carry integration_fault_code",
        )
    })?;
    let mut next_state = state.clone();
    let episode_began = next_state.apply_integration_fault(integration_fault_code, &run.started)?;
    let staged = run.target.stage_policy_run(
        PolicyRunInput::IntegrationFault {
            integration_fault_code,
            episode_began,
        },
        &BTreeMap::new(),
    )?;
    let staged_run = StagedRun::new(next_state, staged, false);
    ensure_failure_event_staging(
        &staged_run,
        episode_began.then_some(OnRunEventCause::IntegrationFaultEpisodeBegan {
            integration_fault_code,
        }),
    )?;
    let policy_evaluation = staged_run.policy_evaluation();
    let lifecycle_after = staged_run.next_state().lifecycle_snapshot()?;
    let commit = match commit_staged_run(run.paths, run.target, run.mode, &staged_run, &run.digest)
    {
        Ok(commit) => commit,
        Err(failure) => {
            return finish_report(FinishReport {
                target: run.target,
                mode: run.mode,
                outcome: RunOutcome::PersistFailed,
                started: run.started,
                digest: Some(run.digest),
                observation: observation.clone(),
                previous: run.previous,
                error: Some(detail_from_error_for_operation(
                    &failure.error,
                    DiagnosticOperation::StateCommit,
                    Some(run.paths.state_file()),
                )),
                policy_evaluation: policy_evaluation.clone(),
                lifecycle_before: run.lifecycle_before.clone(),
                lifecycle_after: Some(lifecycle_after.clone()),
                persisted: failure.persisted,
                delivery: failure.delivery,
            });
        }
    };
    finish_report(FinishReport {
        target: run.target,
        mode: run.mode,
        outcome: RunOutcome::IntegrationFault,
        started: run.started,
        digest: Some(run.digest),
        observation,
        previous: run.previous,
        error: Some(detail),
        policy_evaluation,
        lifecycle_before: run.lifecycle_before,
        lifecycle_after: Some(lifecycle_after),
        persisted: commit.persisted,
        delivery: commit.delivery,
    })
}

pub(in crate::runtime) fn ensure_failure_event_staging(
    staged: &StagedRun,
    expected_cause: Option<OnRunEventCause>,
) -> Result<(), CoreError> {
    match (expected_cause, staged.event_eligibilities()) {
        (None, []) => Ok(()),
        (Some(expected), [StagedEventEligibility::OnRun { cause }]) if cause == &expected => Ok(()),
        _ => Err(CoreError::internal(
            "failure policy staging did not preserve the state-owned event eligibility",
        )),
    }
}

pub(in crate::runtime) fn valid_condition_evaluations(
    staged: &StagedPolicyRun,
) -> Result<&[crate::ConditionEvaluation], CoreError> {
    staged.condition_evaluations().ok_or_else(|| {
        CoreError::internal("valid policy staging did not produce valid-observation evaluations")
    })
}

/// Builds the complete valid-observation transaction plan before any persistence occurs.
///
/// Keeping all fallible staging in this narrow boundary lets its invariants be tested without
/// filesystem side effects, while `run_once_with_mode` remains the sole live coordinator.
pub(in crate::runtime) fn stage_valid_observation_run(
    target: &TargetDocument,
    state: &StateDocument,
    observation: crate::Observation,
    started: &str,
) -> Result<StagedRun, CoreError> {
    let contexts = state.condition_contexts(target);
    let staged = target.stage_policy_run(
        PolicyRunInput::ValidObservation {
            observation: &observation,
        },
        &contexts,
    )?;
    let condition_evaluations = valid_condition_evaluations(&staged)?;
    let mut next_state = state.clone();
    let initialized = state.accepted_observation().is_none();
    next_state.apply_valid_observation(target, observation, condition_evaluations, started)?;
    Ok(StagedRun::new(next_state, staged, initialized))
}

pub(in crate::runtime) struct CommitResult {
    pub(in crate::runtime) persisted: bool,
    pub(in crate::runtime) delivery: DeliveryEvidence,
}

pub(in crate::runtime) struct CommitFailure {
    pub(in crate::runtime) error: CoreError,
    pub(in crate::runtime) persisted: bool,
    pub(in crate::runtime) delivery: DeliveryEvidence,
}

/// Commits the complete staged T0 plan and only then drains durable records.
///
/// The first state write contains both M3 state and all materialized immutable outbox records.
/// Delivery starts strictly after that crash-durable write succeeds and every delivery-state update
/// remains under the same target lock.
pub(in crate::runtime) fn commit_staged_run(
    paths: &TargetPaths,
    target: &TargetDocument,
    mode: RunMode,
    staged: &StagedRun,
    contract_digest_sha256: &str,
) -> Result<CommitResult, Box<CommitFailure>> {
    commit_staged_run_with_clock(
        paths,
        target,
        mode,
        staged,
        contract_digest_sha256,
        &mut now_utc,
    )
}

/// Commits a staged run using the supplied clock.
///
/// Injecting the clock confines deterministic testing of the pre-commit failure contract to this
/// boundary; production always obtains the canonical UTC clock through `now_utc`.
pub(in crate::runtime) fn commit_staged_run_with_clock<F>(
    paths: &TargetPaths,
    target: &TargetDocument,
    mode: RunMode,
    staged: &StagedRun,
    contract_digest_sha256: &str,
    now: &mut F,
) -> Result<CommitResult, Box<CommitFailure>>
where
    F: FnMut() -> Result<String, CoreError>,
{
    commit_staged_run_with_clock_and_persist(
        paths,
        target,
        mode,
        staged,
        contract_digest_sha256,
        now,
        &mut |state| write_state(paths, state),
    )
}

/// Commits a staged run using injected pre-delivery boundaries.
///
/// The persist function is kept at this boundary so tests can prove a failed durable commit cannot
/// enter delivery. Production always uses the storage writer above.
pub(in crate::runtime) fn commit_staged_run_with_clock_and_persist<F, P>(
    paths: &TargetPaths,
    target: &TargetDocument,
    mode: RunMode,
    staged: &StagedRun,
    contract_digest_sha256: &str,
    now: &mut F,
    persist: &mut P,
) -> Result<CommitResult, Box<CommitFailure>>
where
    F: FnMut() -> Result<String, CoreError>,
    P: FnMut(&StateDocument) -> Result<(), CoreError>,
{
    let commit_time = now().map_err(|error| {
        Box::new(CommitFailure {
            error,
            persisted: false,
            delivery: DeliveryEvidence::default(),
        })
    })?;
    let mut state = staged.next_state().clone();
    let records = delivery::materialize(
        target,
        &state,
        staged.event_eligibilities(),
        contract_digest_sha256,
    )
    .map_err(|error| {
        Box::new(CommitFailure {
            error,
            persisted: false,
            delivery: DeliveryEvidence::default(),
        })
    })?;
    let overflow = state
        .enqueue_outbox(records, target.outbox(), &commit_time)
        .map_err(|error| {
            Box::new(CommitFailure {
                error,
                persisted: false,
                delivery: DeliveryEvidence::default(),
            })
        })?;
    let mut delivery = DeliveryEvidence {
        outcomes: Vec::new(),
        overflow,
        outbox_error_detail: None,
    };
    if mode == RunMode::DryRun {
        return Ok(CommitResult {
            persisted: false,
            delivery,
        });
    }
    persist(&state).map_err(|error| {
        Box::new(CommitFailure {
            error,
            persisted: false,
            delivery: delivery.clone(),
        })
    })?;
    match delivery::drain(paths, target, &mut state) {
        Ok(drain) => {
            delivery.outcomes = drain.outcomes();
            Ok(CommitResult {
                persisted: true,
                delivery,
            })
        }
        Err(DeliveryDrainFailure { drain, error }) => {
            delivery.outcomes = drain.outcomes();
            let operation = delivery
                .outcomes
                .last()
                .and_then(|outcome| outcome.outbox_error_detail())
                .map(DiagnosticDetail::operation)
                .unwrap_or(crate::DiagnosticOperation::OutboxDrain);
            delivery.outbox_error_detail = Some(crate::model::detail_from_core_error(
                &error, operation, None,
            ));
            Err(Box::new(CommitFailure {
                error,
                persisted: true,
                delivery,
            }))
        }
    }
}

pub(in crate::runtime) fn previous_value(state: &StateDocument) -> Option<String> {
    state
        .accepted_observation()
        .map(|observation| observation.canonical_value().to_owned())
}

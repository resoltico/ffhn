use std::collections::BTreeMap;
use std::io;
use std::thread;

use crate::{
    BatchRunReport, CoreError, OnRunEventCause, PermanentErrorCode, PolicyRunInput,
    ProcessErrorDetail, ProcessErrorKind, ResetReport, RouteFamily, RunMode, RunOutcome, RunReport,
    SourceSuspectReason, StagedEventEligibility, StagedPolicyRun, StateDocument, StatusKind,
    StatusReport, TargetDocument, TargetPaths,
};

use super::acquire::{
    AcquiredMeasurement, MeasurementAcquisitionFailure, acquire_measurement, fetch_source, now_utc,
};
use super::delivery::{self, DeliveryDrainFailure};
use super::lock::{LockError, lock_exclusive, lock_shared};
use super::report::{
    DeliveryEvidence, FinishReport, detail_from_error, finish_report, report_for_target_load,
    target_load_status_kind,
};
use super::storage::{blind_remove_storage_root, load_state, load_target, write_state};

/// Validates and returns the current target document.
pub(crate) fn validate_target(paths: &TargetPaths) -> Result<TargetDocument, CoreError> {
    paths.require_watch_root_directory()?;
    let target = load_target(paths)?;
    target.validate()?;
    Ok(target)
}

/// Blindly clears target storage and, when a valid delivery configuration remains, durably stages
/// the fresh-lifecycle reset event after the delete has completed.
pub(crate) fn reset(paths: &TargetPaths) -> Result<ResetReport, CoreError> {
    paths.require_watch_root_directory()?;
    let _lock = match lock_exclusive(paths) {
        Ok(lock) => lock,
        Err(LockError::Unavailable) => {
            return Err(CoreError::io(
                paths.run_lock_file(),
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "target lock is held by another live run",
                ),
            ));
        }
        Err(LockError::Io(error)) => return Err(error),
    };
    let storage_cleared = blind_remove_storage_root(&paths.storage_root())?;
    let mut delivery = DeliveryEvidence::default();
    if let Ok(target) = load_target(paths)
        && target.validate_without_projection().is_ok()
        && target.permanent_error().is_none()
        && target.routes_for(RouteFamily::OnRun).next().is_some()
    {
        let digest = target.contract_digest_sha256()?;
        let state = StateDocument::new(crate::TargetId::new(target.target_id())?, digest.clone());
        let staged = StagedRun::from_eligibilities(
            state,
            vec![StagedEventEligibility::OnRun {
                cause: OnRunEventCause::Reset,
            }],
        );
        delivery = match commit_staged_run(paths, &target, RunMode::Live, &staged, &digest) {
            Ok(commit) => commit.delivery,
            Err(failure) => {
                let mut delivery = failure.delivery;
                delivery.outbox_error = Some(failure.error.to_string());
                delivery
            }
        };
    }
    Ok(ResetReport::new(
        paths.target_id(),
        storage_cleared,
        delivery.outcomes,
        delivery.overflow,
        delivery.outbox_error,
    ))
}

/// Executes a live target run.
pub(crate) fn run_once(paths: &TargetPaths) -> Result<RunReport, CoreError> {
    run_once_with_mode(paths, RunMode::Live)
}

/// Executes one target under the requested mode.
pub(crate) fn run_once_with_mode(
    paths: &TargetPaths,
    mode: RunMode,
) -> Result<RunReport, CoreError> {
    paths.require_watch_root_directory()?;
    let started = now_utc()?;
    let target = match load_target(paths) {
        Ok(target) => target,
        Err(error) => return report_for_target_load(paths, mode, started, error),
    };
    if let Err(error) = target.validate_without_projection() {
        return report_for_target_load(paths, mode, started, error);
    }
    let digest = target.contract_digest_sha256()?;
    let lock = match mode {
        RunMode::Live => match lock_exclusive(paths) {
            Ok(lock) => lock,
            Err(LockError::Unavailable) => {
                return finish_report(FinishReport {
                    target: &target,
                    mode,
                    outcome: RunOutcome::LockUnavailable,
                    started,
                    digest: Some(digest),
                    observation: None,
                    previous: None,
                    error: Some(ProcessErrorDetail::new(
                        ProcessErrorKind::Io,
                        "target lock is held by another live run",
                        Some(paths.run_lock_file().display().to_string()),
                    )),
                    persisted: false,
                    delivery: DeliveryEvidence::default(),
                });
            }
            Err(LockError::Io(error)) => return Err(error),
        },
        RunMode::DryRun => lock_shared(paths)?,
    };
    let _lock = lock;
    let prior = match load_state(paths) {
        Ok(prior) => prior,
        Err(error) => {
            return finish_report(FinishReport {
                target: &target,
                mode,
                outcome: RunOutcome::StateInvalid,
                started,
                digest: Some(digest),
                observation: None,
                previous: None,
                error: Some(detail_from_error(&error, Some(paths.state_file()))),
                persisted: false,
                delivery: DeliveryEvidence::default(),
            });
        }
    };
    if let Some(state) = &prior
        && state.contract_digest_sha256() != digest
    {
        return finish_report(FinishReport {
            target: &target,
            mode,
            outcome: RunOutcome::RefusedContractDigest,
            started,
            digest: Some(digest),
            observation: None,
            previous: previous_value(state),
            error: Some(ProcessErrorDetail::new(
                ProcessErrorKind::Contract,
                "stored state belongs to a different measurement contract; run `ffhn reset --target <ID>` before running this target",
                Some(paths.state_file().display().to_string()),
            )),
            persisted: false,
            delivery: DeliveryEvidence::default(),
        });
    }
    let state = match prior {
        Some(state) => state,
        None => StateDocument::new(crate::TargetId::new(target.target_id())?, digest.clone()),
    };
    if let Err(error) = state.validate_for_target(&target) {
        return finish_report(FinishReport {
            target: &target,
            mode,
            outcome: RunOutcome::StateInvalid,
            started,
            digest: Some(digest),
            observation: None,
            previous: previous_value(&state),
            error: Some(detail_from_error(&error, Some(paths.state_file()))),
            persisted: false,
            delivery: DeliveryEvidence::default(),
        });
    }
    let previous = previous_value(&state);
    if !target.enabled() {
        return finish_report(FinishReport {
            target: &target,
            mode,
            outcome: RunOutcome::SkippedDisabled,
            started,
            digest: Some(digest),
            observation: None,
            previous,
            error: None,
            persisted: false,
            delivery: DeliveryEvidence::default(),
        });
    }
    if let Some(error) = target.permanent_error() {
        let error_code = error.code();
        return finish_permanent_error(
            StatefulRun {
                paths,
                target: &target,
                mode,
                started,
                digest,
                previous,
            },
            &state,
            error_code,
            error.into_process_error_detail(Some(paths.target_file().display().to_string())),
        );
    }
    let source = match fetch_source(&target) {
        Ok(source) => source,
        Err(detail) => {
            return finish_source_suspect(
                StatefulRun {
                    paths,
                    target: &target,
                    mode,
                    started,
                    digest,
                    previous,
                },
                &state,
                RunOutcome::FetchFailed,
                SourceSuspectReason::FetchFailed,
                detail,
            );
        }
    };
    let acquired =
        match acquire_measurement(&target, &source.body, source.effective_http_url.as_ref()) {
            Ok(acquired) => acquired,
            Err(MeasurementAcquisitionFailure::SourceSuspect(failure)) => {
                return finish_source_suspect(
                    StatefulRun {
                        paths,
                        target: &target,
                        mode,
                        started,
                        digest,
                        previous,
                    },
                    &state,
                    RunOutcome::AcquisitionFailed,
                    failure.reason,
                    failure.detail,
                );
            }
            Err(MeasurementAcquisitionFailure::Permanent { code, detail }) => {
                return finish_permanent_error(
                    StatefulRun {
                        paths,
                        target: &target,
                        mode,
                        started,
                        digest,
                        previous,
                    },
                    &state,
                    code,
                    detail,
                );
            }
        };
    let observation = match acquired {
        AcquiredMeasurement::JsonScalar(raw) => target.parse_json_scalar_token(raw),
        AcquiredMeasurement::Html(input) => target.parse_html_projection(input),
    };
    let observation = match observation {
        Ok(observation) => observation,
        Err(detail) => {
            return finish_source_suspect(
                StatefulRun {
                    paths,
                    target: &target,
                    mode,
                    started,
                    digest,
                    previous,
                },
                &state,
                RunOutcome::ValueUnparseable,
                SourceSuspectReason::ValueUnparseable,
                detail,
            );
        }
    };
    let outcome = match previous.as_deref() {
        None => RunOutcome::Initialized,
        Some(previous) if previous == observation.canonical_value() => RunOutcome::Unchanged,
        Some(_) => RunOutcome::Changed,
    };
    let staged_run = stage_valid_observation_run(&target, &state, observation.clone(), &started)?;
    let commit = match commit_staged_run(paths, &target, mode, &staged_run, &digest) {
        Ok(commit) => commit,
        Err(failure) => {
            return finish_report(FinishReport {
                target: &target,
                mode,
                outcome: RunOutcome::PersistFailed,
                started,
                digest: Some(digest),
                observation: Some(observation),
                previous,
                error: Some(detail_from_error(&failure.error, Some(paths.state_file()))),
                persisted: failure.persisted,
                delivery: failure.delivery,
            });
        }
    };
    finish_report(FinishReport {
        target: &target,
        mode,
        outcome,
        started,
        digest: Some(digest),
        observation: Some(observation),
        previous,
        error: None,
        persisted: commit.persisted,
        delivery: commit.delivery,
    })
}

struct StatefulRun<'a> {
    paths: &'a TargetPaths,
    target: &'a TargetDocument,
    mode: RunMode,
    started: String,
    digest: String,
    previous: Option<String>,
}

/// One complete in-memory T0 transaction plan.
///
/// The policy decision and its resulting state transition are inseparable: M4 must extend this
/// exact plan with immutable outbox records rather than re-evaluating policy after state changes.
struct StagedRun {
    next_state: StateDocument,
    event_eligibilities: Vec<StagedEventEligibility>,
}

impl StagedRun {
    fn new(next_state: StateDocument, policy: StagedPolicyRun, initialized: bool) -> Self {
        let mut event_eligibilities = policy.event_eligibilities().to_vec();
        if initialized {
            event_eligibilities.push(StagedEventEligibility::OnRun {
                cause: OnRunEventCause::Initialized,
            });
        }
        Self {
            next_state,
            event_eligibilities,
        }
    }

    fn from_eligibilities(
        next_state: StateDocument,
        event_eligibilities: Vec<StagedEventEligibility>,
    ) -> Self {
        Self {
            next_state,
            event_eligibilities,
        }
    }

    fn next_state(&self) -> &StateDocument {
        &self.next_state
    }

    fn event_eligibilities(&self) -> &[StagedEventEligibility] {
        &self.event_eligibilities
    }
}

fn finish_source_suspect(
    run: StatefulRun<'_>,
    state: &StateDocument,
    outcome: RunOutcome,
    reason: SourceSuspectReason,
    detail: ProcessErrorDetail,
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
    )
    .expect("source-suspect policy staging must preserve state-owned event eligibility");
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
                error: Some(detail_from_error(
                    &failure.error,
                    Some(run.paths.state_file()),
                )),
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
        persisted: commit.persisted,
        delivery: commit.delivery,
    })
}

fn finish_permanent_error(
    run: StatefulRun<'_>,
    state: &StateDocument,
    error_code: PermanentErrorCode,
    detail: ProcessErrorDetail,
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
    )
    .expect("permanent-error policy staging must preserve state-owned event eligibility");
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
                error: Some(detail_from_error(
                    &failure.error,
                    Some(run.paths.state_file()),
                )),
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
        persisted: commit.persisted,
        delivery: commit.delivery,
    })
}

fn ensure_failure_event_staging(
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

fn valid_condition_evaluations(
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
fn stage_valid_observation_run(
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

struct CommitResult {
    persisted: bool,
    delivery: DeliveryEvidence,
}

struct CommitFailure {
    error: CoreError,
    persisted: bool,
    delivery: DeliveryEvidence,
}

/// Commits the complete staged T0 plan and only then drains durable records.
///
/// The first state write contains both M3 state and all materialized immutable outbox records.
/// Delivery starts strictly after that crash-durable write succeeds and every delivery-state update
/// remains under the same target lock.
fn commit_staged_run(
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
fn commit_staged_run_with_clock<F>(
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
fn commit_staged_run_with_clock_and_persist<F, P>(
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
        outbox_error: None,
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
            delivery.outbox_error = Some(error.to_string());
            Err(Box::new(CommitFailure {
                error,
                persisted: true,
                delivery,
            }))
        }
    }
}

fn previous_value(state: &StateDocument) -> Option<String> {
    state
        .accepted_observation()
        .map(|observation| observation.canonical_value().to_owned())
}

/// Runs the supplied targets in bounded parallel groups.
pub(crate) fn run_batch(
    paths: Vec<TargetPaths>,
    mode: RunMode,
    jobs: usize,
) -> Result<BatchRunReport, CoreError> {
    if jobs == 0 {
        return Err(CoreError::contract("batch jobs must be positive"));
    }
    let requested_targets = paths
        .iter()
        .map(|path| path.target_id().to_owned())
        .collect::<Vec<_>>();
    let mut reports = Vec::with_capacity(paths.len());
    for group in paths.chunks(jobs) {
        let handles = group
            .iter()
            .cloned()
            .map(|path| thread::spawn(move || run_once_with_mode(&path, mode)))
            .collect::<Vec<_>>();
        for handle in handles {
            reports.push(
                handle
                    .join()
                    .map_err(|_| CoreError::internal("target-run worker panicked"))??,
            );
        }
    }
    Ok(BatchRunReport::new(mode, requested_targets, reports))
}

/// Returns one stable target status.
pub(crate) fn status(paths: &TargetPaths) -> Result<StatusReport, CoreError> {
    paths.require_watch_root_directory()?;
    let target = match load_target(paths) {
        Ok(target) => target,
        Err(error) => {
            return Ok(StatusReport::new(
                paths.target_id(),
                target_load_status_kind(&error),
                None,
                None,
                None,
                None,
                Some(detail_from_error(&error, Some(paths.target_file()))),
            ));
        }
    };
    if let Err(error) = target.validate() {
        return Ok(StatusReport::new(
            paths.target_id(),
            target_load_status_kind(&error),
            None,
            None,
            None,
            None,
            Some(detail_from_error(&error, Some(paths.target_file()))),
        ));
    }
    let _lock = lock_shared(paths)?;
    let digest = target.contract_digest_sha256()?;
    match load_state(paths) {
        Ok(None) => Ok(StatusReport::new(
            paths.target_id(),
            StatusKind::Pending,
            Some(target.display_name().to_owned()),
            Some(target.enabled()),
            Some(digest),
            None,
            None,
        )),
        Ok(Some(state)) if state.contract_digest_sha256() == digest => {
            if let Err(error) = state.validate_for_target(&target) {
                return Ok(StatusReport::new(
                    paths.target_id(),
                    StatusKind::InvalidState,
                    Some(target.display_name().to_owned()),
                    Some(target.enabled()),
                    Some(digest),
                    None,
                    Some(detail_from_error(&error, Some(paths.state_file()))),
                ));
            }
            Ok(StatusReport::new(
                paths.target_id(),
                if state.accepted_observation().is_some() {
                    StatusKind::Ready
                } else {
                    StatusKind::Pending
                },
                Some(target.display_name().to_owned()),
                Some(target.enabled()),
                Some(digest),
                state.accepted_observation().cloned(),
                None,
            ))
        }
        Ok(Some(_)) => Ok(StatusReport::new(
            paths.target_id(),
            StatusKind::InvalidState,
            Some(target.display_name().to_owned()),
            Some(target.enabled()),
            Some(digest),
            None,
            Some(ProcessErrorDetail::new(
                ProcessErrorKind::Contract,
                "stored state belongs to a different measurement contract; reset is required",
                Some(paths.state_file().display().to_string()),
            )),
        )),
        Err(error) => Ok(StatusReport::new(
            paths.target_id(),
            StatusKind::InvalidState,
            Some(target.display_name().to_owned()),
            Some(target.enabled()),
            Some(digest),
            None,
            Some(detail_from_error(&error, Some(paths.state_file()))),
        )),
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    fn target() -> TargetDocument {
        let target: TargetDocument = toml::from_str(
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 1\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = \"/tmp/source.json\"\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
        )
        .expect("target TOML");
        target.validate().expect("valid target");
        target
    }

    fn paths() -> (tempfile::TempDir, TargetPaths) {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let paths = TargetPaths::try_new(temporary.path(), "demo").expect("target paths");
        (temporary, paths)
    }

    fn empty_staged(target: &TargetDocument) -> StagedRun {
        StagedRun::from_eligibilities(
            StateDocument::new(
                crate::TargetId::new("demo").expect("target id"),
                target.contract_digest_sha256().expect("digest"),
            ),
            Vec::new(),
        )
    }

    fn invalid_fetch_target() -> TargetDocument {
        let mut value = serde_json::to_value(target()).expect("target JSON");
        value["fetch"]["max_bytes"] = serde_json::json!(0);
        serde_json::from_value(value).expect("structurally valid target")
    }

    fn state_for(target: &TargetDocument) -> StateDocument {
        StateDocument::new(
            crate::TargetId::new("demo").expect("target id"),
            target.contract_digest_sha256().expect("digest"),
        )
    }

    #[test]
    fn internal_staging_guards_reject_mismatched_policy_shapes() {
        let staged = StagedPolicyRun::SourceSuspect {
            reason_class: SourceSuspectReason::FetchFailed,
            event_eligibilities: Vec::new(),
        };
        assert!(valid_condition_evaluations(&staged).is_err());
        let state = StateDocument::new(
            crate::TargetId::new("demo").expect("target id"),
            "a".repeat(64),
        );
        let mismatched = StagedRun::from_eligibilities(
            state,
            vec![StagedEventEligibility::OnRun {
                cause: OnRunEventCause::Reset,
            }],
        );
        assert!(ensure_failure_event_staging(&mismatched, None).is_err());
        assert!(ensure_failure_event_staging(&mismatched, Some(OnRunEventCause::Reset)).is_ok());
        assert!(
            ensure_failure_event_staging(&mismatched, Some(OnRunEventCause::Initialized)).is_err()
        );
    }

    #[test]
    fn staging_boundaries_surface_invalid_inputs_before_any_commit() {
        let target = target();
        let observation = target
            .parse_json_scalar_token("10".to_owned())
            .expect("observation");
        assert!(
            stage_valid_observation_run(
                &invalid_fetch_target(),
                &state_for(&target),
                observation,
                "2026-07-15T00:00:00Z",
            )
            .is_err()
        );

        let mut state = state_for(&target);
        let initial = target
            .parse_json_scalar_token("10".to_owned())
            .expect("initial observation");
        state
            .apply_valid_observation(&target, initial, &[], "2026-07-15T00:00:00Z")
            .expect("initial state");
        let mut wire = serde_json::to_value(&state).expect("state JSON");
        wire["observation_seq"] = serde_json::json!(u64::MAX);
        let exhausted: StateDocument =
            serde_json::from_value(wire).expect("structurally valid state");
        let observation = target
            .parse_json_scalar_token("11".to_owned())
            .expect("next observation");
        assert!(
            stage_valid_observation_run(&target, &exhausted, observation, "2026-07-15T00:00:01Z",)
                .is_err()
        );
    }

    #[test]
    fn failure_staging_surfaces_state_overflow_and_invalid_target_contracts() {
        let (temporary, paths) = paths();
        let target = target();
        let mut wire = serde_json::to_value(state_for(&target)).expect("state JSON");
        wire["source_health"] = serde_json::json!({
            "state": "suspect",
            "reason_class": "fetch_failed",
            "consecutive_unresolved": u32::MAX,
            "first_unresolved_at": "2026-07-15T00:00:00Z",
            "last_details": {"kind": "io", "message": "prior failure"},
        });
        let overflowed: StateDocument =
            serde_json::from_value(wire).expect("structurally valid source-health state");
        let run = StatefulRun {
            paths: &paths,
            target: &target,
            mode: RunMode::Live,
            started: "2026-07-15T00:00:01Z".to_owned(),
            digest: target.contract_digest_sha256().expect("digest"),
            previous: None,
        };
        assert!(
            finish_source_suspect(
                run,
                &overflowed,
                RunOutcome::FetchFailed,
                SourceSuspectReason::FetchFailed,
                ProcessErrorDetail::new(ProcessErrorKind::Io, "current failure", None),
            )
            .is_err()
        );

        let invalid = invalid_fetch_target();
        let state = state_for(&invalid);
        let run = StatefulRun {
            paths: &paths,
            target: &invalid,
            mode: RunMode::Live,
            started: "2026-07-15T00:00:00Z".to_owned(),
            digest: invalid.contract_digest_sha256().expect("digest"),
            previous: None,
        };
        assert!(
            finish_source_suspect(
                run,
                &state,
                RunOutcome::FetchFailed,
                SourceSuspectReason::FetchFailed,
                ProcessErrorDetail::new(ProcessErrorKind::Io, "source failed", None),
            )
            .is_err()
        );

        let state = state_for(&invalid);
        let run = StatefulRun {
            paths: &paths,
            target: &invalid,
            mode: RunMode::Live,
            started: "2026-07-15T00:00:00Z".to_owned(),
            digest: invalid.contract_digest_sha256().expect("digest"),
            previous: None,
        };
        assert!(
            finish_permanent_error(
                run,
                &state,
                PermanentErrorCode::InvalidJsonPointer,
                ProcessErrorDetail::new(ProcessErrorKind::Contract, "invalid JSON pointer", None,),
            )
            .is_err()
        );
        drop(temporary);
    }

    #[test]
    fn precommit_failures_preserve_the_nonpersistent_commit_boundary() {
        let target = target();
        let (_temporary, paths) = paths();

        let staged = empty_staged(&target);
        let mut unavailable_clock = || Err(CoreError::internal("clock unavailable"));
        let failure = commit_staged_run_with_clock(
            &paths,
            &target,
            RunMode::Live,
            &staged,
            target.contract_digest_sha256().expect("digest").as_str(),
            &mut unavailable_clock,
        )
        .err()
        .expect("clock failure must prevent a commit");
        assert!(!failure.persisted);
        assert!(failure.error.to_string().contains("clock unavailable"));

        let staged = StagedRun::from_eligibilities(
            StateDocument::new(
                crate::TargetId::new("demo").expect("target id"),
                target.contract_digest_sha256().expect("digest"),
            ),
            vec![StagedEventEligibility::OnCondition {
                condition_id: "missing".parse().expect("condition id"),
            }],
        );
        let mut clock = || Ok("2026-07-15T00:00:00Z".to_owned());
        let failure = commit_staged_run_with_clock(
            &paths,
            &target,
            RunMode::Live,
            &staged,
            target.contract_digest_sha256().expect("digest").as_str(),
            &mut clock,
        )
        .err()
        .expect("materialization failure must prevent a commit");
        assert!(!failure.persisted);
        assert!(failure.error.to_string().contains("condition absent"));

        let staged = empty_staged(&target);
        let mut invalid_clock = || Ok("not-a-timestamp".to_owned());
        let failure = commit_staged_run_with_clock(
            &paths,
            &target,
            RunMode::Live,
            &staged,
            target.contract_digest_sha256().expect("digest").as_str(),
            &mut invalid_clock,
        )
        .err()
        .expect("outbox enqueue failure must prevent a commit");
        assert!(!failure.persisted);
        assert!(failure.error.to_string().contains("outbox timestamp"));
    }

    #[test]
    fn durable_commit_failure_cannot_enter_delivery() {
        let target = target();
        let (_temporary, paths) = paths();
        let staged = StagedRun::from_eligibilities(
            StateDocument::new(
                crate::TargetId::new("demo").expect("target id"),
                target.contract_digest_sha256().expect("digest"),
            ),
            vec![StagedEventEligibility::OnRun {
                cause: OnRunEventCause::Reset,
            }],
        );
        let mut clock = || Ok("2026-07-15T00:00:00Z".to_owned());
        let mut failed_persist =
            |_state: &StateDocument| Err(CoreError::internal("durability synchronization refused"));

        let failure = commit_staged_run_with_clock_and_persist(
            &paths,
            &target,
            RunMode::Live,
            &staged,
            target.contract_digest_sha256().expect("digest").as_str(),
            &mut clock,
            &mut failed_persist,
        )
        .err()
        .expect("durability failure must prevent delivery");

        assert!(!failure.persisted);
        assert!(
            failure
                .error
                .to_string()
                .contains("durability synchronization refused")
        );
        assert!(!paths.state_file().exists());
    }
}

//! Generation-atomic application of one source- or measurement-owned delivery result.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::CoreError;

use super::{
    CommitCause, CommitManifest, CommitManifestParts, CommitOperation, DeadLetter,
    DeliveryExecution, DeliveryRecord, ManifestPath, MeasurementId, SourceState, TrustedSourceDir,
    TrustedStorageDir,
};

/// Outbox ownership determines the only pending and dead-letter paths a delivery result may alter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutboxOwner {
    /// Source lifecycle and acquisition-health event outbox.
    Source,
    /// One measurement's condition/lifecycle/extraction outbox.
    Measurement(MeasurementId),
}

/// Atomically commits one delivery outcome and the source state's successor generation.
pub fn commit_delivery_result(
    source_dir: &TrustedSourceDir,
    storage: &TrustedStorageDir,
    source_state: &SourceState,
    owner: &OutboxOwner,
    prior: &DeliveryRecord,
    execution: DeliveryExecution,
) -> Result<SourceState, CoreError> {
    prior.validate()?;
    validate_owner(source_dir, owner, prior)?;
    if prior.envelope().source_instance_id() != source_state.source_instance_id() {
        return Err(CoreError::contract(
            "delivery record source lineage differs from the committed source state",
        ));
    }
    let next = source_state.next_generation()?;
    let source_stage = stage("source-state")?;
    let source_hash = storage.stage_json(&source_stage, &next)?;
    let mut entries = vec![CommitOperation::Install {
        final_path: ManifestPath::new("source-state.json")?,
        staged_path: source_stage,
        expected_prior_sha256: storage.source_state_sha256()?,
        result_sha256: source_hash,
    }];
    let pending = pending_path(owner, prior)?;
    let pending_hash = storage
        .existing_file_sha256(&pending)?
        .ok_or_else(|| CoreError::contract("due delivery record is absent from its outbox"))?;
    match execution {
        DeliveryExecution::Delivered => entries.push(CommitOperation::Remove {
            final_path: pending,
            expected_prior_sha256: pending_hash,
        }),
        DeliveryExecution::Retry(record) => {
            if !record.is_single_attempt_successor_of(prior) {
                return Err(CoreError::contract(
                    "delivery retry changed immutable snapshot facts or attempt history",
                ));
            }
            let staged = stage("delivery-retry")?;
            let result_hash = storage.stage_json(&staged, &record)?;
            entries.push(CommitOperation::Install {
                final_path: pending,
                staged_path: staged,
                expected_prior_sha256: Some(pending_hash),
                result_sha256: result_hash,
            });
        }
        DeliveryExecution::DeadLetter(letter) => {
            validate_dead_letter_identity(&letter, prior)?;
            let staged = stage("dead-letter")?;
            let result_hash = storage.stage_json(&staged, &letter)?;
            entries.push(CommitOperation::Install {
                final_path: dead_letter_path(owner, prior)?,
                staged_path: staged,
                expected_prior_sha256: None,
                result_sha256: result_hash,
            });
            entries.push(CommitOperation::Remove {
                final_path: pending,
                expected_prior_sha256: pending_hash,
            });
        }
    }
    let touched = match owner {
        OutboxOwner::Source => BTreeMap::new(),
        OutboxOwner::Measurement(id) => {
            let identity = source_dir
                .read_identity()?
                .ok_or_else(|| CoreError::contract("delivery commit requires source identity"))?;
            let instance = identity.measurements().get(id).ok_or_else(|| {
                CoreError::contract("measurement delivery record has no identity authority entry")
            })?;
            BTreeMap::from([(id.clone(), instance.measurement_instance_id().clone())])
        }
    };
    let manifest = CommitManifest::new(CommitManifestParts {
        source_id: source_dir.paths().source_id().clone(),
        source_instance_id: next.source_instance_id().clone(),
        generation: next.generation(),
        touched_measurement_instances: touched,
        identity_measurement_additions: BTreeMap::new(),
        cause: CommitCause::DeliveryResult,
        entries,
    })?;
    source_dir.apply_normal_commit(&manifest)?;
    Ok(next)
}

fn validate_owner(
    source_dir: &TrustedSourceDir,
    owner: &OutboxOwner,
    record: &DeliveryRecord,
) -> Result<(), CoreError> {
    if record.envelope().source_id() != source_dir.paths().source_id() {
        return Err(CoreError::contract(
            "delivery record source_id differs from its source directory",
        ));
    }
    match (owner, record.envelope().measurement_lineage()) {
        (OutboxOwner::Source, None) => Ok(()),
        (OutboxOwner::Measurement(expected), Some((observed, _))) if expected == observed => Ok(()),
        (OutboxOwner::Source | OutboxOwner::Measurement(_), _) => Err(CoreError::contract(
            "delivery record emitter scope differs from its outbox owner",
        )),
    }
}

fn pending_path(owner: &OutboxOwner, record: &DeliveryRecord) -> Result<ManifestPath, CoreError> {
    let file = record.storage_file_name();
    match owner {
        OutboxOwner::Source => ManifestPath::new(format!("source-outbox/{file}")),
        OutboxOwner::Measurement(id) => {
            ManifestPath::new(format!("measurements/{}/outbox/{file}", id.as_str()))
        }
    }
}

fn dead_letter_path(
    owner: &OutboxOwner,
    record: &DeliveryRecord,
) -> Result<ManifestPath, CoreError> {
    let file = record.storage_file_name();
    match owner {
        OutboxOwner::Source => ManifestPath::new(format!("dead-letters/{file}")),
        OutboxOwner::Measurement(id) => {
            ManifestPath::new(format!("measurements/{}/dead-letters/{file}", id.as_str()))
        }
    }
}

fn validate_dead_letter_identity(
    letter: &DeadLetter,
    prior: &DeliveryRecord,
) -> Result<(), CoreError> {
    letter.validate()?;
    if letter.record().is_single_attempt_successor_of(prior) {
        Ok(())
    } else {
        Err(CoreError::contract(
            "dead letter changed immutable record identity",
        ))
    }
}

fn stage(label: &str) -> Result<ManifestPath, CoreError> {
    ManifestPath::new(format!("staged/{label}-{}.json", Uuid::new_v4()))
}

#[cfg(test)]
#[path = "delivery_commit/tests.rs"]
mod tests;

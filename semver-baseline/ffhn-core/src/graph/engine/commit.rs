//! Acquisition-generation manifest construction for source and measurement state plus new outbox records.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::CoreError;

use super::super::{
    CommitCause, CommitManifest, CommitManifestParts, CommitOperation, DeliveryRecord,
    ManifestPath, MeasurementId, MeasurementInstanceId, MeasurementState, SourceState,
    TrustedSourceDir, TrustedStorageDir,
};

pub(super) fn commit_acquisition(
    source_dir: &TrustedSourceDir,
    storage: &TrustedStorageDir,
    source_state: &SourceState,
    measurements: Vec<(MeasurementId, MeasurementState)>,
    additions: BTreeMap<MeasurementId, MeasurementInstanceId>,
    source_records: Vec<DeliveryRecord>,
    records: Vec<(MeasurementId, DeliveryRecord)>,
) -> Result<(), CoreError> {
    let source_stage = stage("source-state")?;
    let source_hash = storage.stage_json(&source_stage, source_state)?;
    let mut entries = vec![CommitOperation::Install {
        final_path: ManifestPath::new("source-state.json")?,
        staged_path: source_stage,
        expected_prior_sha256: storage.source_state_sha256()?,
        result_sha256: source_hash,
    }];
    let mut touched = BTreeMap::new();
    for (id, state) in measurements {
        let staged = stage(&format!("measurement-{}", id.as_str()))?;
        let hash = storage.stage_json(&staged, &state)?;
        if !additions.contains_key(&id) {
            touched.insert(id.clone(), state.measurement_instance_id().clone());
        }
        entries.push(CommitOperation::Install {
            final_path: ManifestPath::new(format!("measurements/{}/state.json", id.as_str()))?,
            staged_path: staged,
            expected_prior_sha256: storage.measurement_state_sha256(&id)?,
            result_sha256: hash,
        });
    }
    for (measurement_id, record) in records {
        let staged = stage("measurement-outbox")?;
        let hash = storage.stage_json(&staged, &record)?;
        entries.push(CommitOperation::Install {
            final_path: ManifestPath::new(format!(
                "measurements/{}/outbox/{}",
                measurement_id.as_str(),
                record.storage_file_name()
            ))?,
            staged_path: staged,
            expected_prior_sha256: None,
            result_sha256: hash,
        });
    }
    for record in source_records {
        let staged = stage("source-outbox")?;
        let hash = storage.stage_json(&staged, &record)?;
        entries.push(CommitOperation::Install {
            final_path: ManifestPath::new(format!("source-outbox/{}", record.storage_file_name()))?,
            staged_path: staged,
            expected_prior_sha256: None,
            result_sha256: hash,
        });
    }
    let manifest = CommitManifest::new(CommitManifestParts {
        source_id: source_dir.paths().source_id().clone(),
        source_instance_id: source_state.source_instance_id().clone(),
        generation: source_state.generation(),
        touched_measurement_instances: touched,
        identity_measurement_additions: additions,
        cause: CommitCause::Acquisition,
        entries,
    })?;
    source_dir.apply_normal_commit(&manifest)
}

fn stage(label: &str) -> Result<ManifestPath, CoreError> {
    ManifestPath::new(format!("staged/{label}-{}.json", Uuid::new_v4()))
}

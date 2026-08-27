//! Manifest-driven source-lineage transition orchestration.

use crate::CoreError;

use super::{
    LineageManifest, LineageRecovery, LineageScope, MeasurementId, MeasurementState,
    SourceIdentity, SourceState, TrustedSourceDir,
    storage::{
        open_or_create_real_child, optional_fs_entry, remove_regular_file, remove_tombstone,
    },
};

impl TrustedSourceDir {
    /// Durably starts and completes one source-scope lineage transition.
    pub fn apply_source_transition(&self, manifest: &LineageManifest) -> Result<(), CoreError> {
        self.require_source_scope(manifest)?;
        if matches!(manifest.scope(), LineageScope::SourceReset) && manifest.from().is_none() {
            return Err(CoreError::contract(
                "a source reset without readable prior authority must use blind source reset",
            ));
        }
        self.write_lineage_manifest(manifest)?;
        self.recover_lineage_transition()
    }

    /// Performs the source-reset recovery path that never reads old authority or storage facts.
    ///
    /// Normal turn entry treats the resulting manifest as unresolvable until the fresh authority
    /// is durable; this method is the only writer allowed to bridge that intentional boundary.
    pub fn apply_blind_source_transition(&self, scope: LineageScope) -> Result<(), CoreError> {
        if !matches!(scope, LineageScope::Init | LineageScope::SourceReset) {
            return Err(CoreError::contract(
                "blind source transition requires init or source_reset scope",
            ));
        }
        let target = SourceIdentity::fresh();
        let manifest = LineageManifest::new(super::LineageManifestParts {
            source_id: self.paths.source_id().clone(),
            scope,
            from: None,
            target: target.clone(),
        })?;
        self.write_lineage_manifest(&manifest)?;
        self.write_identity(&target)?;
        self.swap_source_storage(&target)?;
        remove_regular_file(
            &self.dir,
            ".ffhn-lineage.manifest",
            &self.paths.lineage_manifest_file(),
            "lineage manifest",
        )
    }

    /// Durably starts and completes one measurement-scope lineage transition.
    pub fn apply_measurement_transition(
        &self,
        manifest: &LineageManifest,
    ) -> Result<(), CoreError> {
        self.require_measurement_scope(manifest)?;
        self.write_lineage_manifest(manifest)?;
        self.recover_lineage_transition()
    }

    /// Completes one pending source-scope lineage transition after a crash or interrupted reset.
    pub fn recover_lineage_transition(&self) -> Result<(), CoreError> {
        let Some(manifest) = self.read_lineage_manifest()? else {
            return Ok(());
        };
        self.apply_manifest_authority(&manifest)?;
        match manifest.scope() {
            LineageScope::Init | LineageScope::SourceReset => {
                self.require_source_scope(&manifest)?;
                self.swap_source_storage(manifest.target())?;
            }
            LineageScope::MeasurementReset { measurement_id } => {
                self.require_measurement_scope(&manifest)?;
                self.swap_measurement_storage(manifest.target(), measurement_id)?;
            }
        }
        remove_regular_file(
            &self.dir,
            ".ffhn-lineage.manifest",
            &self.paths.lineage_manifest_file(),
            "lineage manifest",
        )
    }

    fn apply_manifest_authority(&self, manifest: &LineageManifest) -> Result<(), CoreError> {
        match manifest.recover_against(self.read_identity()?.as_ref()) {
            LineageRecovery::ApplyTargetIdentity => self.write_identity(manifest.target()),
            LineageRecovery::ContinueScopeSwap => Ok(()),
            LineageRecovery::Unresolvable => Err(CoreError::contract(
                "lineage manifest does not match the observed source identity",
            )),
        }
    }

    fn require_source_scope(&self, manifest: &LineageManifest) -> Result<(), CoreError> {
        if manifest.source_id() != self.paths.source_id() {
            return Err(CoreError::contract(
                "lineage manifest source_id does not match the source directory",
            ));
        }
        if !matches!(
            manifest.scope(),
            LineageScope::Init | LineageScope::SourceReset
        ) {
            return Err(CoreError::contract(
                "source transition requires init or source_reset lineage scope",
            ));
        }
        Ok(())
    }

    fn require_measurement_scope(&self, manifest: &LineageManifest) -> Result<(), CoreError> {
        if manifest.source_id() != self.paths.source_id() {
            return Err(CoreError::contract(
                "lineage manifest source_id does not match the source directory",
            ));
        }
        if !matches!(manifest.scope(), LineageScope::MeasurementReset { .. }) {
            return Err(CoreError::contract(
                "measurement transition requires measurement_reset lineage scope",
            ));
        }
        Ok(())
    }

    fn swap_source_storage(&self, target: &SourceIdentity) -> Result<(), CoreError> {
        remove_tombstone(&self.dir, &self.paths.tombstone_dir())?;
        if optional_fs_entry(
            self.dir.symlink_metadata(".ffhn"),
            &self.paths.storage_dir(),
        )?
        .is_some()
        {
            map_transition_rename(
                self.dir.rename(".ffhn", &self.dir, ".ffhn-tombstone"),
                &self.paths.storage_dir(),
            )?;
        }
        let storage = self.create_storage()?;
        storage.write_source_state(&SourceState::fresh(target.source_instance_id().clone()))?;
        remove_tombstone(&self.dir, &self.paths.tombstone_dir())
    }

    fn swap_measurement_storage(
        &self,
        target: &SourceIdentity,
        measurement_id: &MeasurementId,
    ) -> Result<(), CoreError> {
        let measurement_identity = target.measurements().get(measurement_id).ok_or_else(|| {
            CoreError::contract("measurement reset target lacks the measurement identity")
        })?;
        let storage = self.open_storage()?;
        let measurements = open_or_create_real_child(
            &storage.dir,
            "measurements",
            &storage.paths.storage_dir().join("measurements"),
            "measurement storage root",
        )?;
        remove_tombstone(&self.dir, &self.paths.tombstone_dir())?;
        if optional_fs_entry(
            measurements.symlink_metadata(measurement_id.as_str()),
            &self.paths.measurement_storage_dir(measurement_id),
        )?
        .is_some()
        {
            map_transition_rename(
                measurements.rename(measurement_id.as_str(), &self.dir, ".ffhn-tombstone"),
                &self.paths.measurement_storage_dir(measurement_id),
            )?;
        }
        storage.write_measurement_state(
            measurement_id,
            &MeasurementState::fresh(
                target.source_instance_id().clone(),
                measurement_identity.measurement_instance_id().clone(),
            ),
        )?;
        remove_tombstone(&self.dir, &self.paths.tombstone_dir())
    }
}

fn map_transition_rename(
    result: std::io::Result<()>,
    path: &std::path::Path,
) -> Result<(), CoreError> {
    result.map_err(|error| CoreError::io(path, error))
}

#[cfg(test)]
#[path = "transition/tests.rs"]
mod tests;

//! The v11 lineage-authority gate for normal source operations.

use std::collections::{BTreeMap, BTreeSet};

use crate::CoreError;

use super::storage::optional_fs_entry;
use super::{
    MeasurementId, MeasurementIdentity, MeasurementState, SourceIdentity, SourceState,
    TrustedSourceDir, TrustedStorageDir,
};

/// Source-scope result of applying the authoritative lineage table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceLineage {
    /// Neither authority nor storage exists, so an initialization transition may begin.
    NeedsInitialization,
    /// Authority and source state agree and measurement facts may be evaluated independently.
    Ready(Box<ReadySourceLineage>),
    /// Source artifacts cannot safely participate in any normal operation.
    Refused(SourceLineageRefusal),
}

impl SourceLineage {
    /// Returns source state admitted by the lineage gate, if the source is ready.
    pub const fn as_ready_state(&self) -> Option<&SourceState> {
        match self {
            Self::Ready(ready) => Some(ready.state()),
            Self::NeedsInitialization | Self::Refused(_) => None,
        }
    }
}

/// Source facts admitted by the source-level lineage gate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadySourceLineage {
    identity: SourceIdentity,
    state: SourceState,
}

impl ReadySourceLineage {
    /// Returns the sole source-lineage authority.
    pub const fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    /// Returns the source state proved to bear the authority's source instance.
    pub const fn state(&self) -> &SourceState {
        &self.state
    }
}

/// Closed source-level refusal reasons with the sole remedy `reset --source`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceLineageRefusal {
    /// The authority document is absent while the storage swap scope exists.
    StorageWithoutIdentity,
    /// Authority exists but the storage swap scope does not.
    StorageMissing,
    /// A trusted storage component could not be opened without following a link.
    StorageUnavailable,
    /// The source-state file is absent although authority exists.
    SourceStateMissing,
    /// The source-state document cannot be safely decoded and stamped.
    SourceStateUnreadable,
    /// The source-state source-instance stamp differs from authority.
    SourceInstanceMismatch,
    /// An unrecognized or unsafe measurement storage subtree makes the source tree ambiguous.
    MeasurementStorageUnreadable,
    /// The source identity itself cannot be decoded into its closed authority schema.
    IdentityUnreadable,
    /// A source-owned pending record or dead letter is unreadable, foreign, or misplaced.
    DeliveryArtifactUnreadable,
}

impl SourceLineageRefusal {
    /// Returns the stable report spelling for one source-scoped refusal.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StorageWithoutIdentity => "storage_without_identity",
            Self::StorageMissing => "storage_missing",
            Self::StorageUnavailable => "storage_unavailable",
            Self::SourceStateMissing => "source_state_missing",
            Self::SourceStateUnreadable => "source_state_unreadable",
            Self::SourceInstanceMismatch => "source_instance_mismatch",
            Self::MeasurementStorageUnreadable => "measurement_storage_unreadable",
            Self::IdentityUnreadable => "identity_unreadable",
            Self::DeliveryArtifactUnreadable => "delivery_artifact_unreadable",
        }
    }
}

/// Measurement-scope result of applying rows 5–7 of the lineage table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeasurementLineage {
    /// State and both lineage stamps match the authority entry.
    Ready(Box<MeasurementState>),
    /// The measurement is declared but has never created any durable artifact.
    NeverInitialized,
    /// Only this measurement is withheld; sibling measurements remain usable.
    Held(MeasurementLineageHold),
}

/// Closed measurement-level refusal reasons with the sole remedy `reset --measurement`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasurementLineageHold {
    /// An artifact exists for a measurement with no corresponding authority entry.
    MissingIdentityEntry,
    /// An authority entry exists without its atomically paired state file.
    StateMissing,
    /// A measurement artifact cannot be decoded for its lineage stamps.
    ArtifactUnreadable,
    /// The artifact's source-instance stamp differs from source authority.
    SourceInstanceMismatch,
    /// The artifact's measurement-instance stamp differs from measurement authority.
    MeasurementInstanceMismatch,
}

impl MeasurementLineageHold {
    /// Returns the stable report spelling for one measurement-scoped lineage hold.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingIdentityEntry => "missing_identity_entry",
            Self::StateMissing => "state_missing",
            Self::ArtifactUnreadable => "artifact_unreadable",
            Self::SourceInstanceMismatch => "source_instance_mismatch",
            Self::MeasurementInstanceMismatch => "measurement_instance_mismatch",
        }
    }
}

/// Complete normal-operation lineage result for one source directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineageInspection {
    source: SourceLineage,
    measurements: BTreeMap<MeasurementId, MeasurementLineage>,
}

impl LineageInspection {
    /// Returns the source-scope gate result.
    pub const fn source(&self) -> &SourceLineage {
        &self.source
    }

    /// Returns every observed, configured, or authoritative measurement gate result.
    pub fn measurements(&self) -> &BTreeMap<MeasurementId, MeasurementLineage> {
        &self.measurements
    }

    /// Returns one measurement result when it was declared, authoritative, or artifact-backed.
    pub fn measurement(&self, measurement_id: &MeasurementId) -> Option<&MeasurementLineage> {
        self.measurements.get(measurement_id)
    }
}

impl TrustedSourceDir {
    /// Applies the frozen lineage table before any normal source operation.
    ///
    /// `declared_measurements` is configuration-only input. Entries from authority and storage are
    /// always included too, so removing a measurement from TOML cannot erase or hide its lineage.
    pub fn inspect_lineage<I>(
        &self,
        declared_measurements: I,
    ) -> Result<LineageInspection, CoreError>
    where
        I: IntoIterator<Item = MeasurementId>,
    {
        let declared = declared_measurements.into_iter().collect::<BTreeSet<_>>();
        let identity = match self.read_identity() {
            Ok(identity) => identity,
            Err(_) => return Ok(refused(SourceLineageRefusal::IdentityUnreadable)),
        };
        let storage_present = self.storage_entry_present()?;
        let Some(identity) = identity else {
            return Ok(if storage_present {
                refused(SourceLineageRefusal::StorageWithoutIdentity)
            } else {
                LineageInspection {
                    source: SourceLineage::NeedsInitialization,
                    measurements: declared
                        .into_iter()
                        .map(|id| (id, MeasurementLineage::NeverInitialized))
                        .collect(),
                }
            });
        };
        if !storage_present {
            return Ok(refused(SourceLineageRefusal::StorageMissing));
        }
        let storage = match self.open_storage() {
            Ok(storage) => storage,
            Err(_) => return Ok(refused(SourceLineageRefusal::StorageUnavailable)),
        };
        let state = match storage.read_source_state() {
            Ok(Some(state)) => state,
            Ok(None) => return Ok(refused(SourceLineageRefusal::SourceStateMissing)),
            Err(_) => return Ok(refused(SourceLineageRefusal::SourceStateUnreadable)),
        };
        if state.source_instance_id() != identity.source_instance_id() {
            return Ok(refused(SourceLineageRefusal::SourceInstanceMismatch));
        }
        if !source_delivery_artifacts_match(&storage, self.paths.source_id(), &identity) {
            return Ok(refused(SourceLineageRefusal::DeliveryArtifactUnreadable));
        }
        let artifact_ids = match storage.measurement_storage_ids() {
            Ok(ids) => ids,
            Err(_) => return Ok(refused(SourceLineageRefusal::MeasurementStorageUnreadable)),
        };
        let ids = declared
            .into_iter()
            .chain(identity.measurements().keys().cloned())
            .chain(artifact_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        let measurements = ids
            .into_iter()
            .map(|id| {
                let artifact_exists = artifact_ids.contains(&id);
                let entry = identity.measurements().get(&id);
                let result = match storage.read_measurement_state(&id) {
                    Ok(Some(state)) => match entry {
                        None => {
                            MeasurementLineage::Held(MeasurementLineageHold::MissingIdentityEntry)
                        }
                        Some(_) if state.source_instance_id() != identity.source_instance_id() => {
                            MeasurementLineage::Held(MeasurementLineageHold::SourceInstanceMismatch)
                        }
                        Some(entry)
                            if state.measurement_instance_id()
                                != entry.measurement_instance_id() =>
                        {
                            MeasurementLineage::Held(
                                MeasurementLineageHold::MeasurementInstanceMismatch,
                            )
                        }
                        Some(entry) => match measurement_delivery_artifacts_match(
                            &storage,
                            &id,
                            self.paths.source_id(),
                            &identity,
                            entry,
                        ) {
                            Ok(()) => MeasurementLineage::Ready(Box::new(state)),
                            Err(hold) => MeasurementLineage::Held(hold),
                        },
                    },
                    Ok(None) if entry.is_some() => {
                        MeasurementLineage::Held(MeasurementLineageHold::StateMissing)
                    }
                    Ok(None) if artifact_exists => {
                        MeasurementLineage::Held(MeasurementLineageHold::ArtifactUnreadable)
                    }
                    Ok(None) => MeasurementLineage::NeverInitialized,
                    Err(_) => MeasurementLineage::Held(MeasurementLineageHold::ArtifactUnreadable),
                };
                (id, result)
            })
            .collect();
        Ok(LineageInspection {
            source: SourceLineage::Ready(Box::new(ReadySourceLineage { identity, state })),
            measurements,
        })
    }

    fn storage_entry_present(&self) -> Result<bool, CoreError> {
        optional_fs_entry(
            self.dir.symlink_metadata(".ffhn"),
            &self.paths.storage_dir(),
        )
        .map(|entry| entry.is_some())
    }
}

fn source_delivery_artifacts_match(
    storage: &TrustedStorageDir,
    source_id: &super::SourceId,
    identity: &SourceIdentity,
) -> bool {
    let records = match storage.read_source_delivery_records() {
        Ok(records) => records,
        Err(_) => return false,
    };
    let letters = match storage.read_source_dead_letters() {
        Ok(letters) => letters,
        Err(_) => return false,
    };
    records
        .iter()
        .chain(letters.iter().map(|letter| letter.record()))
        .all(|record| {
            record.envelope().source_id() == source_id
                && record.envelope().source_instance_id() == identity.source_instance_id()
                && record.envelope().measurement_lineage().is_none()
        })
}

fn measurement_delivery_artifacts_match(
    storage: &TrustedStorageDir,
    measurement_id: &MeasurementId,
    source_id: &super::SourceId,
    identity: &SourceIdentity,
    measurement_identity: &MeasurementIdentity,
) -> Result<(), MeasurementLineageHold> {
    let records = storage
        .read_measurement_delivery_records(measurement_id)
        .map_err(|_| MeasurementLineageHold::ArtifactUnreadable)?;
    let letters = storage
        .read_measurement_dead_letters(measurement_id)
        .map_err(|_| MeasurementLineageHold::ArtifactUnreadable)?;
    for record in records
        .iter()
        .chain(letters.iter().map(|letter| letter.record()))
    {
        if record.envelope().source_id() != source_id {
            return Err(MeasurementLineageHold::ArtifactUnreadable);
        }
        if record.envelope().source_instance_id() != identity.source_instance_id() {
            return Err(MeasurementLineageHold::SourceInstanceMismatch);
        }
        match record.envelope().measurement_lineage() {
            Some((event_measurement_id, event_instance))
                if event_measurement_id == measurement_id
                    && event_instance == measurement_identity.measurement_instance_id() => {}
            Some((event_measurement_id, _)) if event_measurement_id == measurement_id => {
                return Err(MeasurementLineageHold::MeasurementInstanceMismatch);
            }
            Some(_) | None => return Err(MeasurementLineageHold::ArtifactUnreadable),
        }
    }
    Ok(())
}

fn refused(reason: SourceLineageRefusal) -> LineageInspection {
    LineageInspection {
        source: SourceLineage::Refused(reason),
        measurements: BTreeMap::new(),
    }
}

impl TrustedStorageDir {
    fn measurement_storage_ids(&self) -> Result<BTreeSet<MeasurementId>, CoreError> {
        let Some(measurements) = self.open_measurement_storage_root()? else {
            return Ok(BTreeSet::new());
        };
        measurements
            .read_dir(".")
            .map_err(|error| CoreError::io(self.paths.storage_dir().join("measurements"), error))?
            .map(|entry| {
                let entry =
                    measurement_dir_entry(entry, &self.paths.storage_dir().join("measurements"))?;
                let name = measurement_storage_name(entry.file_name())?;
                let path = self.paths.storage_dir().join("measurements").join(&name);
                let file_type = measurement_entry_file_type(entry.file_type(), &path)?;
                validate_measurement_entry_type(file_type.is_symlink(), file_type.is_dir())?;
                MeasurementId::new(name)
            })
            .collect()
    }

    fn open_measurement_storage_root(&self) -> Result<Option<cap_std::fs::Dir>, CoreError> {
        super::storage::open_optional_real_child(
            &self.dir,
            "measurements",
            &self.paths.storage_dir().join("measurements"),
            "measurement storage root",
        )
    }
}

fn measurement_storage_name(name: std::ffi::OsString) -> Result<String, CoreError> {
    name.into_string()
        .map_err(|_| CoreError::contract("measurement storage directory name must be UTF-8"))
}

fn measurement_dir_entry<T>(
    result: std::io::Result<T>,
    path: &std::path::Path,
) -> Result<T, CoreError> {
    result.map_err(|error| CoreError::io(path, error))
}

fn measurement_entry_file_type<T>(
    result: std::io::Result<T>,
    path: &std::path::Path,
) -> Result<T, CoreError> {
    result.map_err(|error| CoreError::io(path, error))
}

fn validate_measurement_entry_type(is_symlink: bool, is_dir: bool) -> Result<(), CoreError> {
    if is_symlink || !is_dir {
        Err(CoreError::contract(
            "measurement storage entry must be a non-symlink directory",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "lineage_gate/tests.rs"]
mod tests;

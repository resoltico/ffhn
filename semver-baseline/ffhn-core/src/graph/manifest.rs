//! Typed v11 normal-commit manifest contract.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::CoreError;

use super::{MeasurementId, MeasurementInstanceId, SourceId, SourceInstanceId};

#[path = "manifest/operation_validation.rs"]
mod operation_validation;
#[path = "manifest/path_validation.rs"]
mod path_validation;

/// Canonical normal-commit manifest schema name.
pub const COMMIT_MANIFEST_SCHEMA_NAME: &str = "ffhn.commit_manifest";
/// Canonical normal-commit manifest schema version.
pub const COMMIT_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Validated relative storage path carried by a normal-commit manifest.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ManifestPath(String);

impl ManifestPath {
    /// Validates a nonempty, relative, slash-delimited storage path with no dot segments.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        if value.is_empty()
            || value.starts_with('/')
            || value.contains('\\')
            || value
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(CoreError::contract(
                "commit manifest paths must be nonempty relative slash-delimited paths without dot segments",
            ));
        }
        Ok(Self(value))
    }

    /// Returns the validated relative path text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn is_staged(&self) -> bool {
        self.0.starts_with("staged/")
    }
}

impl<'de> Deserialize<'de> for ManifestPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Cause of a source generation commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitCause {
    /// An acquisition cycle changed durable source or measurement state.
    Acquisition,
    /// A delivery attempt changed durable outbox state.
    DeliveryResult,
}

/// Result of applying the normal-commit manifest recovery gate before filesystem operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitRecovery {
    /// The manifest belongs to the observed authority and can be replayed idempotently.
    Apply,
    /// The manifest must be treated as corrupt or foreign and never replayed.
    Unresolvable,
}

/// One idempotent file operation guarded by content hashes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommitOperation {
    /// Installs one fully staged replacement, optionally requiring an existing prior hash.
    Install {
        /// Destination relative to the storage root.
        final_path: ManifestPath,
        /// Already-synced staged path relative to the storage root.
        staged_path: ManifestPath,
        /// Required hash of an existing destination, when one must exist.
        #[serde(skip_serializing_if = "Option::is_none")]
        expected_prior_sha256: Option<String>,
        /// Required hash of the installed result.
        result_sha256: String,
    },
    /// Removes one destination only when its current bytes have the expected hash.
    Remove {
        /// Destination relative to the storage root.
        final_path: ManifestPath,
        /// Required hash of the pre-removal file.
        expected_prior_sha256: String,
    },
}

/// Input used to construct one fully validated normal-commit manifest.
pub struct CommitManifestParts {
    /// Source directory whose storage root owns this manifest.
    pub source_id: SourceId,
    /// Current source lineage token that must match authority during recovery.
    pub source_instance_id: SourceInstanceId,
    /// Source generation installed by this commit.
    pub generation: u64,
    /// Already-existing measurement identities touched by this commit.
    pub touched_measurement_instances: BTreeMap<MeasurementId, MeasurementInstanceId>,
    /// New measurement identities registered atomically with their first state files.
    pub identity_measurement_additions: BTreeMap<MeasurementId, MeasurementInstanceId>,
    /// Domain operation that produced this commit.
    pub cause: CommitCause,
    /// Ordered idempotent file operations.
    pub entries: Vec<CommitOperation>,
}

/// Crash-recovery record for one source generation transition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommitManifest {
    schema_name: String,
    schema_version: u32,
    source_id: SourceId,
    source_instance_id: SourceInstanceId,
    generation: u64,
    touched_measurement_instances: BTreeMap<MeasurementId, MeasurementInstanceId>,
    identity_measurement_additions: BTreeMap<MeasurementId, MeasurementInstanceId>,
    cause: CommitCause,
    entries: Vec<CommitOperation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommitManifestWire {
    schema_name: String,
    schema_version: u32,
    source_id: SourceId,
    source_instance_id: SourceInstanceId,
    generation: u64,
    touched_measurement_instances: BTreeMap<MeasurementId, MeasurementInstanceId>,
    identity_measurement_additions: BTreeMap<MeasurementId, MeasurementInstanceId>,
    cause: CommitCause,
    entries: Vec<CommitOperation>,
}

impl<'de> Deserialize<'de> for CommitManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = CommitManifestWire::deserialize(deserializer)?;
        let manifest = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            source_id: wire.source_id,
            source_instance_id: wire.source_instance_id,
            generation: wire.generation,
            touched_measurement_instances: wire.touched_measurement_instances,
            identity_measurement_additions: wire.identity_measurement_additions,
            cause: wire.cause,
            entries: wire.entries,
        };
        manifest.validate().map_err(serde::de::Error::custom)?;
        Ok(manifest)
    }
}

impl CommitManifest {
    /// Builds a normal-commit manifest that is safe to persist as its durable commit point.
    pub fn new(parts: CommitManifestParts) -> Result<Self, CoreError> {
        let manifest = Self {
            schema_name: COMMIT_MANIFEST_SCHEMA_NAME.to_owned(),
            schema_version: COMMIT_MANIFEST_SCHEMA_VERSION,
            source_id: parts.source_id,
            source_instance_id: parts.source_instance_id,
            generation: parts.generation,
            touched_measurement_instances: parts.touched_measurement_instances,
            identity_measurement_additions: parts.identity_measurement_additions,
            cause: parts.cause,
            entries: parts.entries,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Returns the source directory this manifest is permitted to mutate.
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    /// Returns the source lineage token that recovery must match.
    pub fn source_instance_id(&self) -> &SourceInstanceId {
        &self.source_instance_id
    }

    /// Returns the generation installed by this manifest.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the pre-existing measurement identities touched by this commit.
    pub fn touched_measurement_instances(&self) -> &BTreeMap<MeasurementId, MeasurementInstanceId> {
        &self.touched_measurement_instances
    }

    /// Returns the measurement identities that this commit creates atomically with state.
    pub fn identity_measurement_additions(
        &self,
    ) -> &BTreeMap<MeasurementId, MeasurementInstanceId> {
        &self.identity_measurement_additions
    }

    /// Returns the ordered idempotent file operations.
    pub fn entries(&self) -> &[CommitOperation] {
        &self.entries
    }

    /// Applies the lineage and adjacent-generation recovery gate without touching storage.
    pub fn recover_against(
        &self,
        identity: &super::SourceIdentity,
        current_generation: u64,
    ) -> CommitRecovery {
        let lineage_matches = self.source_instance_id == *identity.source_instance_id();
        let touched_matches =
            self.touched_measurement_instances
                .iter()
                .all(|(measurement_id, instance_id)| {
                    identity
                        .measurements()
                        .get(measurement_id)
                        .is_some_and(|known| known.measurement_instance_id() == instance_id)
                });
        let additions_match =
            self.identity_measurement_additions
                .iter()
                .all(|(measurement_id, instance_id)| {
                    identity
                        .measurements()
                        .get(measurement_id)
                        .is_none_or(|known| known.measurement_instance_id() == instance_id)
                });
        let generation_matches = self.generation == current_generation
            || self.generation == current_generation.saturating_add(1);
        if lineage_matches && touched_matches && additions_match && generation_matches {
            CommitRecovery::Apply
        } else {
            CommitRecovery::Unresolvable
        }
    }

    /// Validates the closed manifest schema, lineage token, hash guards, and disjoint identity maps.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_name != COMMIT_MANIFEST_SCHEMA_NAME
            || self.schema_version != COMMIT_MANIFEST_SCHEMA_VERSION
            || self.generation == 0
            || self.entries.is_empty()
        {
            return Err(CoreError::contract(
                "commit manifest is not a current nonempty FFHN commit manifest",
            ));
        }
        self.source_instance_id.validate()?;
        if self
            .touched_measurement_instances
            .keys()
            .any(|measurement_id| {
                self.identity_measurement_additions
                    .contains_key(measurement_id)
            })
        {
            return Err(CoreError::contract(
                "commit manifest touched and identity-addition measurement maps must be disjoint",
            ));
        }
        if self.cause == CommitCause::DeliveryResult
            && !self.identity_measurement_additions.is_empty()
        {
            return Err(CoreError::contract(
                "delivery-result commit must not create measurement lineage",
            ));
        }
        for measurement_id in self.identity_measurement_additions.keys() {
            let required_state_path =
                format!("measurements/{}/state.json", measurement_id.as_str());
            if !self.entries.iter().any(|entry| {
                matches!(entry, CommitOperation::Install { final_path, .. } if final_path.as_str() == required_state_path)
            }) {
                return Err(CoreError::contract(
                    "commit identity addition requires its measurement state install operation",
                ));
            }
        }
        let mut final_paths = BTreeSet::new();
        let mut staged_paths = BTreeSet::new();
        let mut source_state_installs = 0_usize;
        for entry in &self.entries {
            match entry {
                CommitOperation::Install {
                    final_path,
                    staged_path,
                    ..
                } => {
                    if !final_paths.insert(final_path) || !staged_paths.insert(staged_path) {
                        return Err(CoreError::contract(
                            "commit manifest must not apply more than one operation to a final or staged path",
                        ));
                    }
                    if final_path.as_str() == "source-state.json" {
                        source_state_installs += 1;
                    }
                }
                CommitOperation::Remove { final_path, .. } => {
                    if !final_paths.insert(final_path) {
                        return Err(CoreError::contract(
                            "commit manifest must not apply more than one operation to a final or staged path",
                        ));
                    }
                }
            }
        }
        if source_state_installs != 1 {
            return Err(CoreError::contract(
                "every normal commit must install exactly one successor source-state document",
            ));
        }
        for instance_id in self
            .touched_measurement_instances
            .values()
            .chain(self.identity_measurement_additions.values())
        {
            instance_id.validate()?;
        }
        for entry in &self.entries {
            if self.cause == CommitCause::Acquisition
                && matches!(entry, CommitOperation::Remove { .. })
            {
                return Err(CoreError::contract(
                    "acquisition commit must not remove durable records",
                ));
            }
            entry.validate()?;
        }
        Ok(())
    }
}

fn require_sha256(field: &str, value: &str) -> Result<(), CoreError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(CoreError::contract(format!(
            "{field} must be lowercase SHA-256"
        )))
    }
}

#[cfg(test)]
#[path = "manifest/tests.rs"]
mod tests;

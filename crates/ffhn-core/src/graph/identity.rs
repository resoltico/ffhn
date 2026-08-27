//! Graph, source, and measurement lineage value objects and state envelopes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::{Uuid, Variant};

use crate::CoreError;

/// Canonical graph-identity schema name.
pub const GRAPH_IDENTITY_SCHEMA_NAME: &str = "ffhn.graph_identity";
/// Canonical graph-identity schema version.
pub const GRAPH_IDENTITY_SCHEMA_VERSION: u32 = 1;
/// Canonical source-identity schema name.
pub const SOURCE_IDENTITY_SCHEMA_NAME: &str = "ffhn.source_identity";
/// Canonical source-identity schema version.
pub const SOURCE_IDENTITY_SCHEMA_VERSION: u32 = 1;

macro_rules! instance_id {
    ($name:ident) => {
        /// Random UUIDv4 lineage token owned by the observation graph.
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Mints a fresh, non-reused lineage token.
            pub fn mint() -> Self {
                Self(Uuid::new_v4())
            }

            /// Verifies that this token is an RFC 4122 version-4 UUID.
            pub fn validate(&self) -> Result<(), CoreError> {
                if self.0.get_version_num() == 4 && self.0.get_variant() == Variant::RFC4122 {
                    Ok(())
                } else {
                    Err(CoreError::contract(concat!(
                        stringify!($name),
                        " must be a UUIDv4"
                    )))
                }
            }

            #[cfg(test)]
            pub(crate) const fn non_v4_for_tests() -> Self {
                Self(Uuid::nil())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = Self(Uuid::deserialize(deserializer)?);
                value.validate().map_err(serde::de::Error::custom)?;
                Ok(value)
            }
        }
    };
}

instance_id!(GraphId);
instance_id!(SourceInstanceId);
instance_id!(MeasurementInstanceId);

macro_rules! node_id {
    ($name:ident) => {
        /// Validated source- or measurement-directory identifier.
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Validates one identifier used as a source or measurement directory name.
            pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
                let value = value.into();
                if is_valid_node_id(&value) {
                    Ok(Self(value))
                } else {
                    Err(CoreError::contract(concat!(
                        stringify!($name),
                        " must use lowercase letters or digits separated only by one internal '-' or '_' and be at most 64 bytes"
                    )))
                }
            }

            /// Returns the validated identifier text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }

        impl std::str::FromStr for $name {
            type Err = CoreError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

node_id!(SourceId);
node_id!(MeasurementId);

/// Immutable root identity for one observation graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphIdentity {
    schema_name: String,
    schema_version: u32,
    graph_id: GraphId,
    created_at_utc: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GraphIdentityWire {
    schema_name: String,
    schema_version: u32,
    graph_id: GraphId,
    created_at_utc: String,
}

impl<'de> Deserialize<'de> for GraphIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = GraphIdentityWire::deserialize(deserializer)?;
        let identity = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            graph_id: wire.graph_id,
            created_at_utc: wire.created_at_utc,
        };
        identity.validate().map_err(serde::de::Error::custom)?;
        Ok(identity)
    }
}

impl GraphIdentity {
    /// Creates the immutable identity document for a new graph root.
    pub fn new(created_at_utc: String) -> Result<Self, CoreError> {
        let identity = Self {
            schema_name: GRAPH_IDENTITY_SCHEMA_NAME.to_owned(),
            schema_version: GRAPH_IDENTITY_SCHEMA_VERSION,
            graph_id: GraphId::mint(),
            created_at_utc,
        };
        identity.validate()?;
        Ok(identity)
    }

    /// Returns the graph-wide lineage token.
    pub fn graph_id(&self) -> &GraphId {
        &self.graph_id
    }

    /// Validates the closed graph-identity schema and its UUIDv4 token.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_name != GRAPH_IDENTITY_SCHEMA_NAME
            || self.schema_version != GRAPH_IDENTITY_SCHEMA_VERSION
        {
            return Err(CoreError::contract(
                "graph identity is not a current FFHN graph identity",
            ));
        }
        self.graph_id.validate()?;
        crate::model::require_canonical_utc_rfc3339(
            "graph identity created_at_utc",
            &self.created_at_utc,
        )
    }
}

/// Lineage authority for one source directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    schema_name: String,
    schema_version: u32,
    source_instance_id: SourceInstanceId,
    measurements: BTreeMap<MeasurementId, MeasurementIdentity>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceIdentityWire {
    schema_name: String,
    schema_version: u32,
    source_instance_id: SourceInstanceId,
    measurements: BTreeMap<MeasurementId, MeasurementIdentity>,
}

impl<'de> Deserialize<'de> for SourceIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = SourceIdentityWire::deserialize(deserializer)?;
        let identity = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            source_instance_id: wire.source_instance_id,
            measurements: wire.measurements,
        };
        identity.validate().map_err(serde::de::Error::custom)?;
        Ok(identity)
    }
}

impl SourceIdentity {
    /// Creates a fresh source identity with no measurement entries.
    pub fn fresh() -> Self {
        Self {
            schema_name: SOURCE_IDENTITY_SCHEMA_NAME.to_owned(),
            schema_version: SOURCE_IDENTITY_SCHEMA_VERSION,
            source_instance_id: SourceInstanceId::mint(),
            measurements: BTreeMap::new(),
        }
    }

    /// Returns this source's current lineage token.
    pub fn source_instance_id(&self) -> &SourceInstanceId {
        &self.source_instance_id
    }

    /// Returns the identity entries created atomically with measurement state.
    pub fn measurements(&self) -> &BTreeMap<MeasurementId, MeasurementIdentity> {
        &self.measurements
    }

    /// Registers the identity minted for a measurement's atomic first-state creation.
    pub fn register_measurement(
        &mut self,
        measurement_id: MeasurementId,
        measurement_identity: MeasurementIdentity,
    ) -> Result<(), CoreError> {
        measurement_identity.validate()?;
        if self.measurements.contains_key(&measurement_id) {
            return Err(CoreError::contract(
                "measurement identity already exists and cannot be registered again",
            ));
        }
        self.measurements
            .insert(measurement_id, measurement_identity);
        Ok(())
    }

    /// Registers the exact instance id already minted in a durable first-projection commit manifest.
    pub fn register_measurement_instance(
        &mut self,
        measurement_id: MeasurementId,
        measurement_instance_id: MeasurementInstanceId,
    ) -> Result<(), CoreError> {
        self.register_measurement(
            measurement_id,
            MeasurementIdentity::from_first_instance(measurement_instance_id)?,
        )
    }

    /// Mints and installs the one measurement identity changed by an explicit measurement reset.
    pub fn reset_measurement(
        &mut self,
        measurement_id: MeasurementId,
    ) -> Result<MeasurementIdentity, CoreError> {
        let replacement = MeasurementIdentity::reset_from(self.measurements.get(&measurement_id))?;
        self.measurements
            .insert(measurement_id, replacement.clone());
        Ok(replacement)
    }

    /// Validates the closed source-identity schema and all registered lineage entries.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_name != SOURCE_IDENTITY_SCHEMA_NAME
            || self.schema_version != SOURCE_IDENTITY_SCHEMA_VERSION
        {
            return Err(CoreError::contract(
                "source identity is not a current FFHN source identity",
            ));
        }
        self.source_instance_id.validate()?;
        for identity in self.measurements.values() {
            identity.validate()?;
        }
        Ok(())
    }
}

/// One measurement's authority entry, created atomically with its first state document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeasurementIdentity {
    measurement_instance_id: MeasurementInstanceId,
    reset_count: u64,
}

impl MeasurementIdentity {
    /// Creates a fresh measurement lineage entry for an atomic state creation or reset.
    pub fn fresh() -> Self {
        Self {
            measurement_instance_id: MeasurementInstanceId::mint(),
            reset_count: 0,
        }
    }

    /// Reconstitutes a first-projection identity entry from a manifest-minted instance token.
    pub fn from_first_instance(
        measurement_instance_id: MeasurementInstanceId,
    ) -> Result<Self, CoreError> {
        measurement_instance_id.validate()?;
        Ok(Self {
            measurement_instance_id,
            reset_count: 0,
        })
    }

    /// Mints a replacement identity and advances its audit-only reset count when one existed.
    pub fn reset_from(previous: Option<&Self>) -> Result<Self, CoreError> {
        let reset_count = match previous {
            Some(identity) => identity
                .reset_count
                .checked_add(1)
                .ok_or_else(|| CoreError::contract("measurement reset_count overflowed"))?,
            None => 1,
        };
        Ok(Self {
            measurement_instance_id: MeasurementInstanceId::mint(),
            reset_count,
        })
    }

    /// Returns this measurement's current lineage token.
    pub fn measurement_instance_id(&self) -> &MeasurementInstanceId {
        &self.measurement_instance_id
    }

    /// Returns the audit-only number of explicit measurement resets in this source lineage.
    pub const fn reset_count(&self) -> u64 {
        self.reset_count
    }

    /// Validates this measurement identity entry.
    pub fn validate(&self) -> Result<(), CoreError> {
        self.measurement_instance_id.validate()
    }
}

fn is_valid_node_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return false;
    }
    let mut previous_was_separator = true;
    for byte in bytes {
        let is_alphanumeric = byte.is_ascii_lowercase() || byte.is_ascii_digit();
        let is_separator = matches!(byte, b'-' | b'_');
        if !is_alphanumeric && !is_separator {
            return false;
        }
        if is_separator && previous_was_separator {
            return false;
        }
        previous_was_separator = is_separator;
    }
    !previous_was_separator
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lineage_authority_uses_uuidv4s_and_never_preallocates_measurement_entries() {
        let graph = GraphIdentity::new("2026-08-25T00:00:00Z".to_owned()).expect("graph");
        graph.validate().expect("graph identity");
        assert_ne!(
            graph.graph_id().0,
            GraphIdentity::new("2026-08-25T00:00:01Z".to_owned())
                .expect("other graph")
                .graph_id()
                .0
        );
        let source = SourceIdentity::fresh();
        source.validate().expect("source identity");
        assert!(source.measurements().is_empty());
        assert_ne!(
            source.source_instance_id().0,
            SourceIdentity::fresh().source_instance_id().0
        );
    }

    #[test]
    fn graph_node_ids_are_closed_directory_names() {
        for value in ["source", "source-42", "source_42", "a1-b2_c3"] {
            assert_eq!(SourceId::new(value).expect("source id").as_str(), value);
            assert_eq!(
                MeasurementId::new(value).expect("measurement id").as_str(),
                value
            );
        }
        for value in [
            "",
            "-source",
            "source-",
            "source__id",
            "Source",
            "source/id",
            "source--id",
            "source id",
            &"a".repeat(65),
        ] {
            assert!(SourceId::new(value).is_err(), "{value}");
            assert!(MeasurementId::new(value).is_err(), "{value}");
        }
        let source = SourceId::new("source-42").expect("source id");
        let measurement = MeasurementId::new("measurement-42").expect("measurement id");
        assert_eq!(source.to_string(), "source-42");
        assert_eq!(measurement.to_string(), "measurement-42");
        assert_eq!("source-42".parse::<SourceId>().expect("from str"), source);
        assert_eq!(
            "measurement-42".parse::<MeasurementId>().expect("from str"),
            measurement
        );

        for valid in ["a", "1", "a1", "a-b", "a_b", &"a".repeat(64)] {
            assert!(is_valid_node_id(valid), "{valid}");
        }
        for invalid in [
            "",
            &"a".repeat(65),
            "A",
            "-a",
            "_a",
            "a-",
            "a_",
            "a--b",
            "a__b",
            "a-_b",
            "a_-b",
            "a/b",
            "a b",
        ] {
            assert!(!is_valid_node_id(invalid), "{invalid}");
        }
    }

    #[test]
    fn public_graph_documents_reject_retired_schemas_and_non_v4_lineage_tokens() {
        let graph = GraphIdentity::new("2026-08-25T00:00:00Z".to_owned()).expect("graph");
        let mut graph_wire = serde_json::to_value(graph).expect("graph wire");
        graph_wire["schema_version"] = serde_json::json!(2);
        assert!(serde_json::from_value::<GraphIdentity>(graph_wire).is_err());

        let mut source_wire = serde_json::to_value(SourceIdentity::fresh()).expect("source wire");
        source_wire["source_instance_id"] =
            serde_json::json!("00000000-0000-1000-8000-000000000000");
        assert!(serde_json::from_value::<SourceIdentity>(source_wire).is_err());

        let mut graph_wire = serde_json::to_value(
            GraphIdentity::new("2026-08-25T00:00:00Z".to_owned()).expect("graph"),
        )
        .expect("graph wire");
        graph_wire["schema_name"] = serde_json::json!("foreign.graph_identity");
        assert!(serde_json::from_value::<GraphIdentity>(graph_wire).is_err());
        assert!(GraphIdentity::new("2026-08-25T00:00:00+00:00".to_owned()).is_err());

        for (field, value) in [
            ("schema_name", serde_json::json!("foreign.source_identity")),
            ("schema_version", serde_json::json!(2)),
        ] {
            let mut wire = serde_json::to_value(SourceIdentity::fresh()).expect("source wire");
            wire[field] = value;
            assert!(serde_json::from_value::<SourceIdentity>(wire).is_err());
        }

        let mut source_wire = serde_json::to_value(SourceIdentity::fresh()).expect("source wire");
        source_wire["source_instance_id"] =
            serde_json::json!("00000000-0000-4000-0000-000000000000");
        assert!(serde_json::from_value::<SourceIdentity>(source_wire).is_err());
    }

    #[test]
    fn source_identity_registers_first_state_once_and_resets_only_one_measurement() {
        let mut source = SourceIdentity::fresh();
        let first_id = MeasurementId::new("first").expect("first id");
        let second_id = MeasurementId::new("second").expect("second id");
        source
            .register_measurement(first_id.clone(), MeasurementIdentity::fresh())
            .expect("first registration");
        source
            .register_measurement(second_id.clone(), MeasurementIdentity::fresh())
            .expect("second registration");
        assert!(
            source
                .register_measurement(first_id.clone(), MeasurementIdentity::fresh())
                .is_err()
        );

        let first_before = source
            .measurements()
            .get(&first_id)
            .expect("first identity")
            .clone();
        let second_before = source
            .measurements()
            .get(&second_id)
            .expect("second identity")
            .clone();
        let first_after = source
            .reset_measurement(first_id.clone())
            .expect("first reset");

        assert_ne!(first_after, first_before);
        assert_eq!(first_after.reset_count(), 1);
        assert_eq!(source.measurements().get(&second_id), Some(&second_before));
        assert_eq!(source.measurements().get(&first_id), Some(&first_after));

        let orphan = source
            .reset_measurement(MeasurementId::new("orphan").expect("orphan id"))
            .expect("orphan reset");
        assert_eq!(orphan.reset_count(), 1);

        let minted = MeasurementInstanceId::mint();
        let third_id = MeasurementId::new("third").expect("third id");
        source
            .register_measurement_instance(third_id.clone(), minted.clone())
            .expect("manifest registration");
        let third = source
            .measurements()
            .get(&third_id)
            .expect("third identity");
        assert_eq!(third.measurement_instance_id(), &minted);
        assert_eq!(third.reset_count(), 0);
        third.validate().expect("valid identity");
        let invalid = MeasurementIdentity {
            measurement_instance_id: MeasurementInstanceId::non_v4_for_tests(),
            reset_count: 0,
        };
        assert!(invalid.validate().is_err());
    }
}

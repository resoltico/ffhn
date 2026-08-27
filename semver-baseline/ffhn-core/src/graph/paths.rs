//! Canonical v11 graph-root and source-directory layout.

use std::path::{Path, PathBuf};

use super::{MeasurementId, SourceId};

/// Resolved filesystem layout for one v11 graph root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphPaths {
    root: PathBuf,
}

impl GraphPaths {
    /// Binds path generation to one graph root without opening or creating it.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the graph root supplied by the caller.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the graph-wide agent configuration path.
    pub fn agent_file(&self) -> PathBuf {
        self.root.join("agent.toml")
    }

    /// Returns the immutable graph-identity document path.
    pub fn identity_file(&self) -> PathBuf {
        self.root.join(".ffhn-graph.json")
    }

    /// Returns the singleton agent-lease path.
    pub fn agent_lock_file(&self) -> PathBuf {
        self.root.join(".ffhn-agent.lock")
    }

    /// Returns the containing source-directory root.
    pub fn sources_dir(&self) -> PathBuf {
        self.root.join("sources")
    }

    /// Resolves all paths owned by one validated source identifier.
    pub fn source(&self, source_id: SourceId) -> SourcePaths {
        SourcePaths {
            graph_root: self.root.clone(),
            source_id,
        }
    }
}

/// Resolved filesystem layout for one v11 source directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcePaths {
    graph_root: PathBuf,
    source_id: SourceId,
}

impl SourcePaths {
    /// Returns the graph root containing this source.
    pub fn graph_root(&self) -> &Path {
        &self.graph_root
    }

    /// Returns this source's validated identifier.
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    /// Returns the source directory that is the lineage-operation root.
    pub fn source_dir(&self) -> PathBuf {
        self.graph_root
            .join("sources")
            .join(self.source_id.as_str())
    }

    /// Returns the source configuration document path.
    pub fn source_file(&self) -> PathBuf {
        self.source_dir().join("source.toml")
    }

    /// Returns the source lock path, intentionally outside the storage swap scope.
    pub fn lock_file(&self) -> PathBuf {
        self.source_dir().join(".ffhn.lock")
    }

    /// Returns the sole lineage-authority document path.
    pub fn identity_file(&self) -> PathBuf {
        self.source_dir().join(".ffhn-identity.json")
    }

    /// Returns the durable lineage-transition manifest path.
    pub fn lineage_manifest_file(&self) -> PathBuf {
        self.source_dir().join(".ffhn-lineage.manifest")
    }

    /// Returns the fixed implementation-owned tombstone path.
    pub fn tombstone_dir(&self) -> PathBuf {
        self.source_dir().join(".ffhn-tombstone")
    }

    /// Returns the storage swap root.
    pub fn storage_dir(&self) -> PathBuf {
        self.source_dir().join(".ffhn")
    }

    /// Returns the durable source-state document path.
    pub fn source_state_file(&self) -> PathBuf {
        self.storage_dir().join("source-state.json")
    }

    /// Returns the durable normal-commit manifest path.
    pub fn commit_manifest_file(&self) -> PathBuf {
        self.storage_dir().join("commit.manifest")
    }

    /// Returns the source-owned pending-outbox directory.
    pub fn source_outbox_dir(&self) -> PathBuf {
        self.storage_dir().join("source-outbox")
    }

    /// Returns the source-owned dead-letter directory.
    pub fn source_dead_letters_dir(&self) -> PathBuf {
        self.storage_dir().join("dead-letters")
    }

    /// Returns the static configuration directory for all measurements under this source.
    pub fn measurements_dir(&self) -> PathBuf {
        self.source_dir().join("measurements")
    }

    /// Returns one measurement configuration path.
    pub fn measurement_file(&self, measurement_id: &MeasurementId) -> PathBuf {
        self.measurements_dir()
            .join(measurement_id.as_str())
            .join("measurement.toml")
    }

    /// Returns one measurement's state and outbox root, created only at first projection.
    pub fn measurement_storage_dir(&self, measurement_id: &MeasurementId) -> PathBuf {
        self.storage_dir()
            .join("measurements")
            .join(measurement_id.as_str())
    }

    /// Returns one measurement's pending-delivery directory.
    pub fn measurement_outbox_dir(&self, measurement_id: &MeasurementId) -> PathBuf {
        self.measurement_storage_dir(measurement_id).join("outbox")
    }

    /// Returns one measurement's terminal-delivery directory.
    pub fn measurement_dead_letters_dir(&self, measurement_id: &MeasurementId) -> PathBuf {
        self.measurement_storage_dir(measurement_id)
            .join("dead-letters")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_and_source_paths_are_fixed_and_identifier_derived() {
        let graph = GraphPaths::new("/graph");
        let source = graph.source(SourceId::new("shop-product").expect("source id"));
        let measurement = MeasurementId::new("displayed-price").expect("measurement id");

        assert_eq!(graph.root(), Path::new("/graph"));
        assert_eq!(
            graph.agent_lock_file(),
            PathBuf::from("/graph/.ffhn-agent.lock")
        );
        assert_eq!(graph.sources_dir(), PathBuf::from("/graph/sources"));
        assert_eq!(source.graph_root(), Path::new("/graph"));
        assert_eq!(source.source_id().as_str(), "shop-product");
        assert_eq!(graph.agent_file(), PathBuf::from("/graph/agent.toml"));
        assert_eq!(
            graph.identity_file(),
            PathBuf::from("/graph/.ffhn-graph.json")
        );
        assert_eq!(
            source.source_file(),
            PathBuf::from("/graph/sources/shop-product/source.toml")
        );
        assert_eq!(
            source.lock_file(),
            PathBuf::from("/graph/sources/shop-product/.ffhn.lock")
        );
        assert_eq!(
            source.identity_file(),
            PathBuf::from("/graph/sources/shop-product/.ffhn-identity.json")
        );
        assert_eq!(
            source.measurement_file(&measurement),
            PathBuf::from(
                "/graph/sources/shop-product/measurements/displayed-price/measurement.toml"
            )
        );
        assert_eq!(
            source.measurement_storage_dir(&measurement),
            PathBuf::from("/graph/sources/shop-product/.ffhn/measurements/displayed-price")
        );
    }
}

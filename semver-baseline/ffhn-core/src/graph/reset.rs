//! Mint-only graph reset commands over the lineage-transition protocol.

use crate::CoreError;

use super::{
    LineageManifest, LineageManifestParts, LineageScope, MeasurementId, SourceId, SourceTurnEntry,
    TrustedGraphRoot, UnresolvableManifest,
};

/// Result facts from one graph reset command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphResetResult {
    source_id: SourceId,
    measurement_id: Option<MeasurementId>,
    discarded_manifests: Vec<DiscardedManifestEvidence>,
}

/// Opaque evidence for one unresolvable commit point discarded by source reset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscardedManifestEvidence {
    kind: UnresolvableManifest,
    bytes: Option<Vec<u8>>,
}

impl GraphResetResult {
    /// Returns the reset source identifier.
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }
    /// Returns the reset measurement when this was a measurement-scoped reset.
    pub fn measurement_id(&self) -> Option<&MeasurementId> {
        self.measurement_id.as_ref()
    }
    /// Returns every unresolvable commit point discarded by this reset.
    pub fn discarded_manifests(&self) -> &[DiscardedManifestEvidence] {
        &self.discarded_manifests
    }
}

impl DiscardedManifestEvidence {
    /// Returns the discarded manifest class.
    pub const fn kind(&self) -> UnresolvableManifest {
        self.kind
    }
    /// Returns exact opaque artifact bytes when they were safely readable.
    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
}

/// Performs a blind, fresh-lineage source reset under the source writer lock.
pub fn reset_source(
    graph: &TrustedGraphRoot,
    source_id: SourceId,
) -> Result<GraphResetResult, CoreError> {
    graph.validate_graph_documents()?;
    let source = graph.open_source(source_id.clone())?;
    let Some(_lease) = source.try_acquire_write_lease()? else {
        return Err(CoreError::contract("source is busy"));
    };
    let turn_entry = source.recover_turn_entry()?;
    let discarded_manifests = match turn_entry {
        SourceTurnEntry::Ready => Vec::new(),
        SourceTurnEntry::Unresolvable(kind) => {
            let bytes = match kind {
                UnresolvableManifest::Lineage => {
                    source.read_lineage_manifest_bytes().ok().flatten()
                }
                UnresolvableManifest::Commit => source
                    .open_storage()
                    .ok()
                    .and_then(|storage| storage.read_commit_manifest_bytes().ok().flatten()),
            };
            vec![DiscardedManifestEvidence { kind, bytes }]
        }
    };
    let scope = if matches!(source.read_identity(), Ok(None)) {
        LineageScope::Init
    } else {
        LineageScope::SourceReset
    };
    source.apply_blind_source_transition(scope)?;
    Ok(GraphResetResult {
        source_id,
        measurement_id: None,
        discarded_manifests,
    })
}

/// Performs one measurement-scoped reset while preserving source and sibling lineage.
pub fn reset_measurement(
    graph: &TrustedGraphRoot,
    source_id: SourceId,
    measurement_id: MeasurementId,
) -> Result<GraphResetResult, CoreError> {
    graph.validate_graph_documents()?;
    let source = graph.open_source(source_id.clone())?;
    let Some(_lease) = source.try_acquire_write_lease()? else {
        return Err(CoreError::contract("source is busy"));
    };
    if !matches!(source.recover_turn_entry()?, SourceTurnEntry::Ready) {
        return Err(CoreError::contract(
            "measurement reset is refused while a source manifest is unresolvable; reset the source",
        ));
    }
    let prior = source
        .read_identity()?
        .ok_or_else(|| CoreError::contract("measurement reset requires source identity"))?;
    let mut target = prior.clone();
    target.reset_measurement(measurement_id.clone())?;
    let manifest = LineageManifest::new(LineageManifestParts {
        source_id: source_id.clone(),
        scope: LineageScope::MeasurementReset {
            measurement_id: measurement_id.clone(),
        },
        from: Some(prior),
        target,
    })?;
    source.apply_measurement_transition(&manifest)?;
    Ok(GraphResetResult {
        source_id,
        measurement_id: Some(measurement_id),
        discarded_manifests: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::graph::{GraphPaths, SourceIdentity, SourceState};

    #[test]
    fn source_reset_mints_fresh_lineage_without_parsing_corrupt_prior_authority() {
        let temporary = tempfile::tempdir().expect("temporary graph");
        let root = temporary.path().join("graph");
        let graph = TrustedGraphRoot::initialize(
            GraphPaths::new(root.clone()),
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("graph");
        fs::create_dir_all(root.join("sources/demo/.ffhn")).expect("storage");
        let source_id = SourceId::new("demo").expect("source id");
        let source = graph.open_source(source_id.clone()).expect("source");
        let identity = SourceIdentity::fresh();
        source.write_identity(&identity).expect("identity");
        source
            .open_storage()
            .expect("storage")
            .write_source_state(&SourceState::fresh(identity.source_instance_id().clone()))
            .expect("state");
        let original = identity.source_instance_id().clone();
        reset_source(&graph, source_id).expect("reset");
        assert_ne!(
            source
                .read_identity()
                .expect("identity")
                .expect("fresh")
                .source_instance_id(),
            &original
        );
    }

    #[test]
    fn source_reset_reports_exact_discarded_manifest_bytes_without_parsing_them() {
        let temporary = tempfile::tempdir().expect("temporary graph");
        let root = temporary.path().join("graph");
        let graph = TrustedGraphRoot::initialize(
            GraphPaths::new(root.clone()),
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("graph");
        fs::create_dir_all(root.join("sources/demo")).expect("source directory");
        let bytes = [0_u8, 0xff, b'{'];
        fs::write(root.join("sources/demo/.ffhn-lineage.manifest"), bytes)
            .expect("opaque manifest");
        let result =
            reset_source(&graph, SourceId::new("demo").expect("source id")).expect("blind reset");
        assert_eq!(result.discarded_manifests().len(), 1);
        assert_eq!(
            result.discarded_manifests()[0].kind(),
            UnresolvableManifest::Lineage
        );
        assert_eq!(
            result.discarded_manifests()[0].bytes(),
            Some(bytes.as_slice())
        );
        let report = serde_json::to_value(super::super::GraphResetReport::from(&result))
            .expect("reset report");
        assert_eq!(report["discarded_manifests"][0]["bytes_base64"], "AP97");
    }

    #[test]
    fn source_and_measurement_reset_accessors_busy_missing_identity_and_commit_evidence_are_complete()
     {
        let temporary = tempfile::tempdir().expect("temporary graph");
        let root = temporary.path().join("graph");
        let graph = TrustedGraphRoot::initialize(
            GraphPaths::new(root.clone()),
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("graph");
        fs::create_dir_all(root.join("sources/demo")).expect("source");
        let source_id = SourceId::new("demo").expect("source id");
        let source = graph.open_source(source_id.clone()).expect("source");
        assert!(
            reset_measurement(
                &graph,
                source_id.clone(),
                MeasurementId::new("price").expect("measurement"),
            )
            .is_err()
        );
        let initialized = reset_source(&graph, source_id.clone()).expect("initialize source");
        assert_eq!(initialized.source_id(), &source_id);
        assert!(initialized.measurement_id().is_none());
        assert!(initialized.discarded_manifests().is_empty());

        let measurement_id = MeasurementId::new("price").expect("measurement");
        let reset = reset_measurement(&graph, source_id.clone(), measurement_id.clone())
            .expect("measurement reset");
        assert_eq!(reset.source_id(), &source_id);
        assert_eq!(reset.measurement_id(), Some(&measurement_id));

        let lease = source
            .try_acquire_write_lease()
            .expect("lock")
            .expect("lease");
        assert!(reset_source(&graph, source_id.clone()).is_err());
        assert!(reset_measurement(&graph, source_id.clone(), measurement_id.clone()).is_err());
        drop(lease);

        fs::write(source.paths().lineage_manifest_file(), "not-json").expect("manifest");
        assert!(reset_measurement(&graph, source_id, measurement_id).is_err());

        let evidence = DiscardedManifestEvidence {
            kind: UnresolvableManifest::Commit,
            bytes: None,
        };
        assert_eq!(evidence.kind(), UnresolvableManifest::Commit);
        assert!(evidence.bytes().is_none());
    }

    #[test]
    fn source_reset_reports_opaque_unresolvable_commit_manifest_bytes() {
        let temporary = tempfile::tempdir().expect("temporary graph");
        let root = temporary.path().join("graph");
        let graph = TrustedGraphRoot::initialize(
            GraphPaths::new(root.clone()),
            "2026-08-25T00:00:00Z".to_owned(),
        )
        .expect("graph");
        fs::create_dir_all(root.join("sources/demo/.ffhn")).expect("storage");
        let bytes = b"not a commit manifest";
        fs::write(root.join("sources/demo/.ffhn/commit.manifest"), bytes).expect("manifest");
        let result =
            reset_source(&graph, SourceId::new("demo").expect("source")).expect("source reset");
        assert_eq!(result.discarded_manifests().len(), 1);
        assert_eq!(
            result.discarded_manifests()[0].kind(),
            UnresolvableManifest::Commit
        );
        assert_eq!(
            result.discarded_manifests()[0].bytes(),
            Some(bytes.as_slice())
        );
    }
}

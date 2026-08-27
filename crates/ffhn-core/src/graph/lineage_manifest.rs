//! Typed v11 lineage-transition manifest contract.

use serde::{Deserialize, Serialize};

use crate::CoreError;

use super::{MeasurementId, SourceId, SourceIdentity};

/// Canonical lineage-transition manifest schema name.
pub const LINEAGE_MANIFEST_SCHEMA_NAME: &str = "ffhn.lineage_manifest";
/// Canonical lineage-transition manifest schema version.
pub const LINEAGE_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Scope of one mint-only lineage transition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LineageScope {
    /// Establishes a graph source that had no identity or storage.
    Init,
    /// Discards one complete source lineage and starts a fresh source identity.
    SourceReset,
    /// Replaces one measurement lineage while preserving every sibling identity.
    MeasurementReset {
        /// Measurement whose state subtree and identity entry are replaced.
        measurement_id: MeasurementId,
    },
}

/// Required recovery action after observing source identity while a lineage manifest is durable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineageRecovery {
    /// Install the target identity, then continue the idempotent scope swap.
    ApplyTargetIdentity,
    /// The target identity is already durable; continue the idempotent scope swap.
    ContinueScopeSwap,
    /// The manifest cannot be safely interpreted under the observed authority.
    Unresolvable,
}

/// Input used to construct one fully validated lineage-transition manifest.
pub struct LineageManifestParts {
    /// Source directory whose lineage authority changes.
    pub source_id: SourceId,
    /// Declared reset or initialization scope.
    pub scope: LineageScope,
    /// Authority identity before the transition; absent for initialization and a deliberately
    /// blind source reset that cannot safely read old authority.
    pub from: Option<SourceIdentity>,
    /// Fresh authority identity after the transition.
    pub target: SourceIdentity,
}

/// Durable transition record that makes source or measurement lineage recovery idempotent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageManifest {
    schema_name: String,
    schema_version: u32,
    source_id: SourceId,
    scope: LineageScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<SourceIdentity>,
    target: SourceIdentity,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LineageManifestWire {
    schema_name: String,
    schema_version: u32,
    source_id: SourceId,
    scope: LineageScope,
    #[serde(default)]
    from: Option<SourceIdentity>,
    target: SourceIdentity,
}

impl<'de> Deserialize<'de> for LineageManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = LineageManifestWire::deserialize(deserializer)?;
        let manifest = Self {
            schema_name: wire.schema_name,
            schema_version: wire.schema_version,
            source_id: wire.source_id,
            scope: wire.scope,
            from: wire.from,
            target: wire.target,
        };
        manifest.validate().map_err(serde::de::Error::custom)?;
        Ok(manifest)
    }
}

impl LineageManifest {
    /// Builds a lineage transition whose before and after authority facts are self-consistent.
    pub fn new(parts: LineageManifestParts) -> Result<Self, CoreError> {
        let manifest = Self {
            schema_name: LINEAGE_MANIFEST_SCHEMA_NAME.to_owned(),
            schema_version: LINEAGE_MANIFEST_SCHEMA_VERSION,
            source_id: parts.source_id,
            scope: parts.scope,
            from: parts.from,
            target: parts.target,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Returns the source directory whose authority is transitioned.
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    /// Returns the transition scope.
    pub fn scope(&self) -> &LineageScope {
        &self.scope
    }

    /// Returns the pre-transition authority identity when one existed.
    pub fn from(&self) -> Option<&SourceIdentity> {
        self.from.as_ref()
    }

    /// Returns the post-transition authority identity.
    pub fn target(&self) -> &SourceIdentity {
        &self.target
    }

    /// Validates the manifest schema and the reset scope's lineage mutation boundary.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_name != LINEAGE_MANIFEST_SCHEMA_NAME
            || self.schema_version != LINEAGE_MANIFEST_SCHEMA_VERSION
        {
            return Err(CoreError::contract(
                "lineage manifest is not a current FFHN lineage manifest",
            ));
        }
        self.target.validate()?;
        if let Some(from) = &self.from {
            from.validate()?;
        }
        match (&self.scope, &self.from) {
            (LineageScope::Init, None) if self.target.measurements().is_empty() => Ok(()),
            (LineageScope::Init, None) => Err(CoreError::contract(
                "initial lineage manifest target must not preallocate measurement identities",
            )),
            (LineageScope::Init, Some(_)) => Err(CoreError::contract(
                "initial lineage manifest must not carry a prior identity",
            )),
            (LineageScope::SourceReset, Some(from))
                if self.target.measurements().is_empty()
                    && self.target.source_instance_id() != from.source_instance_id() =>
            {
                Ok(())
            }
            (LineageScope::SourceReset, None) if self.target.measurements().is_empty() => Ok(()),
            (LineageScope::SourceReset, _) => Err(CoreError::contract(
                "source-reset lineage manifest must mint a fresh source identity and must not preallocate measurement identities",
            )),
            (LineageScope::MeasurementReset { measurement_id }, Some(from)) => {
                self.validate_measurement_reset(measurement_id, from)
            }
            (LineageScope::MeasurementReset { .. }, None) => Err(CoreError::contract(
                "measurement-reset lineage manifest requires a prior identity",
            )),
        }
    }

    /// Classifies a durable manifest against the currently observed source identity.
    pub fn recover_against(&self, observed: Option<&SourceIdentity>) -> LineageRecovery {
        match observed {
            None if matches!(self.scope, LineageScope::Init) => {
                LineageRecovery::ApplyTargetIdentity
            }
            Some(identity) if self.from.as_ref() == Some(identity) => {
                LineageRecovery::ApplyTargetIdentity
            }
            Some(identity) if identity == &self.target => LineageRecovery::ContinueScopeSwap,
            None | Some(_) => LineageRecovery::Unresolvable,
        }
    }

    fn validate_measurement_reset(
        &self,
        measurement_id: &MeasurementId,
        from: &SourceIdentity,
    ) -> Result<(), CoreError> {
        if from.source_instance_id() != self.target.source_instance_id() {
            return Err(CoreError::contract(
                "measurement reset must preserve the source instance identity",
            ));
        }
        let Some(target_measurement) = self.target.measurements().get(measurement_id) else {
            return Err(CoreError::contract(
                "measurement reset target must contain the reset measurement identity",
            ));
        };
        for (id, prior_measurement) in from.measurements() {
            let Some(target_entry) = self.target.measurements().get(id) else {
                return Err(CoreError::contract(
                    "measurement reset must preserve every non-reset measurement identity",
                ));
            };
            if id != measurement_id && target_entry != prior_measurement {
                return Err(CoreError::contract(
                    "measurement reset must not change a sibling measurement identity",
                ));
            }
        }
        for id in self.target.measurements().keys() {
            if id != measurement_id && !from.measurements().contains_key(id) {
                return Err(CoreError::contract(
                    "measurement reset must not add an unrelated measurement identity",
                ));
            }
        }
        if from
            .measurements()
            .get(measurement_id)
            .is_some_and(|prior| prior == target_measurement)
        {
            return Err(CoreError::contract(
                "measurement reset must mint a fresh measurement identity",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::graph::MeasurementIdentity;

    fn source_identity_with(
        source_instance_id: super::super::SourceInstanceId,
        measurements: BTreeMap<MeasurementId, MeasurementIdentity>,
    ) -> SourceIdentity {
        let value = serde_json::json!({
            "schema_name": "ffhn.source_identity",
            "schema_version": 1,
            "source_instance_id": source_instance_id,
            "measurements": measurements,
        });
        serde_json::from_value(value).expect("source identity")
    }

    #[test]
    fn source_transitions_start_with_empty_measurement_identity_maps() {
        let source = SourceIdentity::fresh();
        LineageManifest::new(LineageManifestParts {
            source_id: SourceId::new("source").expect("source id"),
            scope: LineageScope::Init,
            from: None,
            target: SourceIdentity::fresh(),
        })
        .expect("init manifest");
        let result = LineageManifest::new(LineageManifestParts {
            source_id: SourceId::new("source").expect("source id"),
            scope: LineageScope::SourceReset,
            from: Some(source.clone()),
            target: source_identity_with(
                super::super::SourceInstanceId::mint(),
                BTreeMap::from([(
                    MeasurementId::new("preallocated").expect("measurement id"),
                    MeasurementIdentity::fresh(),
                )]),
            ),
        });
        assert!(result.is_err());
        let same_lineage = LineageManifest::new(LineageManifestParts {
            source_id: SourceId::new("source").expect("source id"),
            scope: LineageScope::SourceReset,
            from: Some(source.clone()),
            target: source_identity_with(source.source_instance_id().clone(), BTreeMap::new()),
        });
        assert!(same_lineage.is_err());
    }

    #[test]
    fn measurement_reset_changes_only_the_named_measurement_lineage() {
        let source_instance_id = super::super::SourceInstanceId::mint();
        let reset_id = MeasurementId::new("reset").expect("reset id");
        let sibling_id = MeasurementId::new("sibling").expect("sibling id");
        let reset_prior = MeasurementIdentity::fresh();
        let sibling = MeasurementIdentity::fresh();
        let from = source_identity_with(
            source_instance_id.clone(),
            BTreeMap::from([
                (reset_id.clone(), reset_prior),
                (sibling_id.clone(), sibling.clone()),
            ]),
        );
        let target = source_identity_with(
            source_instance_id,
            BTreeMap::from([
                (reset_id.clone(), MeasurementIdentity::fresh()),
                (sibling_id, sibling),
            ]),
        );
        LineageManifest::new(LineageManifestParts {
            source_id: SourceId::new("source").expect("source id"),
            scope: LineageScope::MeasurementReset {
                measurement_id: reset_id,
            },
            from: Some(from),
            target,
        })
        .expect("measurement reset manifest");
    }

    #[test]
    fn recovery_uses_only_the_manifest_authority_transition_table() {
        let target = SourceIdentity::fresh();
        let init = LineageManifest::new(LineageManifestParts {
            source_id: SourceId::new("source").expect("source id"),
            scope: LineageScope::Init,
            from: None,
            target: target.clone(),
        })
        .expect("init manifest");
        assert_eq!(
            init.recover_against(None),
            LineageRecovery::ApplyTargetIdentity
        );
        assert_eq!(
            init.recover_against(Some(&target)),
            LineageRecovery::ContinueScopeSwap
        );

        let from = SourceIdentity::fresh();
        let reset_target = SourceIdentity::fresh();
        let reset = LineageManifest::new(LineageManifestParts {
            source_id: SourceId::new("source").expect("source id"),
            scope: LineageScope::SourceReset,
            from: Some(from.clone()),
            target: reset_target.clone(),
        })
        .expect("source reset manifest");
        assert_eq!(
            reset.recover_against(Some(&from)),
            LineageRecovery::ApplyTargetIdentity
        );
        assert_eq!(
            reset.recover_against(Some(&reset_target)),
            LineageRecovery::ContinueScopeSwap
        );
        assert_eq!(reset.recover_against(None), LineageRecovery::Unresolvable);
        assert_eq!(
            reset.recover_against(Some(&SourceIdentity::fresh())),
            LineageRecovery::Unresolvable
        );
    }

    #[test]
    fn blind_source_reset_manifest_is_only_resumable_after_its_new_authority_is_installed() {
        let prior = SourceIdentity::fresh();
        let target = SourceIdentity::fresh();
        let reset = LineageManifest::new(LineageManifestParts {
            source_id: SourceId::new("source").expect("source id"),
            scope: LineageScope::SourceReset,
            from: None,
            target: target.clone(),
        })
        .expect("blind reset manifest");
        assert_eq!(
            reset.recover_against(Some(&prior)),
            LineageRecovery::Unresolvable
        );
        assert_eq!(
            reset.recover_against(Some(&target)),
            LineageRecovery::ContinueScopeSwap
        );
    }

    #[test]
    fn lineage_manifest_envelope_and_every_scope_invariant_fail_closed() {
        let init = LineageManifest::new(LineageManifestParts {
            source_id: SourceId::new("source").expect("source id"),
            scope: LineageScope::Init,
            from: None,
            target: SourceIdentity::fresh(),
        })
        .expect("init");
        assert_eq!(init.source_id().as_str(), "source");
        assert!(matches!(init.scope(), LineageScope::Init));
        assert!(init.from().is_none());
        init.target().validate().expect("target");
        let base = serde_json::to_value(&init).expect("manifest wire");
        for (field, value) in [
            ("schema_name", serde_json::json!("foreign.lineage")),
            ("schema_version", serde_json::json!(2)),
        ] {
            let mut wire = base.clone();
            wire[field] = value;
            assert!(serde_json::from_value::<LineageManifest>(wire).is_err());
        }

        let prior = SourceIdentity::fresh();
        assert!(
            LineageManifest::new(LineageManifestParts {
                source_id: SourceId::new("source").expect("source"),
                scope: LineageScope::Init,
                from: Some(prior.clone()),
                target: SourceIdentity::fresh(),
            })
            .is_err()
        );
        let preallocated = source_identity_with(
            super::super::SourceInstanceId::mint(),
            BTreeMap::from([(
                MeasurementId::new("preallocated").expect("measurement"),
                MeasurementIdentity::fresh(),
            )]),
        );
        assert!(
            LineageManifest::new(LineageManifestParts {
                source_id: SourceId::new("source").expect("source"),
                scope: LineageScope::Init,
                from: None,
                target: preallocated,
            })
            .is_err()
        );
        let preallocated = source_identity_with(
            super::super::SourceInstanceId::mint(),
            BTreeMap::from([(
                MeasurementId::new("preallocated-again").expect("measurement"),
                MeasurementIdentity::fresh(),
            )]),
        );
        assert!(
            LineageManifest::new(LineageManifestParts {
                source_id: SourceId::new("source").expect("source"),
                scope: LineageScope::SourceReset,
                from: None,
                target: preallocated,
            })
            .is_err()
        );

        let reset_id = MeasurementId::new("reset").expect("reset");
        assert!(
            LineageManifest::new(LineageManifestParts {
                source_id: SourceId::new("source").expect("source"),
                scope: LineageScope::MeasurementReset {
                    measurement_id: reset_id
                },
                from: None,
                target: SourceIdentity::fresh(),
            })
            .is_err()
        );
    }

    #[test]
    fn measurement_reset_rejects_foreign_source_missing_changed_added_and_reused_entries() {
        let source_instance = super::super::SourceInstanceId::mint();
        let reset_id = MeasurementId::new("reset").expect("reset");
        let sibling_id = MeasurementId::new("sibling").expect("sibling");
        let reset_prior = MeasurementIdentity::fresh();
        let sibling = MeasurementIdentity::fresh();
        let from = source_identity_with(
            source_instance.clone(),
            BTreeMap::from([
                (reset_id.clone(), reset_prior.clone()),
                (sibling_id.clone(), sibling.clone()),
            ]),
        );
        let manifest = |target: SourceIdentity| {
            LineageManifest::new(LineageManifestParts {
                source_id: SourceId::new("source").expect("source"),
                scope: LineageScope::MeasurementReset {
                    measurement_id: reset_id.clone(),
                },
                from: Some(from.clone()),
                target,
            })
        };
        assert!(
            manifest(source_identity_with(
                super::super::SourceInstanceId::mint(),
                BTreeMap::from([
                    (reset_id.clone(), MeasurementIdentity::fresh()),
                    (sibling_id.clone(), sibling.clone()),
                ]),
            ))
            .is_err()
        );
        assert!(
            manifest(source_identity_with(
                source_instance.clone(),
                BTreeMap::from([(sibling_id.clone(), sibling.clone())]),
            ))
            .is_err()
        );
        assert!(
            manifest(source_identity_with(
                source_instance.clone(),
                BTreeMap::from([(reset_id.clone(), MeasurementIdentity::fresh())]),
            ))
            .is_err()
        );
        assert!(
            manifest(source_identity_with(
                source_instance.clone(),
                BTreeMap::from([
                    (reset_id.clone(), MeasurementIdentity::fresh()),
                    (sibling_id.clone(), MeasurementIdentity::fresh()),
                ]),
            ))
            .is_err()
        );
        assert!(
            manifest(source_identity_with(
                source_instance.clone(),
                BTreeMap::from([
                    (reset_id.clone(), MeasurementIdentity::fresh()),
                    (sibling_id.clone(), sibling.clone()),
                    (
                        MeasurementId::new("extra").expect("extra"),
                        MeasurementIdentity::fresh()
                    ),
                ]),
            ))
            .is_err()
        );
        assert!(
            manifest(source_identity_with(
                source_instance,
                BTreeMap::from([(reset_id.clone(), reset_prior), (sibling_id, sibling)]),
            ))
            .is_err()
        );
    }
}

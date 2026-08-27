use super::*;

fn hash() -> String {
    "a".repeat(64)
}

#[test]
fn commit_manifest_preserves_disjoint_existing_and_new_measurement_lineage() {
    let existing_id = MeasurementId::new("existing").expect("existing id");
    let added_id = MeasurementId::new("added").expect("added id");
    let manifest = CommitManifest::new(CommitManifestParts {
        source_id: SourceId::new("source").expect("source id"),
        source_instance_id: SourceInstanceId::mint(),
        generation: 2,
        touched_measurement_instances: BTreeMap::from([(
            existing_id,
            MeasurementInstanceId::mint(),
        )]),
        identity_measurement_additions: BTreeMap::from([(added_id, MeasurementInstanceId::mint())]),
        cause: CommitCause::Acquisition,
        entries: vec![
            CommitOperation::Install {
                final_path: ManifestPath::new("source-state.json").expect("source final"),
                staged_path: ManifestPath::new("staged/source-state.json").expect("source staged"),
                expected_prior_sha256: Some(hash()),
                result_sha256: hash(),
            },
            CommitOperation::Install {
                final_path: ManifestPath::new("measurements/added/state.json").expect("final"),
                staged_path: ManifestPath::new("staged/added-state.json").expect("staged"),
                expected_prior_sha256: None,
                result_sha256: hash(),
            },
        ],
    })
    .expect("manifest");
    assert_eq!(manifest.generation(), 2);
    assert_eq!(manifest.touched_measurement_instances().len(), 1);
    assert_eq!(manifest.identity_measurement_additions().len(), 1);
    serde_json::from_value::<CommitManifest>(serde_json::to_value(manifest).expect("wire"))
        .expect("validated manifest wire");
}

#[test]
fn commit_manifest_refuses_path_escapes_and_overlapping_identity_maps() {
    for path in ["", "/state.json", "../state.json", "a//b", "a\\b"] {
        assert!(ManifestPath::new(path).is_err(), "{path}");
    }
    let measurement_id = MeasurementId::new("same").expect("measurement id");
    let result = CommitManifest::new(CommitManifestParts {
        source_id: SourceId::new("source").expect("source id"),
        source_instance_id: SourceInstanceId::mint(),
        generation: 1,
        touched_measurement_instances: BTreeMap::from([(
            measurement_id.clone(),
            MeasurementInstanceId::mint(),
        )]),
        identity_measurement_additions: BTreeMap::from([(
            measurement_id,
            MeasurementInstanceId::mint(),
        )]),
        cause: CommitCause::DeliveryResult,
        entries: vec![CommitOperation::Remove {
            final_path: ManifestPath::new("source-outbox/event.json").expect("final"),
            expected_prior_sha256: hash(),
        }],
    });
    assert!(result.is_err());

    assert!(
        CommitManifest::new(CommitManifestParts {
            source_id: SourceId::new("source").expect("source id"),
            source_instance_id: SourceInstanceId::mint(),
            generation: 2,
            touched_measurement_instances: BTreeMap::new(),
            identity_measurement_additions: BTreeMap::new(),
            cause: CommitCause::Acquisition,
            entries: vec![
                CommitOperation::Install {
                    final_path: ManifestPath::new("source-state.json").expect("state path"),
                    staged_path: ManifestPath::new("staged/source.json").expect("staged"),
                    expected_prior_sha256: Some(hash()),
                    result_sha256: hash(),
                },
                CommitOperation::Remove {
                    final_path: ManifestPath::new("commit.manifest").expect("reserved path"),
                    expected_prior_sha256: hash(),
                },
            ],
        })
        .is_err()
    );
}

#[test]
fn commit_manifest_refuses_a_measurement_identity_addition_without_its_state_install() {
    let result = CommitManifest::new(CommitManifestParts {
        source_id: SourceId::new("source").expect("source id"),
        source_instance_id: SourceInstanceId::mint(),
        generation: 1,
        touched_measurement_instances: BTreeMap::new(),
        identity_measurement_additions: BTreeMap::from([(
            MeasurementId::new("price").expect("measurement id"),
            MeasurementInstanceId::mint(),
        )]),
        cause: CommitCause::Acquisition,
        entries: vec![CommitOperation::Install {
            final_path: ManifestPath::new("source-state.json").expect("final"),
            staged_path: ManifestPath::new("staged/source-state.json").expect("staged"),
            expected_prior_sha256: Some(hash()),
            result_sha256: hash(),
        }],
    });
    assert!(result.is_err());
}

#[test]
fn commit_recovery_requires_exact_lineage_maps_and_an_adjacent_generation() {
    let existing_id = MeasurementId::new("existing").expect("existing id");
    let added_id = MeasurementId::new("added").expect("added id");
    let existing = MeasurementInstanceId::mint();
    let added = MeasurementInstanceId::mint();
    let source_instance_id = SourceInstanceId::mint();
    let source_identity = source_identity_with(
        source_instance_id.clone(),
        BTreeMap::from([(existing_id.clone(), existing.clone())]),
    );
    let manifest = CommitManifest::new(CommitManifestParts {
        source_id: SourceId::new("source").expect("source id"),
        source_instance_id,
        generation: 3,
        touched_measurement_instances: BTreeMap::from([(existing_id, existing)]),
        identity_measurement_additions: BTreeMap::from([(added_id.clone(), added)]),
        cause: CommitCause::Acquisition,
        entries: vec![
            CommitOperation::Install {
                final_path: ManifestPath::new("source-state.json").expect("final"),
                staged_path: ManifestPath::new("staged/source-state.json").expect("staged"),
                expected_prior_sha256: Some(hash()),
                result_sha256: hash(),
            },
            CommitOperation::Install {
                final_path: ManifestPath::new(format!(
                    "measurements/{}/state.json",
                    added_id.as_str()
                ))
                .expect("measurement final"),
                staged_path: ManifestPath::new("staged/added-state.json").expect("staged"),
                expected_prior_sha256: None,
                result_sha256: hash(),
            },
        ],
    })
    .expect("manifest");
    assert_eq!(
        manifest.recover_against(&source_identity, 2),
        CommitRecovery::Apply
    );
    assert_eq!(
        manifest.recover_against(&source_identity, 3),
        CommitRecovery::Apply
    );
    assert_eq!(
        manifest.recover_against(&source_identity, 1),
        CommitRecovery::Unresolvable
    );
    let foreign_identity = source_identity_with(
        SourceInstanceId::mint(),
        BTreeMap::from([(
            MeasurementId::new("existing").expect("existing id"),
            MeasurementInstanceId::mint(),
        )]),
    );
    assert_eq!(
        manifest.recover_against(&foreign_identity, 2),
        CommitRecovery::Unresolvable
    );
    let touched_missing =
        source_identity_with(manifest.source_instance_id().clone(), BTreeMap::new());
    assert_eq!(
        manifest.recover_against(&touched_missing, 2),
        CommitRecovery::Unresolvable
    );
    let existing_id = MeasurementId::new("existing").expect("existing");
    let added_id = MeasurementId::new("added").expect("added");
    let touched_instance = manifest
        .touched_measurement_instances()
        .get(&existing_id)
        .expect("touched")
        .clone();
    let addition_conflict = source_identity_with(
        manifest.source_instance_id().clone(),
        BTreeMap::from([
            (existing_id, touched_instance),
            (added_id, MeasurementInstanceId::mint()),
        ]),
    );
    assert_eq!(
        manifest.recover_against(&addition_conflict, 2),
        CommitRecovery::Unresolvable
    );
}

fn source_identity_with(
    source_instance_id: SourceInstanceId,
    measurements: BTreeMap<MeasurementId, MeasurementInstanceId>,
) -> super::super::SourceIdentity {
    let measurement_entries = measurements
        .into_iter()
        .map(|(id, instance_id)| {
            (
                id,
                serde_json::json!({
                    "measurement_instance_id": instance_id,
                    "reset_count": 0,
                }),
            )
        })
        .collect::<BTreeMap<_, _>>();
    serde_json::from_value(serde_json::json!({
        "schema_name": "ffhn.source_identity",
        "schema_version": 1,
        "source_instance_id": source_instance_id,
        "measurements": measurement_entries,
    }))
    .expect("source identity")
}

fn simple_manifest() -> CommitManifest {
    CommitManifest::new(CommitManifestParts {
        source_id: SourceId::new("source").expect("source id"),
        source_instance_id: SourceInstanceId::mint(),
        generation: 2,
        touched_measurement_instances: BTreeMap::new(),
        identity_measurement_additions: BTreeMap::new(),
        cause: CommitCause::Acquisition,
        entries: vec![CommitOperation::Install {
            final_path: ManifestPath::new("source-state.json").expect("final"),
            staged_path: ManifestPath::new("staged/source-state.json").expect("staged"),
            expected_prior_sha256: Some(hash()),
            result_sha256: hash(),
        }],
    })
    .expect("manifest")
}

#[test]
fn manifest_paths_operations_and_envelope_reject_every_closed_layout_crossing() {
    for path in [".", "..", "a/./b", "a/../b", "a/", "staged//a"] {
        assert!(ManifestPath::new(path).is_err(), "{path}");
    }
    let path = ManifestPath::new("staged/value.json").expect("path");
    assert_eq!(path.as_str(), "staged/value.json");
    assert!(path.is_staged());

    for final_path in [
        "source-state.json".to_owned(),
        format!("source-outbox/{}--route.json", "a".repeat(64)),
        format!("source-outbox/{}--1.json", "0".repeat(64)),
        format!("dead-letters/{}--route.json", "b".repeat(64)),
        "measurements/price/state.json".to_owned(),
        format!("measurements/price/outbox/{}--route.json", "c".repeat(64)),
        format!(
            "measurements/price/dead-letters/{}--route.json",
            "d".repeat(64)
        ),
    ] {
        super::path_validation::validate_final_path(
            &ManifestPath::new(final_path).expect("manifest path"),
        )
        .expect("valid final path");
    }
    for final_path in [
        "commit.manifest",
        "source-outbox/no-extension",
        "source-outbox/short--route.json",
        "source-outbox/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.json",
        "source-outbox/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa--bad--route.json",
        "source-outbox/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA--route.json",
        "source-outbox/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa--UPPER.json",
        "measurements/UPPER/state.json",
        "measurements/UPPER/outbox/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa--route.json",
        "measurements/price/foreign/value.json",
    ] {
        assert!(
            super::path_validation::validate_final_path(
                &ManifestPath::new(final_path).expect("structural path"),
            )
            .is_err(),
            "{final_path}"
        );
    }

    let final_path = ManifestPath::new("source-state.json").expect("final");
    let staged_path = ManifestPath::new("staged/source-state.json").expect("staged");
    let valid = CommitOperation::Install {
        final_path: final_path.clone(),
        staged_path: staged_path.clone(),
        expected_prior_sha256: None,
        result_sha256: hash(),
    };
    valid.validate().expect("valid install");
    for operation in [
        CommitOperation::Install {
            final_path: staged_path.clone(),
            staged_path: staged_path.clone(),
            expected_prior_sha256: None,
            result_sha256: hash(),
        },
        CommitOperation::Install {
            final_path: ManifestPath::new("staged/final.json").expect("final"),
            staged_path: ManifestPath::new("staged/other.json").expect("staged"),
            expected_prior_sha256: None,
            result_sha256: hash(),
        },
        CommitOperation::Install {
            final_path: final_path.clone(),
            staged_path: ManifestPath::new("not-staged.json").expect("staged"),
            expected_prior_sha256: None,
            result_sha256: hash(),
        },
        CommitOperation::Install {
            final_path: final_path.clone(),
            staged_path: staged_path.clone(),
            expected_prior_sha256: Some("invalid".to_owned()),
            result_sha256: hash(),
        },
        CommitOperation::Install {
            final_path: final_path.clone(),
            staged_path: staged_path.clone(),
            expected_prior_sha256: None,
            result_sha256: "invalid".to_owned(),
        },
        CommitOperation::Remove {
            final_path,
            expected_prior_sha256: "invalid".to_owned(),
        },
    ] {
        assert!(operation.validate().is_err());
    }

    let manifest = simple_manifest();
    assert_eq!(manifest.source_id().as_str(), "source");
    manifest
        .source_instance_id()
        .validate()
        .expect("source instance");
    assert_eq!(manifest.generation(), 2);
    assert!(manifest.touched_measurement_instances().is_empty());
    assert!(manifest.identity_measurement_additions().is_empty());
    assert_eq!(manifest.entries().len(), 1);
    let base = serde_json::to_value(&manifest).expect("manifest wire");
    for (field, value) in [
        ("schema_name", serde_json::json!("foreign.manifest")),
        ("schema_version", serde_json::json!(2)),
        ("generation", serde_json::json!(0)),
        ("entries", serde_json::json!([])),
    ] {
        let mut wire = base.clone();
        wire[field] = value;
        assert!(serde_json::from_value::<CommitManifest>(wire).is_err());
    }
}

#[test]
fn manifest_level_operation_counts_causes_and_recovery_maps_fail_closed() {
    let state_install = || CommitOperation::Install {
        final_path: ManifestPath::new("source-state.json").expect("final"),
        staged_path: ManifestPath::new("staged/source-state.json").expect("staged"),
        expected_prior_sha256: Some(hash()),
        result_sha256: hash(),
    };
    let added = MeasurementId::new("added").expect("measurement");
    assert!(
        CommitManifest::new(CommitManifestParts {
            source_id: SourceId::new("source").expect("source"),
            source_instance_id: SourceInstanceId::mint(),
            generation: 2,
            touched_measurement_instances: BTreeMap::new(),
            identity_measurement_additions: BTreeMap::from([(
                added.clone(),
                MeasurementInstanceId::mint()
            )]),
            cause: CommitCause::DeliveryResult,
            entries: vec![
                state_install(),
                CommitOperation::Install {
                    final_path: ManifestPath::new("measurements/added/state.json").expect("final"),
                    staged_path: ManifestPath::new("staged/added.json").expect("staged"),
                    expected_prior_sha256: None,
                    result_sha256: hash(),
                }
            ],
        })
        .is_err()
    );
    for entries in [
        vec![CommitOperation::Install {
            final_path: ManifestPath::new(format!("source-outbox/{}--route.json", "a".repeat(64)))
                .expect("final"),
            staged_path: ManifestPath::new("staged/record.json").expect("staged"),
            expected_prior_sha256: None,
            result_sha256: hash(),
        }],
        vec![state_install(), state_install()],
        vec![
            state_install(),
            CommitOperation::Remove {
                final_path: ManifestPath::new(format!(
                    "source-outbox/{}--route.json",
                    "a".repeat(64)
                ))
                .expect("final"),
                expected_prior_sha256: hash(),
            },
        ],
    ] {
        assert!(
            CommitManifest::new(CommitManifestParts {
                source_id: SourceId::new("source").expect("source"),
                source_instance_id: SourceInstanceId::mint(),
                generation: 2,
                touched_measurement_instances: BTreeMap::new(),
                identity_measurement_additions: BTreeMap::new(),
                cause: CommitCause::Acquisition,
                entries,
            })
            .is_err()
        );
    }

    assert!(
        CommitManifest::new(CommitManifestParts {
            source_id: SourceId::new("source").expect("source"),
            source_instance_id: SourceInstanceId::mint(),
            generation: 2,
            touched_measurement_instances: BTreeMap::new(),
            identity_measurement_additions: BTreeMap::new(),
            cause: CommitCause::DeliveryResult,
            entries: vec![
                state_install(),
                CommitOperation::Install {
                    final_path: ManifestPath::new("measurements/price/state.json").expect("final"),
                    staged_path: ManifestPath::new("staged/source-state.json").expect("staged"),
                    expected_prior_sha256: None,
                    result_sha256: hash(),
                },
            ],
        })
        .is_err()
    );

    require_sha256("digest", &"0".repeat(64)).expect("digit digest");
    assert!(require_sha256("digest", &"A".repeat(64)).is_err());

    assert!(
        CommitManifest::new(CommitManifestParts {
            source_id: SourceId::new("source").expect("source"),
            source_instance_id: SourceInstanceId::mint(),
            generation: 2,
            touched_measurement_instances: BTreeMap::new(),
            identity_measurement_additions: BTreeMap::new(),
            cause: CommitCause::DeliveryResult,
            entries: vec![
                state_install(),
                CommitOperation::Remove {
                    final_path: ManifestPath::new("source-state.json").expect("final"),
                    expected_prior_sha256: hash(),
                },
            ],
        })
        .is_err()
    );

    let manifest = simple_manifest();
    let identity = source_identity_with(manifest.source_instance_id().clone(), BTreeMap::new());
    assert_eq!(
        manifest.recover_against(&identity, u64::MAX),
        CommitRecovery::Unresolvable
    );
}

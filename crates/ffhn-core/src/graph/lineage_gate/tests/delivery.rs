use super::*;

#[test]
fn measurement_delivery_gate_distinguishes_source_measurement_and_scope_mismatches() {
    let (_temporary, source) = source_with_storage();
    let mut identity = ready_source(&source);
    let measurement_id = MeasurementId::new("price").expect("measurement");
    let measurement_identity = MeasurementIdentity::fresh();
    identity
        .register_measurement(measurement_id.clone(), measurement_identity.clone())
        .expect("identity");
    source.write_identity(&identity).expect("identity");
    let storage = source.open_storage().expect("storage");
    storage
        .write_measurement_state(
            &measurement_id,
            &MeasurementState::fresh(
                identity.source_instance_id().clone(),
                measurement_identity.measurement_instance_id().clone(),
            ),
        )
        .expect("state");
    fs::create_dir_all(source.paths().measurement_outbox_dir(&measurement_id)).expect("outbox");
    let write_record = |record: &DeliveryRecord| {
        fs::write(
            source
                .paths()
                .measurement_outbox_dir(&measurement_id)
                .join(record.storage_file_name()),
            format!(
                "{}\n",
                crate::stable_json::stable_json(record).expect("record")
            ),
        )
        .expect("record write");
    };

    let valid = measurement_record(
        identity.source_instance_id().clone(),
        measurement_id.clone(),
        measurement_identity.measurement_instance_id().clone(),
    );
    write_record(&valid);
    assert!(matches!(
        source
            .inspect_lineage([])
            .expect("inspection")
            .measurement(&measurement_id),
        Some(MeasurementLineage::Ready(_))
    ));
    fs::remove_file(
        source
            .paths()
            .measurement_outbox_dir(&measurement_id)
            .join(valid.storage_file_name()),
    )
    .expect("remove");

    let wrong_source = measurement_record_for(
        SourceId::new("other").expect("source"),
        identity.source_instance_id().clone(),
        measurement_id.clone(),
        measurement_identity.measurement_instance_id().clone(),
    );
    write_record(&wrong_source);
    assert_eq!(
        source
            .inspect_lineage([])
            .expect("inspection")
            .measurement(&measurement_id),
        Some(&MeasurementLineage::Held(
            MeasurementLineageHold::ArtifactUnreadable
        ))
    );
    fs::remove_file(
        source
            .paths()
            .measurement_outbox_dir(&measurement_id)
            .join(wrong_source.storage_file_name()),
    )
    .expect("remove");

    let wrong_source_instance = measurement_record(
        SourceInstanceId::mint(),
        measurement_id.clone(),
        measurement_identity.measurement_instance_id().clone(),
    );
    write_record(&wrong_source_instance);
    assert_eq!(
        source
            .inspect_lineage([])
            .expect("inspection")
            .measurement(&measurement_id),
        Some(&MeasurementLineage::Held(
            MeasurementLineageHold::SourceInstanceMismatch
        ))
    );
    fs::remove_file(
        source
            .paths()
            .measurement_outbox_dir(&measurement_id)
            .join(wrong_source_instance.storage_file_name()),
    )
    .expect("remove");
    let source_scoped = source_record(identity.source_instance_id().clone());
    write_record(&source_scoped);
    assert_eq!(
        source
            .inspect_lineage([])
            .expect("inspection")
            .measurement(&measurement_id),
        Some(&MeasurementLineage::Held(
            MeasurementLineageHold::ArtifactUnreadable
        ))
    );
    fs::remove_file(
        source
            .paths()
            .measurement_outbox_dir(&measurement_id)
            .join(source_scoped.storage_file_name()),
    )
    .expect("remove");
    let wrong_measurement = measurement_record(
        identity.source_instance_id().clone(),
        MeasurementId::new("other").expect("measurement"),
        MeasurementInstanceId::mint(),
    );
    write_record(&wrong_measurement);
    assert_eq!(
        source
            .inspect_lineage([])
            .expect("inspection")
            .measurement(&measurement_id),
        Some(&MeasurementLineage::Held(
            MeasurementLineageHold::ArtifactUnreadable
        ))
    );
}

#[test]
fn unreadable_source_dead_letter_is_a_source_scope_refusal() {
    let (_temporary, source) = source_with_storage();
    ready_source(&source);
    fs::create_dir_all(source.paths().source_dead_letters_dir()).expect("letters");
    fs::write(
        source.paths().source_dead_letters_dir().join("bad.json"),
        "not-json",
    )
    .expect("letter");
    assert_eq!(
        source.inspect_lineage([]).expect("inspection").source(),
        &SourceLineage::Refused(SourceLineageRefusal::DeliveryArtifactUnreadable)
    );
}

#[test]
fn unreadable_source_pending_record_is_a_source_scope_refusal() {
    let (_temporary, source) = source_with_storage();
    ready_source(&source);
    fs::create_dir_all(source.paths().source_outbox_dir()).expect("outbox");
    fs::write(
        source.paths().source_outbox_dir().join("bad.json"),
        "not-json",
    )
    .expect("record");
    assert_eq!(
        source.inspect_lineage([]).expect("inspection").source(),
        &SourceLineage::Refused(SourceLineageRefusal::DeliveryArtifactUnreadable)
    );
}

#[test]
fn source_delivery_gate_rejects_wrong_source_and_source_instance_stamps() {
    let (_temporary, source) = source_with_storage();
    let identity = ready_source(&source);
    fs::create_dir_all(source.paths().source_outbox_dir()).expect("outbox");
    let write_record = |record: &DeliveryRecord| {
        fs::write(
            source
                .paths()
                .source_outbox_dir()
                .join(record.storage_file_name()),
            format!(
                "{}\n",
                crate::stable_json::stable_json(record).expect("record")
            ),
        )
        .expect("record");
    };
    let wrong_source = measurement_record_for(
        SourceId::new("other").expect("source"),
        identity.source_instance_id().clone(),
        MeasurementId::new("measurement").expect("measurement"),
        MeasurementInstanceId::mint(),
    );
    write_record(&wrong_source);
    assert_eq!(
        source.inspect_lineage([]).expect("inspection").source(),
        &SourceLineage::Refused(SourceLineageRefusal::DeliveryArtifactUnreadable)
    );
    fs::remove_file(
        source
            .paths()
            .source_outbox_dir()
            .join(wrong_source.storage_file_name()),
    )
    .expect("remove");
    let wrong_instance = source_record(SourceInstanceId::mint());
    write_record(&wrong_instance);
    assert_eq!(
        source.inspect_lineage([]).expect("inspection").source(),
        &SourceLineage::Refused(SourceLineageRefusal::DeliveryArtifactUnreadable)
    );
}

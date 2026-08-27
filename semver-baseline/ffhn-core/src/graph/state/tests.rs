use super::*;

#[test]
fn state_timestamp_and_http_url_helpers_enforce_each_independent_boundary() {
    assert!(require_timestamp("time", "2026-08-25T00:00:00Z").is_ok());
    for invalid in [
        "2026-08-25T00:00:00+00:00",
        "2026-08-25T00:00:00.000Z",
        "not-a-time",
    ] {
        assert!(require_timestamp("time", invalid).is_err(), "{invalid}");
    }

    for valid in ["http://example.test/path", "https://example.test/path"] {
        assert!(valid_http_url(&url::Url::parse(valid).expect("valid URL")));
    }
    for invalid in [
        "file:///tmp/value",
        "https://user@example.test/path",
        "https://user:password@example.test/path",
    ] {
        assert!(!valid_http_url(
            &url::Url::parse(invalid).expect("parsed URL")
        ));
    }
}
use crate::graph::{
    ExtractionFailureReason, GraphIntegrationFaultCode, MeasurementInstanceId, MeasurementState,
    SourceFetchFailure, SourceFetchFailureKind,
};

#[test]
fn source_and_measurement_state_keep_health_and_integration_faults_at_their_owned_scopes() {
    let source_instance = SourceInstanceId::mint();
    let mut source = SourceState::fresh(source_instance.clone());
    assert!(
        source
            .apply_acquisition_failure(
                SourceFetchFailure {
                    kind: SourceFetchFailureKind::HttpStatus,
                    status: Some(503),
                    raw_platform_error: None,
                },
                "2026-08-25T00:00:00Z",
                1,
            )
            .expect("source failure")
    );
    assert!(
        source
            .apply_source_integration_fault(
                GraphIntegrationFaultCode::SecretUnavailable,
                "2026-08-25T00:01:00Z".to_owned(),
            )
            .expect("source fault")
    );
    assert!(
        source
            .apply_source_integration_fault(
                GraphIntegrationFaultCode::HtmlcutInternalError,
                "2026-08-25T00:01:00Z".to_owned(),
            )
            .is_err()
    );
    source
        .apply_acquisition_failure(
            SourceFetchFailure {
                kind: SourceFetchFailureKind::InvalidUtf8,
                status: None,
                raw_platform_error: None,
            },
            "2026-08-25T00:02:00Z",
            2,
        )
        .expect("non-integration acquisition outcome");
    assert!(source.integration_fault_episode().is_none());

    let mut measurement = MeasurementState::fresh(source_instance, MeasurementInstanceId::mint());
    assert!(
        measurement
            .apply_extraction_failure(
                ExtractionFailureReason::JsonMalformed,
                "2026-08-25T00:00:00Z",
                1,
            )
            .expect("extraction failure")
    );
    assert!(
        measurement
            .apply_measurement_integration_fault(
                GraphIntegrationFaultCode::HtmlcutInternalError,
                "2026-08-25T00:01:00Z".to_owned(),
            )
            .expect("measurement fault")
    );
    assert!(
        measurement
            .apply_measurement_integration_fault(
                GraphIntegrationFaultCode::SecretUnavailable,
                "2026-08-25T00:01:00Z".to_owned(),
            )
            .is_err()
    );
    measurement
        .apply_extraction_failure(
            ExtractionFailureReason::ValueUnparseable,
            "2026-08-25T00:02:00Z",
            2,
        )
        .expect("non-integration extraction outcome");
    assert!(measurement.integration_fault_episode().is_none());
}

#[test]
fn source_cycle_schedule_is_an_ordered_utc_fact() {
    let source = SourceState::fresh(SourceInstanceId::mint());
    assert!(
        source
            .with_cycle_schedule(
                "2026-08-25T00:00:00Z".to_owned(),
                "2026-08-25T00:00:00Z".to_owned(),
            )
            .is_err()
    );
    source
        .with_cycle_schedule(
            "2026-08-25T00:00:00Z".to_owned(),
            "2026-08-25T00:00:01Z".to_owned(),
        )
        .expect("ordered schedule");
}

#[test]
fn source_representation_provenance_is_exclusively_http_file_or_absent() {
    let source_url = url::Url::parse("https://example.test/source").expect("source URL");
    let redirected = url::Url::parse("https://cdn.example.test/final").expect("effective URL");
    let state = SourceState::fresh(SourceInstanceId::mint())
        .with_representation_facts(
            None,
            Some(source_url.clone()),
            Some(redirected),
            None,
            "a".repeat(64),
        )
        .expect("HTTP provenance");
    let wire = serde_json::to_value(&state).expect("HTTP state wire");
    assert_eq!(wire["source_url"], source_url.as_str());
    assert!(wire.get("file_content_sha256").is_none());

    let direct_validators = HttpValidators {
        issued_url: source_url.clone(),
        etag: Some("\"v1\"".to_owned()),
        last_modified: None,
    };
    assert!(
        SourceState::fresh(SourceInstanceId::mint())
            .with_representation_facts(
                Some(direct_validators),
                Some(source_url),
                Some(url::Url::parse("https://cdn.example.test/final").expect("effective URL")),
                None,
                "a".repeat(64),
            )
            .is_err()
    );

    let mut crossed = serde_json::to_value(
        SourceState::fresh(SourceInstanceId::mint())
            .with_representation_facts(None, None, None, Some("b".repeat(64)), "a".repeat(64))
            .expect("file provenance"),
    )
    .expect("file state wire");
    crossed["source_url"] = serde_json::json!("https://example.test/source");
    crossed["effective_base_url"] = serde_json::json!("https://example.test/source");
    assert!(serde_json::from_value::<SourceState>(crossed).is_err());

    let valid_validators = serde_json::json!({
        "issued_url": "https://example.test/source",
        "etag": "\"v1\""
    });
    let mut validators_without_source =
        serde_json::to_value(SourceState::fresh(SourceInstanceId::mint())).expect("state wire");
    validators_without_source["validators"] = valid_validators.clone();
    assert!(serde_json::from_value::<SourceState>(validators_without_source).is_err());

    let mut invalid_effective =
        serde_json::to_value(SourceState::fresh(SourceInstanceId::mint())).expect("state wire");
    invalid_effective["source_url"] = serde_json::json!("https://example.test/source");
    invalid_effective["effective_base_url"] = serde_json::json!("ftp://example.test/source");
    invalid_effective["last_source_representation_digest"] = serde_json::json!("a".repeat(64));
    assert!(serde_json::from_value::<SourceState>(invalid_effective).is_err());
    let mut invalid_source =
        serde_json::to_value(SourceState::fresh(SourceInstanceId::mint())).expect("state wire");
    invalid_source["source_url"] = serde_json::json!("ftp://example.test/source");
    invalid_source["effective_base_url"] = serde_json::json!("https://example.test/source");
    invalid_source["last_source_representation_digest"] = serde_json::json!("a".repeat(64));
    assert!(serde_json::from_value::<SourceState>(invalid_source).is_err());

    let mut wrong_issuer = serde_json::to_value(
        SourceState::fresh(SourceInstanceId::mint())
            .with_representation_facts(
                None,
                Some(url::Url::parse("https://example.test/source").expect("URL")),
                Some(url::Url::parse("https://example.test/source").expect("URL")),
                None,
                "a".repeat(64),
            )
            .expect("HTTP state"),
    )
    .expect("state wire");
    wrong_issuer["validators"] = serde_json::json!({
        "issued_url": "https://other.test/source",
        "etag": "\"v1\""
    });
    assert!(serde_json::from_value::<SourceState>(wrong_issuer).is_err());

    let mut file_with_validators = serde_json::to_value(
        SourceState::fresh(SourceInstanceId::mint())
            .with_representation_facts(None, None, None, Some("b".repeat(64)), "a".repeat(64))
            .expect("file state"),
    )
    .expect("state wire");
    file_with_validators["validators"] = valid_validators;
    assert!(serde_json::from_value::<SourceState>(file_with_validators).is_err());
}

#[test]
fn source_state_accessors_successors_and_episode_transitions_are_complete() {
    let source_instance = SourceInstanceId::mint();
    let mut state = SourceState::fresh(source_instance.clone());
    assert_eq!(state.source_instance_id(), &source_instance);
    assert_eq!(state.generation(), 1);
    assert_eq!(state.source_episode_seq(), 0);
    assert_eq!(state.source_integration_fault_code(), None);
    assert_eq!(state.source_health(), &SourceAcquisitionHealth::healthy());
    assert!(state.integration_fault_episode().is_none());
    assert!(state.outbox_overflow().is_empty());
    assert!(state.validators().is_none());
    assert!(state.source_url().is_none());
    assert!(state.effective_base_url().is_none());
    assert!(state.file_content_sha256().is_none());
    assert!(state.last_source_representation_digest().is_none());
    assert!(state.last_cycle_completed_utc().is_none());
    assert!(state.next_due_utc().is_none());
    assert!(!state.matches_file_representation(&"a".repeat(64), &"b".repeat(64)));

    state
        .apply_acquisition_failure(
            SourceFetchFailure {
                kind: SourceFetchFailureKind::HttpStatus,
                status: Some(500),
                raw_platform_error: None,
            },
            "2026-08-25T00:00:00Z",
            2,
        )
        .expect("first failure");
    assert_eq!(state.source_episode_seq(), 1);
    state
        .apply_acquisition_failure(
            SourceFetchFailure {
                kind: SourceFetchFailureKind::HttpStatus,
                status: Some(500),
                raw_platform_error: None,
            },
            "2026-08-25T00:01:00Z",
            2,
        )
        .expect("same episode");
    assert_eq!(state.source_episode_seq(), 1);
    assert!(
        state
            .apply_source_integration_fault(
                GraphIntegrationFaultCode::SecretUnavailable,
                "2026-08-25T00:02:00Z".to_owned(),
            )
            .expect("fault")
    );
    assert!(
        !state
            .apply_source_integration_fault(
                GraphIntegrationFaultCode::SecretUnavailable,
                "2026-08-25T00:03:00Z".to_owned(),
            )
            .expect("same fault")
    );
    assert_eq!(
        state.source_integration_fault_code(),
        Some(GraphIntegrationFaultCode::SecretUnavailable)
    );
    state.clear_transient_episodes();
    assert_eq!(state.source_health(), &SourceAcquisitionHealth::healthy());
    assert!(state.integration_fault_episode().is_none());

    let file = state
        .with_representation_facts(None, None, None, Some("b".repeat(64)), "a".repeat(64))
        .expect("file facts");
    assert_eq!(file.file_content_sha256(), Some("b".repeat(64).as_str()));
    assert_eq!(
        file.last_source_representation_digest(),
        Some("a".repeat(64).as_str())
    );
    assert!(file.matches_file_representation(&"a".repeat(64), &"b".repeat(64)));
    assert!(!file.matches_file_representation(&"c".repeat(64), &"b".repeat(64)));
    assert!(!file.matches_file_representation(&"a".repeat(64), &"c".repeat(64)));
    assert!(
        state
            .with_representation_facts(None, None, None, Some("invalid".to_owned()), "a".repeat(64))
            .is_err()
    );
    assert!(
        state
            .with_representation_facts(None, None, None, None, "invalid".to_owned())
            .is_err()
    );

    let scheduled = file
        .with_cycle_schedule(
            "2026-08-25T00:00:00Z".to_owned(),
            "2026-08-25T00:00:01Z".to_owned(),
        )
        .expect("schedule");
    assert_eq!(
        scheduled.last_cycle_completed_utc(),
        Some("2026-08-25T00:00:00Z")
    );
    assert_eq!(scheduled.next_due_utc(), Some("2026-08-25T00:00:01Z"));
    let successor = scheduled.next_generation().expect("successor");
    assert_eq!(successor.generation(), 2);
    assert_eq!(
        successor.file_content_sha256(),
        scheduled.file_content_sha256()
    );
}

#[test]
fn source_state_rejects_every_crossed_envelope_provenance_and_schedule_shape() {
    let fresh = SourceState::fresh(SourceInstanceId::mint());
    let base = serde_json::to_value(&fresh).expect("fresh wire");
    for (field, value) in [
        ("schema_name", serde_json::json!("foreign.source_state")),
        ("schema_version", serde_json::json!(2)),
        ("generation", serde_json::json!(0)),
        ("file_content_sha256", serde_json::json!("invalid")),
        (
            "last_source_representation_digest",
            serde_json::json!("invalid"),
        ),
        ("source_url", serde_json::json!("ftp://example.test/a")),
        (
            "last_cycle_completed_utc",
            serde_json::json!("2026-08-25T00:00:00Z"),
        ),
    ] {
        let mut wire = base.clone();
        wire[field] = value;
        assert!(
            serde_json::from_value::<SourceState>(wire).is_err(),
            "{field}"
        );
    }

    let mut scheduled = serde_json::to_value(
        fresh
            .with_cycle_schedule(
                "2026-08-25T00:00:00Z".to_owned(),
                "2026-08-25T00:00:01Z".to_owned(),
            )
            .expect("schedule"),
    )
    .expect("scheduled wire");
    scheduled["next_due_utc"] = serde_json::json!("2026-08-25T00:00:00Z");
    assert!(serde_json::from_value::<SourceState>(scheduled).is_err());

    let source_url = url::Url::parse("https://example.test/source").expect("URL");
    let validators = HttpValidators {
        issued_url: source_url.clone(),
        etag: Some("\"v1\"".to_owned()),
        last_modified: None,
    };
    let http = fresh
        .with_representation_facts(
            Some(validators),
            Some(source_url.clone()),
            Some(source_url.clone()),
            None,
            "a".repeat(64),
        )
        .expect("HTTP facts");
    assert_eq!(http.source_url(), Some(&source_url));
    assert_eq!(http.effective_base_url(), Some(&source_url));
    assert!(http.validators().is_some());
    let mut foreign_fault = serde_json::to_value(&http).expect("HTTP wire");
    foreign_fault["integration_fault_episode"] = serde_json::json!({
        "code": "htmlcut_internal_error",
        "first_seen_at_utc": "2026-08-25T00:00:00Z"
    });
    assert!(serde_json::from_value::<SourceState>(foreign_fault).is_err());

    let mut max_generation = serde_json::to_value(&fresh).expect("wire");
    max_generation["generation"] = serde_json::json!(u64::MAX);
    let max_generation: SourceState =
        serde_json::from_value(max_generation).expect("max generation");
    assert!(max_generation.next_generation().is_err());
    let mut max_episode = serde_json::to_value(&fresh).expect("wire");
    max_episode["source_episode_seq"] = serde_json::json!(u64::MAX);
    let mut max_episode: SourceState = serde_json::from_value(max_episode).expect("max episode");
    assert!(
        max_episode
            .apply_acquisition_failure(
                SourceFetchFailure {
                    kind: SourceFetchFailureKind::InvalidUtf8,
                    status: None,
                    raw_platform_error: None,
                },
                "2026-08-25T00:00:00Z",
                1,
            )
            .is_err()
    );
    assert!(valid_http_url(
        &url::Url::parse("http://example.test").expect("HTTP")
    ));
    validate_sha256("digest", &"0".repeat(64)).expect("digit digest");
    assert!(validate_sha256("digest", &"A".repeat(64)).is_err());
    assert!(valid_http_url(
        &url::Url::parse("https://example.test").expect("HTTPS")
    ));
    assert!(!valid_http_url(
        &url::Url::parse("ftp://example.test").expect("FTP")
    ));
    assert!(!valid_http_url(
        &url::Url::parse("https://user@example.test").expect("userinfo")
    ));
    assert!(!valid_http_url(
        &url::Url::parse("https://user:pass@example.test").expect("password")
    ));
    for (field, value) in [
        (
            "last_cycle_completed_utc",
            serde_json::json!("2026-08-25T00:00:00Z"),
        ),
        ("next_due_utc", serde_json::json!("2026-08-25T00:00:01Z")),
    ] {
        let mut wire = serde_json::to_value(&fresh).expect("fresh wire");
        wire[field] = value;
        assert!(serde_json::from_value::<SourceState>(wire).is_err());
    }
}

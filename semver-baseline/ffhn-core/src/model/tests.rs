use super::delivery::validate_routes;
use super::observation::{
    decode_json_pointer_token, json_scalar_value, normalize_decimal_input,
    normalize_grouped_decimal, normalized_raw_token, parse_canonical_value, parse_datetime,
    parse_json_array_index, parse_offset, select_json_pointer_child, select_json_scalar_token,
};
use super::state::is_sha256;
use super::target::{
    default_follow_redirects, default_max_bytes, default_timeout_ms,
    permanent_code_for_htmlcut_error, require_text, validate_json_pointer, validate_max_bytes,
    validate_type_params,
};
use super::*;

fn target(declared_type: &str, type_params: &str) -> TargetDocument {
    let source_path = crate::test_support::absolute_file_path("source.json");
    let document: TargetDocument = toml::from_str(&format!(
            "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"{declared_type}\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n{type_params}\n"
        ))
        .expect("target toml");
    document.validate().expect("valid target");
    document
}

fn mutate_target(
    document: &TargetDocument,
    mutate: impl FnOnce(&mut serde_json::Value),
) -> TargetDocument {
    let mut wire = serde_json::to_value(document).expect("target JSON");
    mutate(&mut wire);
    serde_json::from_value(wire).expect("target JSON remains structurally valid")
}

fn http_target() -> TargetDocument {
    let document: TargetDocument = toml::from_str(
        "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"http\"\ndisplay_name = \"HTTP\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"http\"\nsource_url = \"https://example.test/value\"\n\n[fetch]\nengine = \"http\"\nmethod = \"GET\"\ntimeout_ms = 1000\nmax_bytes = 1024\nuser_agent = \"ffhn-test\"\nfollow_redirects = false\naccept = \"application/json\"\n\n[fetch.headers]\nX-Test = \"yes\"\n\n[projection]\nkind = \"json_pointer\"\npointer = \"\"\n",
    )
    .expect("HTTP target TOML");
    document.validate().expect("valid HTTP target");
    document
}

fn html_target(
    projection_kind: &str,
    selector: &str,
    attribute_name: Option<&str>,
    declared_type: &str,
    type_params: &str,
) -> TargetDocument {
    let source_path = crate::test_support::absolute_file_path("source.html");
    let attribute = attribute_name
        .map(|name| format!("name = {name:?}\n"))
        .unwrap_or_default();
    let document: TargetDocument = toml::from_str(&format!(
        "schema_name = \"ffhn.target\"\nschema_version = 9\ntarget_id = \"html\"\ndisplay_name = \"HTML\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"{declared_type}\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"{projection_kind}\"\n{attribute}\n[projection.selection.strategy]\nkind = \"css_selector\"\nselector = {selector:?}\n\n[projection.selection.selection]\nmode = \"single\"\n\n[projection.selection.rendering]\nwhitespace = \"rendered\"\nrewrite_urls = false\n{type_params}\n"
    ))
    .expect("HTML target TOML");
    document.validate().expect("valid HTML target");
    document
}

fn mutate_state(
    document: &StateDocument,
    mutate: impl FnOnce(&mut serde_json::Value),
) -> StateDocument {
    let mut wire = serde_json::to_value(document).expect("state JSON");
    mutate(&mut wire);
    serde_json::from_value(wire).expect("state JSON remains structurally valid")
}

fn immutable_payload_bytes(value: &serde_json::Value) -> Vec<u8> {
    value
        .as_array()
        .expect("payload byte array")
        .iter()
        .map(|value| u8::try_from(value.as_u64().expect("payload byte")).expect("u8"))
        .collect()
}

#[test]
fn html_target_contracts_hide_structured_output_and_bind_htmlcut_semantics_only_for_html() {
    #[derive(serde::Serialize)]
    struct JsonContract<'a> {
        source_kind: &'static str,
        target: &'a TargetSource,
        fetch: &'a FetchConfig,
        projection: &'a Projection,
        declared_type: DeclaredType,
        parser_id: &'static str,
        parser_grammar_version: u32,
        type_params: &'a TypeParams,
        conditions: &'a [Condition],
        escalate_after: u32,
    }
    #[derive(serde::Serialize)]
    struct HtmlContract<'a> {
        source_kind: &'static str,
        target: &'a TargetSource,
        fetch: &'a FetchConfig,
        projection: &'a Projection,
        declared_type: DeclaredType,
        parser_id: &'static str,
        parser_grammar_version: u32,
        type_params: &'a TypeParams,
        conditions: &'a [Condition],
        escalate_after: u32,
        htmlcut_extraction_semantics_version: u32,
    }

    let json = target("integer", "");
    let json_expected = crate::stable_json::stable_digest(&JsonContract {
        source_kind: "json_pointer",
        target: json.source(),
        fetch: json.fetch(),
        projection: json.projection(),
        declared_type: json.declared_type(),
        parser_id: PARSER_ID,
        parser_grammar_version: PARSER_GRAMMAR_VERSION,
        type_params: json.type_params(),
        conditions: json.conditions(),
        escalate_after: json.escalate_after(),
    })
    .expect("JSON expected digest");
    assert_eq!(
        json.contract_digest_sha256().expect("JSON digest"),
        json_expected
    );

    let html = html_target(
        "html_attribute",
        "meta#price",
        Some("content"),
        "decimal",
        "",
    );
    let html_expected = crate::stable_json::stable_digest(&HtmlContract {
        source_kind: "html_attribute",
        target: html.source(),
        fetch: html.fetch(),
        projection: html.projection(),
        declared_type: html.declared_type(),
        parser_id: PARSER_ID,
        parser_grammar_version: PARSER_GRAMMAR_VERSION,
        type_params: html.type_params(),
        conditions: html.conditions(),
        escalate_after: html.escalate_after(),
        htmlcut_extraction_semantics_version:
            htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION,
    })
    .expect("HTML expected digest");
    assert_eq!(
        html.contract_digest_sha256().expect("HTML digest"),
        html_expected
    );

    let incorrect_counter = crate::stable_json::stable_digest(&HtmlContract {
        source_kind: "html_attribute",
        target: html.source(),
        fetch: html.fetch(),
        projection: html.projection(),
        declared_type: html.declared_type(),
        parser_id: PARSER_ID,
        parser_grammar_version: PARSER_GRAMMAR_VERSION,
        type_params: html.type_params(),
        conditions: html.conditions(),
        escalate_after: html.escalate_after(),
        htmlcut_extraction_semantics_version:
            htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION + 1,
    })
    .expect("incorrect counter digest");
    assert_ne!(
        html.contract_digest_sha256().expect("HTML digest"),
        incorrect_counter
    );

    let mut wire = serde_json::to_value(&html).expect("HTML target JSON");
    wire["projection"]["output"] = serde_json::json!({"kind": "structured"});
    assert!(serde_json::from_value::<TargetDocument>(wire).is_err());
}

#[test]
fn html_attribute_requires_css_selection_and_a_single_measurement() {
    let css_attribute = html_target(
        "html_attribute",
        "time#published",
        Some("datetime"),
        "datetime",
        "[type_params]\nformat = \"rfc3339\"\n",
    );
    assert!(matches!(
        css_attribute.projection(),
        Projection::HtmlAttribute { .. }
    ));

    let delimiter_attribute = mutate_target(&css_attribute, |wire| {
        wire["projection"]["selection"]["strategy"] = serde_json::json!({
            "kind": "delimiter_pair",
            "start": "<time>",
            "end": "</time>",
            "mode": "literal",
            "boundary_retention": "exclude_both"
        });
    });
    assert_eq!(
        delimiter_attribute
            .permanent_error()
            .map(|error| error.code()),
        Some(PermanentErrorCode::HtmlAttributeRequiresCssSelector)
    );
    assert!(delimiter_attribute.validate().is_err());

    let every_match = mutate_target(&css_attribute, |wire| {
        wire["projection"]["selection"]["selection"] = serde_json::json!({"mode": "all"});
    });
    assert_eq!(
        every_match.permanent_error().map(|error| error.code()),
        Some(PermanentErrorCode::HtmlSelectionMustSelectOne)
    );
    assert!(every_match.validate().is_err());
}

#[test]
fn html_dom_canonicalization_is_css_text_only_and_binds_the_measurement_contract() {
    let html_text = html_target("html_text", "article.measurement a", None, "integer", "");
    let canonicalized = mutate_target(&html_text, |wire| {
        wire["projection"]["selection"]["dom_canonicalization"] = serde_json::json!({
            "ignore_attributes": ["href"],
            "strip_whitespace_nodes": true
        });
    });
    canonicalized
        .validate()
        .expect("CSS HTML text accepts DOM canonicalization");
    assert_ne!(
        canonicalized
            .contract_digest_sha256()
            .expect("canonicalized digest"),
        html_text.contract_digest_sha256().expect("plain digest")
    );

    let Projection::HtmlText { selection } = canonicalized.projection() else {
        panic!("fixture must remain an HTML text projection");
    };
    let policy = selection
        .dom_canonicalization()
        .expect("canonicalization policy is retained");
    assert_eq!(policy.ignore_attributes.len(), 1);
    assert!(policy.strip_whitespace_nodes);
    assert_eq!(
        selection
            .structured_plan()
            .dom_canonicalization
            .as_ref()
            .expect("HTMLCut plan retains policy"),
        policy
    );

    let delimiter = mutate_target(&canonicalized, |wire| {
        wire["projection"]["selection"]["strategy"] = serde_json::json!({
            "kind": "delimiter_pair",
            "start": "<article>",
            "end": "</article>",
            "mode": "literal",
            "boundary_retention": "exclude_both"
        });
    });
    let delimiter_error = delimiter
        .permanent_error()
        .expect("non-CSS canonicalization must be permanently invalid");
    assert_eq!(
        delimiter_error.code(),
        PermanentErrorCode::HtmlcutPlanInvalid
    );
    assert!(delimiter.validate().is_err());

    let html_attribute = html_target(
        "html_attribute",
        "meta#price",
        Some("content"),
        "decimal",
        "",
    );
    let canonicalized_attribute = mutate_target(&html_attribute, |wire| {
        wire["projection"]["selection"]["dom_canonicalization"] = serde_json::json!({
            "ignore_attributes": ["content"],
            "strip_whitespace_nodes": false
        });
    });
    let attribute_error = canonicalized_attribute
        .permanent_error()
        .expect("attribute canonicalization must be permanently invalid");
    assert_eq!(
        attribute_error.code(),
        PermanentErrorCode::HtmlcutPlanInvalid
    );
    assert!(canonicalized_attribute.validate().is_err());

    for removed_control in ["sort_attributes", "strip_comments"] {
        let mut wire = serde_json::to_value(&html_text).expect("HTML target JSON");
        wire["projection"]["selection"]["dom_canonicalization"] = serde_json::json!({
            "ignore_attributes": [],
            "strip_whitespace_nodes": false,
            removed_control: true
        });
        assert!(
            serde_json::from_value::<TargetDocument>(wire).is_err(),
            "removed {removed_control} control must be rejected rather than ignored"
        );
    }
}

#[test]
fn html_http_sources_with_url_userinfo_are_permanent_input_contract_errors() {
    let html = html_target("html_text", "article", None, "integer", "");
    let configured_http = |source_url: &str| {
        mutate_target(&html, |wire| {
            wire["target"] = serde_json::json!({
                "kind": "http",
                "source_url": source_url
            });
            wire["fetch"] = serde_json::json!({
                "engine": "http",
                "method": "GET",
                "timeout_ms": 1000,
                "max_bytes": 1024,
                "user_agent": "ffhn-test",
                "follow_redirects": false,
                "accept": "text/html",
                "headers": {}
            });
        })
    };
    let credential_free = configured_http("https://example.test/article");
    assert!(credential_free.permanent_error().is_none());

    let invalid = mutate_target(&html, |wire| {
        wire["target"] = serde_json::json!({
            "kind": "http",
            "source_url": "https://reader:secret@example.test/article"
        });
        wire["fetch"] = serde_json::json!({
            "engine": "http",
            "method": "GET",
            "timeout_ms": 1000,
            "max_bytes": 1024,
            "user_agent": "ffhn-test",
            "follow_redirects": false,
            "accept": "text/html",
            "headers": {}
        });
    });

    assert_eq!(
        invalid.permanent_error().map(|error| error.code()),
        Some(PermanentErrorCode::HtmlcutInputInvalid)
    );
    assert!(invalid.validate_without_projection().is_ok());
    assert!(invalid.validate().is_err());

    let password_only = configured_http("https://:secret@example.test/article");
    assert_eq!(
        password_only.permanent_error().map(|error| error.code()),
        Some(PermanentErrorCode::HtmlcutInputInvalid)
    );
}

#[test]
fn state_validation_covers_each_coherence_guard_without_relaxing_the_contract() {
    let plain_target = target("integer", "");
    let empty = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        plain_target.contract_digest_sha256().expect("digest"),
    );

    let missing_baseline = mutate_state(&empty, |wire| {
        wire["condition_state"] = serde_json::json!({
            "condition": {"result": "not_satisfied", "active": false},
        });
    });
    assert!(missing_baseline.validate().is_err());

    let observation = plain_target
        .parse_json_scalar_token("10".to_owned())
        .expect("observation");
    let zero_sequence = mutate_state(&empty, |wire| {
        wire["accepted_observation"] =
            serde_json::to_value(&observation).expect("observation JSON");
        wire["fixed_initial_baseline"] = serde_json::to_value(&observation).expect("baseline JSON");
    });
    assert!(zero_sequence.validate().is_err());

    let conditioned = mutate_target(&plain_target, |wire| {
        wire["conditions"] = serde_json::json!([{
            "condition_id": "condition",
            "predicate": {"kind": "lt", "threshold": "20"},
        }]);
    });
    conditioned.validate().expect("conditioned target");
    let observation = conditioned
        .parse_json_scalar_token("10".to_owned())
        .expect("conditioned observation");
    let mut state = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        conditioned.contract_digest_sha256().expect("digest"),
    );
    let staged = conditioned
        .stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: &observation,
            },
            &state.condition_contexts(&conditioned),
        )
        .expect("policy stage");
    state
        .apply_valid_observation(
            &conditioned,
            observation,
            staged.condition_evaluations().expect("evaluations"),
            "2026-07-15T00:00:00Z",
        )
        .expect("state transition");

    let no_transition_value = mutate_state(&state, |wire| {
        wire["condition_state"]["condition"]["last_transition_at"] = serde_json::Value::Null;
        wire["condition_state"]["condition"]["last_transition_value"] = serde_json::Value::Null;
    });
    no_transition_value
        .validate()
        .expect("coherent no-transition state");
    no_transition_value
        .validate_for_target(&conditioned)
        .expect("target accepts no transition value");

    let missing_condition = mutate_state(&state, |wire| {
        wire["condition_state"] = serde_json::json!({});
    });
    assert!(missing_condition.validate_for_target(&conditioned).is_err());

    let wrong_condition = mutate_state(&state, |wire| {
        wire["condition_state"] = serde_json::json!({
            "other": wire["condition_state"]["condition"].clone(),
        });
    });
    assert!(wrong_condition.validate_for_target(&conditioned).is_err());

    for source_health in [
        serde_json::json!({"state": "healthy", "consecutive_unresolved": 1}),
        serde_json::json!({
            "state": "healthy",
            "consecutive_unresolved": 0,
            "first_unresolved_at": "2026-07-15T00:00:00Z",
        }),
        serde_json::json!({
            "state": "healthy",
            "consecutive_unresolved": 0,
            "last_details": {"kind": "io", "message": "failed"},
        }),
    ] {
        let invalid_health = mutate_state(&empty, |wire| {
            wire["source_health"] = source_health;
        });
        assert!(invalid_health.validate().is_err());
    }
}

#[test]
fn typed_parser_covers_exact_integer_decimal_money_and_semver() {
    assert_eq!(
        target("integer", "")
            .parse_json_scalar_token(i128::MAX.to_string())
            .expect("i128")
            .canonical_value(),
        i128::MAX.to_string()
    );
    assert_eq!(
        target("decimal", "")
            .parse_json_scalar_token("1.00".to_owned())
            .expect("decimal")
            .canonical_value(),
        "1"
    );
    assert_eq!(
        target("integer", "")
            .parse_json_scalar_token("not-json".to_owned())
            .expect_err("invalid JSON scalar token")
            .kind(),
        ProcessErrorKind::ValueUnparseable
    );
    assert!(
        target("decimal", "")
            .parse_json_scalar_token("999999999999999999999999999999999999999".to_owned())
            .is_err()
    );
    assert_eq!(
        target("money", "[type_params]\ncurrency = \"USD\"\n")
            .parse_json_scalar_token(r#""1,234.50""#.to_owned())
            .expect_err("invariant money rejects grouping")
            .kind(),
        ProcessErrorKind::ValueUnparseable
    );
    assert_eq!(
        target(
            "money",
            "[type_params]\ncurrency = \"USD\"\nlocale = \"en_us\"\n"
        )
        .parse_json_scalar_token(r#""1,234.50""#.to_owned())
        .expect("money")
        .canonical_value(),
        "1234.5"
    );
    assert_eq!(
        target("semver", "")
            .parse_json_scalar_token(r#""1.2.3+build.7""#.to_owned())
            .expect("semver")
            .canonical_value(),
        "1.2.3+build.7"
    );
}

#[test]
fn datetime_requires_a_declared_format_and_never_uses_machine_timezone() {
    assert_eq!(
        target("datetime", "[type_params]\nformat = \"rfc3339\"\n")
            .parse_json_scalar_token(r#""2026-07-14T10:00:00+02:00""#.to_owned())
            .expect("explicit offset")
            .canonical_value(),
        "2026-07-14T08:00:00Z"
    );
    assert_eq!(
            target(
                "datetime",
                "[type_params]\nformat = \"[year]-[month]-[day] [hour]:[minute]\"\nassumed_offset = \"+02:00\"\n"
            )
            .parse_json_scalar_token(r#""2026-07-14 10:00""#.to_owned())
            .expect("configured assumed offset")
            .canonical_value(),
            "2026-07-14T08:00:00Z"
        );
    assert!(
        target("datetime", "[type_params]\nformat = \"rfc3339\"\n")
            .parse_json_scalar_token(r#""2026-07-14T10:00:00""#.to_owned())
            .is_err()
    );
}

#[test]
fn json_pointer_acquisition_preserves_the_original_scalar_token() {
    assert_eq!(
        select_json_scalar_token(r#"{"value":"1.2.3\u002bbuild.7"}"#, "/value")
            .expect("escaped semver token"),
        r#""1.2.3\u002bbuild.7""#
    );
    assert_eq!(
        select_json_scalar_token(r#"{"a/b":{"~key":"1.00"}}"#, "/a~1b/~0key")
            .expect("escaped pointer token"),
        r#""1.00""#
    );
    assert_eq!(
        select_json_scalar_token(r#"{"items":[0,1.00]}"#, "/items/1").expect("array token"),
        "1.00"
    );
    assert_eq!(parse_json_array_index("0"), Some(0));
    assert!(decode_json_pointer_token("~2").is_err());
    assert!(normalized_raw_token(" ").is_err());
    assert!(json_scalar_value(" 1 ").is_err());
    assert!(select_json_pointer_child("{", "value").is_err());
    assert!(select_json_pointer_child("[", "0").is_err());
    assert!(select_json_pointer_child("1", "value").is_err());
    assert!(select_json_scalar_token(r#"{"value":[]}"#, "/value").is_err());
    assert!(select_json_scalar_token(r#"{"value":{}}"#, "/value").is_err());
    assert!(select_json_scalar_token(r#"{"items":[0]}"#, "/items/01").is_err());
}

#[test]
fn typed_observation_golden_fixtures_preserve_complete_v2_wire_records() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        name: String,
        declared_type: String,
        type_params_toml: String,
        raw_token: String,
        expected: serde_json::Value,
    }

    let fixtures: Vec<Fixture> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/typed-observations.json"
    )))
    .expect("typed observation fixtures");
    for fixture in fixtures {
        let observation = target(&fixture.declared_type, &fixture.type_params_toml)
            .parse_json_scalar_token(fixture.raw_token)
            .expect("valid typed observation");
        assert_eq!(
            serde_json::to_value(observation).expect("observation JSON"),
            fixture.expected,
            "fixture {}",
            fixture.name
        );
    }
}

#[test]
fn contract_digest_binds_every_measurement_and_policy_input() {
    let document = target("decimal", "");
    let digest = document.contract_digest_sha256().expect("base digest");

    let changed = mutate_target(&document, |wire| {
        wire["target"] = serde_json::json!({
            "kind": "file",
            "file_path": crate::test_support::absolute_file_path("other-source.json"),
        });
    });
    assert_ne!(
        changed.contract_digest_sha256().expect("source digest"),
        digest
    );

    let changed = mutate_target(&document, |wire| {
        wire["fetch"] = serde_json::json!({"engine": "file", "max_bytes": 4_096});
    });
    assert_ne!(
        changed.contract_digest_sha256().expect("fetch digest"),
        digest
    );

    let changed = mutate_target(&document, |wire| {
        wire["projection"] = serde_json::json!({"kind": "json_pointer", "pointer": "/other"});
    });
    assert_ne!(
        changed.contract_digest_sha256().expect("projection digest"),
        digest
    );

    let changed = mutate_target(&document, |wire| {
        wire["type_params"] = serde_json::json!({"locale": "en_us"});
    });
    assert_ne!(
        changed
            .contract_digest_sha256()
            .expect("type parameter digest"),
        digest
    );

    let changed = mutate_target(&document, |wire| {
        wire["declared_type"] = serde_json::json!("money");
        wire["type_params"] = serde_json::json!({"currency": "USD"});
    });
    assert_ne!(
        changed
            .contract_digest_sha256()
            .expect("declared type digest"),
        digest
    );

    let changed = mutate_target(&document, |wire| {
        wire["conditions"] = serde_json::json!([{
            "condition_id": "price-change",
            "predicate": {
                "kind": "changed",
                "reference": "last_accepted_observation"
            }
        }]);
    });
    changed.validate().expect("valid policy target");
    assert_ne!(
        changed.contract_digest_sha256().expect("condition digest"),
        digest
    );

    let changed = mutate_target(&document, |wire| {
        wire["escalate_after"] = serde_json::json!(4);
    });
    assert_ne!(
        changed.contract_digest_sha256().expect("escalation digest"),
        digest
    );
}

#[test]
fn delivery_configuration_is_operational_but_pending_records_cannot_be_rerouted() {
    let document = target("integer", "");
    let measurement_digest = document
        .contract_digest_sha256()
        .expect("measurement digest");
    let routed = mutate_target(&document, |wire| {
        wire["outbox"] = serde_json::json!({
            "max_pending": 9,
            "max_attempts": 3,
            "base_backoff_ms": 250,
            "max_backoff_ms": 500,
        });
        wire["routes"] = serde_json::json!([{
            "route_id": "run",
            "route_family": "on_run",
            "adapter": {
                "kind": "process_stdin",
                "program": crate::test_support::PROCESS_PROGRAM,
                "args": [],
                "timeout_ms": 1000,
            }
        }]);
    });
    routed.validate().expect("valid delivery configuration");
    assert_eq!(routed.routes().len(), 1);
    assert_eq!(routed.routes()[0].route_id(), "run");
    assert_eq!(
        routed.contract_digest_sha256().expect("operational digest"),
        measurement_digest
    );

    let empty_state = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        measurement_digest,
    );
    let route_id = RouteId::new("run").expect("route id");
    let payload = ProcessStdinPayload::new(
        &route_id,
        RouteFamily::OnRun,
        &TargetId::new("demo").expect("target id"),
        "Demo",
        ProcessStdinEventKey::Reset {
            contract_digest_sha256: empty_state.contract_digest_sha256().to_owned(),
        },
        "Demo: reset",
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("payload");
    let event_id = payload.event_id().to_owned();
    let payload = payload.immutable_bytes().expect("payload bytes");
    let pending_state = mutate_state(&empty_state, |wire| {
        wire["outbox"] = serde_json::json!([{
            "event_id": event_id,
            "route_id": "run",
            "route_family": "on_run",
            "immutable_payload": payload,
            "attempt_count": 0,
            "next_retry_at": "2026-07-15T00:00:00Z",
        }]);
    });
    pending_state.validate().expect("valid pending record");
    pending_state
        .validate_for_target(&routed)
        .expect("matching pending route");
    let attempt_without_error = mutate_state(&pending_state, |wire| {
        wire["outbox"][0]["attempt_count"] = serde_json::json!(1);
    });
    assert!(attempt_without_error.validate().is_err());
    let error_without_attempt = mutate_state(&pending_state, |wire| {
        wire["outbox"][0]["last_error"] = serde_json::json!("failed");
    });
    assert!(error_without_attempt.validate().is_err());

    let removed_route = mutate_target(&routed, |wire| {
        wire["routes"] = serde_json::json!([]);
    });
    removed_route
        .validate()
        .expect("valid route removal config");
    assert_eq!(
        removed_route
            .contract_digest_sha256()
            .expect("unchanged measurement digest"),
        pending_state.contract_digest_sha256()
    );
    assert!(pending_state.validate_for_target(&removed_route).is_err());

    let moved_route = mutate_target(&routed, |wire| {
        wire["routes"][0]["route_family"] = serde_json::json!("on_condition");
    });
    moved_route.validate().expect("valid family move config");
    assert!(pending_state.validate_for_target(&moved_route).is_err());
}

#[test]
fn persisted_process_payloads_are_canonical_and_bound_to_their_pending_records() {
    let routed = mutate_target(&target("integer", ""), |wire| {
        wire["routes"] = serde_json::json!([{
            "route_id": "run",
            "route_family": "on_run",
            "adapter": {
                "kind": "process_stdin",
                "program": crate::test_support::PROCESS_PROGRAM,
                "args": [],
                "timeout_ms": 1000,
            }
        }]);
    });
    routed.validate().expect("routed target");
    let state = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        routed.contract_digest_sha256().expect("digest"),
    );
    let route_id = RouteId::new("run").expect("route id");
    let payload = ProcessStdinPayload::new(
        &route_id,
        RouteFamily::OnRun,
        &TargetId::new("demo").expect("target id"),
        "Demo",
        ProcessStdinEventKey::Reset {
            contract_digest_sha256: state.contract_digest_sha256().to_owned(),
        },
        "Demo: reset",
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("payload");
    let event_id = payload.event_id().to_owned();
    let payload = payload.immutable_bytes().expect("canonical payload");
    let valid = mutate_state(&state, |wire| {
        wire["outbox"] = serde_json::json!([{
            "event_id": event_id,
            "route_id": "run",
            "route_family": "on_run",
            "immutable_payload": payload,
            "attempt_count": 0,
            "next_retry_at": "2026-07-15T00:00:00Z",
        }]);
    });
    valid
        .validate_for_target(&routed)
        .expect("canonical matching payload");

    let malformed = mutate_state(&valid, |wire| {
        wire["outbox"][0]["immutable_payload"] = serde_json::json!([123]);
    });
    assert!(malformed.validate().is_err());

    let mismatched = mutate_state(&valid, |wire| {
        let mut payload: serde_json::Value = serde_json::from_slice(&immutable_payload_bytes(
            &wire["outbox"][0]["immutable_payload"],
        ))
        .expect("payload JSON");
        payload["event_id"] = serde_json::json!("b".repeat(64));
        wire["outbox"][0]["immutable_payload"] = serde_json::json!(
            crate::stable_json::stable_json(&payload)
                .expect("canonical altered payload")
                .into_bytes()
        );
    });
    assert!(mismatched.validate().is_err());

    let forged_identity = mutate_state(&valid, |wire| {
        let forged_event_id = "b".repeat(64);
        let mut payload: serde_json::Value = serde_json::from_slice(&immutable_payload_bytes(
            &wire["outbox"][0]["immutable_payload"],
        ))
        .expect("payload JSON");
        payload["event_id"] = serde_json::json!(forged_event_id);
        wire["outbox"][0]["event_id"] = payload["event_id"].clone();
        wire["outbox"][0]["immutable_payload"] = serde_json::json!(
            crate::stable_json::stable_json(&payload)
                .expect("canonical forged payload")
                .into_bytes()
        );
    });
    assert!(forged_identity.validate().is_err());

    let noncanonical = mutate_state(&valid, |wire| {
        let bytes = immutable_payload_bytes(&wire["outbox"][0]["immutable_payload"])
            .into_iter()
            .chain(std::iter::once(b' '))
            .collect::<Vec<_>>();
        wire["outbox"][0]["immutable_payload"] = serde_json::json!(bytes);
    });
    assert!(noncanonical.validate().is_err());

    let inconsistent_summary = mutate_state(&valid, |wire| {
        let mut payload: serde_json::Value = serde_json::from_slice(&immutable_payload_bytes(
            &wire["outbox"][0]["immutable_payload"],
        ))
        .expect("payload JSON");
        payload["summary"] = serde_json::json!("incorrect summary");
        wire["outbox"][0]["immutable_payload"] = serde_json::json!(
            crate::stable_json::stable_json(&payload)
                .expect("canonical altered payload")
                .into_bytes()
        );
    });
    assert!(inconsistent_summary.validate().is_err());
}

#[test]
fn delivery_value_objects_enforce_the_complete_operational_contract() {
    assert_eq!(RouteFamily::OnRun.as_str(), "on_run");
    assert_eq!(RouteFamily::OnCondition.as_str(), "on_condition");

    let route_id = RouteId::new("primary-route").expect("valid route id");
    assert_eq!(route_id.as_str(), "primary-route");
    assert_eq!(route_id.as_ref(), "primary-route");
    assert_eq!(route_id.to_string(), "primary-route");
    assert_eq!(String::from(route_id.clone()), "primary-route");
    assert_eq!(
        "primary-route".parse::<RouteId>().expect("parse route id"),
        route_id
    );
    assert_eq!(
        RouteId::try_from("secondary".to_owned())
            .expect("try route id")
            .as_str(),
        "secondary"
    );
    assert_eq!(
        RouteId::new("1route").expect("digit-led route id").as_str(),
        "1route"
    );
    for invalid in [
        "",
        "-leading",
        "_leading",
        "trailing-",
        "double--separator",
        "double__separator",
        "mixed-_separator",
        "mixed_-separator",
        "upperCase",
        "has space",
        &"a".repeat(65),
    ] {
        assert!(
            RouteId::new(invalid).is_err(),
            "{invalid:?} must be rejected"
        );
    }

    let defaults = OutboxPolicy::default();
    assert_eq!(defaults.max_pending(), 100);
    assert_eq!(defaults.max_attempts(), 5);
    assert_eq!(defaults.base_backoff_ms(), 1_000);
    assert_eq!(defaults.max_backoff_ms(), 300_000);
    defaults.validate().expect("default policy");
    for (field, value) in [
        ("max_pending", serde_json::json!(0)),
        ("max_attempts", serde_json::json!(0)),
        ("base_backoff_ms", serde_json::json!(0)),
        ("max_backoff_ms", serde_json::json!(999)),
    ] {
        let mut wire = serde_json::to_value(&defaults).expect("policy JSON");
        wire[field] = value;
        let policy: OutboxPolicy = serde_json::from_value(wire).expect("policy shape");
        assert!(policy.validate().is_err(), "{field} must be bounded");
    }

    let successful_args = crate::test_support::SUCCESSFUL_PROCESS_ARGS
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect::<Vec<_>>();
    let valid_route: DeliveryRoute = serde_json::from_value(serde_json::json!({
        "route_id": "primary",
        "route_family": "on_run",
        "adapter": {
            "kind": "process_stdin",
            "program": crate::test_support::PROCESS_PROGRAM,
            "args": successful_args,
            "timeout_ms": 100,
        }
    }))
    .expect("route shape");
    valid_route.validate().expect("valid route");
    assert_eq!(valid_route.route_id(), "primary");
    assert_eq!(valid_route.route_family(), RouteFamily::OnRun);
    assert_eq!(
        valid_route.adapter().process_stdin(),
        (
            crate::test_support::PROCESS_PROGRAM,
            &successful_args[..],
            100,
        )
    );
    validate_routes(&[]).expect("empty route list");
    validate_routes(std::slice::from_ref(&valid_route)).expect("one route");

    for (field, value) in [
        ("program", serde_json::json!("relative-program")),
        ("program", serde_json::json!("   ")),
        ("args", serde_json::json!([" "])),
        ("timeout_ms", serde_json::json!(99)),
    ] {
        let mut wire = serde_json::to_value(&valid_route).expect("route JSON");
        wire["adapter"][field] = value;
        let route: DeliveryRoute = serde_json::from_value(wire).expect("route shape");
        assert!(route.validate().is_err(), "{field} must be bounded");
    }
    assert!(validate_routes(&[valid_route.clone(), valid_route.clone()]).is_err());
    let later: DeliveryRoute = serde_json::from_value(serde_json::json!({
        "route_id": "zeta",
        "route_family": "on_condition",
        "adapter": {"kind": "process_stdin", "program": crate::test_support::PROCESS_PROGRAM, "timeout_ms": 60000}
    }))
    .expect("later route");
    assert!(validate_routes(&[later, valid_route]).is_err());
}

#[test]
fn state_validation_rejects_each_incoherent_temporal_shape_before_runtime_use() {
    let plain_target = target("integer", "");
    let plain_digest = plain_target.contract_digest_sha256().expect("digest");
    let empty = StateDocument::new(TargetId::new("demo").expect("target id"), plain_digest);
    empty.validate().expect("empty state");
    let incoherent_baseline = mutate_state(&empty, |wire| {
        wire["observation_seq"] = serde_json::json!(1);
    });
    assert!(incoherent_baseline.validate().is_err());

    let one_condition = mutate_target(&plain_target, |wire| {
        wire["conditions"] = serde_json::json!([{
            "condition_id": "condition",
            "predicate": {"kind": "lt", "threshold": "20"},
        }]);
    });
    one_condition.validate().expect("condition target");
    let mut state = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        one_condition.contract_digest_sha256().expect("digest"),
    );
    let observation = one_condition
        .parse_json_scalar_token("10".to_owned())
        .expect("observation");
    let staged = one_condition
        .stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: &observation,
            },
            &state.condition_contexts(&one_condition),
        )
        .expect("policy stage");
    state
        .apply_valid_observation(
            &one_condition,
            observation,
            staged.condition_evaluations().expect("evaluations"),
            "2026-07-15T00:00:00Z",
        )
        .expect("valid temporal state");

    let type_mismatched_observations = mutate_state(&state, |wire| {
        wire["accepted_observation"]["declared_type"] = serde_json::json!("decimal");
        wire["fixed_initial_baseline"]["declared_type"] = serde_json::json!("decimal");
    });
    type_mismatched_observations
        .validate()
        .expect("individually valid typed observations");
    assert!(
        type_mismatched_observations
            .validate_for_target(&one_condition)
            .is_err()
    );

    let transition_mismatch = mutate_state(&state, |wire| {
        wire["condition_state"]["condition"]["last_transition_value"]["declared_type"] =
            serde_json::json!("decimal");
    });
    transition_mismatch
        .validate()
        .expect("individually valid transition value");
    assert!(
        transition_mismatch
            .validate_for_target(&one_condition)
            .is_err()
    );
    assert!(state.validate_for_target(&plain_target).is_err());

    let half_transition = mutate_state(&state, |wire| {
        wire["condition_state"]["condition"]["last_transition_at"] = serde_json::Value::Null;
    });
    assert!(half_transition.validate().is_err());

    let invalid_source_shapes = [
        mutate_state(&empty, |wire| {
            wire["source_health"] = serde_json::json!({
                "state": "healthy",
                "reason_class": "fetch_failed",
                "consecutive_unresolved": 0,
            });
        }),
        mutate_state(&empty, |wire| {
            wire["source_health"] = serde_json::json!({
                "state": "suspect",
                "consecutive_unresolved": 1,
            });
        }),
        mutate_state(&empty, |wire| {
            wire["source_health"] = serde_json::json!({
                "state": "suspect",
                "reason_class": "fetch_failed",
                "consecutive_unresolved": 0,
                "first_unresolved_at": "2026-07-15T00:00:00Z",
                "last_details": {"kind": "io", "message": "failed"},
            });
        }),
    ];
    assert!(
        invalid_source_shapes
            .iter()
            .all(|state| state.validate().is_err())
    );

    let decimal_observation = target("decimal", "")
        .parse_json_scalar_token("10".to_owned())
        .expect("decimal observation");
    assert!(
        StateDocument::new(
            TargetId::new("demo").expect("target id"),
            one_condition.contract_digest_sha256().expect("digest"),
        )
        .apply_valid_observation(
            &one_condition,
            decimal_observation,
            &[],
            "2026-07-15T00:00:00Z"
        )
        .is_err()
    );
    assert!(
        StateDocument::new(
            TargetId::new("demo").expect("target id"),
            one_condition.contract_digest_sha256().expect("digest"),
        )
        .apply_valid_observation(
            &one_condition,
            one_condition
                .parse_json_scalar_token("10".to_owned())
                .expect("integer observation"),
            &[],
            "2026-07-15T00:00:00Z",
        )
        .is_err()
    );
    let other_condition = mutate_target(&one_condition, |wire| {
        wire["conditions"][0]["condition_id"] = serde_json::json!("other");
    });
    other_condition.validate().expect("other condition target");
    let other_observation = other_condition
        .parse_json_scalar_token("10".to_owned())
        .expect("other observation");
    let other_state = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        other_condition.contract_digest_sha256().expect("digest"),
    );
    let other_staged = other_condition
        .stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: &other_observation,
            },
            &other_state.condition_contexts(&other_condition),
        )
        .expect("other policy stage");
    assert!(
        StateDocument::new(
            TargetId::new("demo").expect("target id"),
            one_condition.contract_digest_sha256().expect("digest"),
        )
        .apply_valid_observation(
            &one_condition,
            one_condition
                .parse_json_scalar_token("10".to_owned())
                .expect("integer observation"),
            other_staged
                .condition_evaluations()
                .expect("other evaluations"),
            "2026-07-15T00:00:00Z",
        )
        .is_err()
    );
    assert!(
        empty
            .clone()
            .apply_source_suspect(
                SourceSuspectReason::FetchFailed,
                ProcessErrorDetail::new(ProcessErrorKind::Io, "failed", None),
                "2026-07-15T00:00:00Z",
                0,
            )
            .is_err()
    );
}

#[test]
fn delivery_report_accessors_expose_attempt_errors_and_queue_overflow() {
    let failed = DeliveryOutcome::dead_lettered(
        "a".repeat(64),
        "route".to_owned(),
        2,
        "delivery failed".to_owned(),
    );
    assert_eq!(failed.event_id(), "a".repeat(64));
    assert_eq!(failed.route_id(), "route");
    assert_eq!(failed.attempt_count(), 2);
    assert_eq!(failed.error(), Some("delivery failed"));
    let uncommitted = DeliveryOutcome::delivered_uncommitted(
        "c".repeat(64),
        "route".to_owned(),
        3,
        "state write failed",
    );
    assert_eq!(uncommitted.status(), DeliveryStatus::DeliveredUncommitted);
    assert!(
        uncommitted
            .error()
            .is_some_and(|error| error.contains("state write failed"))
    );
    let overflow = OutboxOverflow::new("b".repeat(64), RouteId::new("overflow").expect("route"));
    let run = RunReport::new(RunReportParts {
        target_id: "demo".to_owned(),
        display_name: Some("Demo".to_owned()),
        run_mode: RunMode::Live,
        outcome: RunOutcome::Initialized,
        started: "2026-07-15T00:00:00Z".to_owned(),
        finished: "2026-07-15T00:00:01Z".to_owned(),
        digest: Some("c".repeat(64)),
        observation: None,
        previous: None,
        error: None,
        state_persisted: true,
        delivery_outcomes: vec![failed.clone()],
        outbox_overflow: vec![overflow.clone()],
        outbox_error: Some("outbox drain stopped".to_owned()),
    });
    assert!(run.has_delivery_failure());
    assert!(run.has_delivery_problem());
    assert_eq!(run.delivery_outcomes(), &[failed]);
    assert_eq!(run.outbox_overflow(), std::slice::from_ref(&overflow));
    assert_eq!(run.outbox_error(), Some("outbox drain stopped"));
    assert_eq!(
        serde_json::to_value(&run).expect("run report JSON")["schema_version"],
        8
    );

    let reset = ResetReport::new(
        "demo",
        true,
        Vec::new(),
        vec![overflow],
        Some("outbox drain stopped".to_owned()),
    );
    assert!(reset.delivery_outcomes().is_empty());
    assert_eq!(reset.outbox_overflow().len(), 1);
    assert_eq!(reset.outbox_error(), Some("outbox drain stopped"));
    assert!(reset.has_delivery_problem());
    assert_eq!(
        serde_json::to_value(reset).expect("reset report JSON")["schema_version"],
        3
    );
}

#[test]
fn target_validation_covers_source_fetch_pointer_and_type_parameter_boundaries() {
    let valid_file = target("decimal", "");
    assert_eq!(valid_file.target_id(), "demo");
    assert_eq!(valid_file.display_name(), "Demo");
    assert!(valid_file.enabled());
    assert_eq!(valid_file.escalate_after(), 3);
    assert!(matches!(valid_file.source(), TargetSource::File { .. }));
    assert_eq!(valid_file.fetch().engine(), FetchEngine::File);
    assert!(matches!(
        valid_file.projection(),
        Projection::JsonPointer { .. }
    ));
    assert_eq!(valid_file.declared_type(), DeclaredType::Decimal);
    assert_eq!(valid_file.type_params(), &TypeParams::default());
    assert_eq!(
        valid_file.contract_digest_sha256().expect("digest").len(),
        64
    );

    let mut invalid = mutate_target(&valid_file, |wire| {
        wire["schema_version"] = serde_json::json!(8);
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["schema_name"] = serde_json::json!("other");
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["display_name"] = serde_json::json!(" ");
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["escalate_after"] = serde_json::json!(0);
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["target"] = serde_json::json!({"kind": "file", "file_path": "relative.json"});
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["projection"] = serde_json::json!({"kind": "json_pointer", "pointer": "value"});
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["projection"] =
            serde_json::json!({"kind": "json_pointer", "pointer": "/broken~2escape"});
    });
    assert!(invalid.validate().is_err());
    assert!(validate_json_pointer("/~0/~1").is_ok());
    invalid = mutate_target(&valid_file, |wire| {
        wire["fetch"] = serde_json::json!({"engine": "file", "max_bytes": 1});
    });
    assert!(invalid.validate().is_err());
    invalid = mutate_target(&valid_file, |wire| {
        wire["type_params"] = serde_json::json!({"currency": "USD"});
    });
    assert!(invalid.validate().is_err());

    let http = http_target();
    assert_eq!(http.fetch().engine(), FetchEngine::Http);
    let mut invalid_http = mutate_target(&http, |wire| {
        wire["target"] =
            serde_json::json!({"kind": "http", "source_url": "ftp://example.test/value"});
    });
    assert!(invalid_http.validate().is_err());
    invalid_http = mutate_target(&http, |wire| {
        wire["fetch"] = serde_json::json!({"engine": "file", "max_bytes": 1_024});
    });
    assert!(invalid_http.validate().is_err());
    invalid_http = mutate_target(&http, |wire| {
        wire["fetch"]["timeout_ms"] = serde_json::json!(999);
    });
    assert!(invalid_http.validate().is_err());
    invalid_http = mutate_target(&http, |wire| {
        wire["fetch"]["user_agent"] = serde_json::json!(" ");
        wire["fetch"]["accept"] = serde_json::json!(" ");
        wire["fetch"]["headers"] = serde_json::json!({" ": " "});
    });
    assert!(invalid_http.validate().is_err());

    let invalid_money = mutate_target(&valid_file, |wire| {
        wire["declared_type"] = serde_json::json!("money");
        wire["type_params"] = serde_json::json!({"currency": "US"});
    });
    assert!(invalid_money.validate().is_err());
    assert!(
        target("datetime", "[type_params]\nformat = \"rfc3339\"\n")
            .clone()
            .validate()
            .is_ok()
    );
    assert!(
        validate_type_params(
            DeclaredType::Datetime,
            &TypeParams {
                format: Some("[bad".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Datetime,
            &TypeParams {
                format: Some("rfc3339".to_owned()),
                assumed_offset: Some("+02:00".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Datetime,
            &TypeParams {
                format: Some("[year]".to_owned()),
                assumed_offset: Some("not-an-offset".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
}

#[test]
fn typed_value_helpers_reject_invalid_presentations_and_cover_all_formats() {
    assert_eq!(
        normalize_decimal_input("12.50", None).expect("invariant"),
        "12.50"
    );
    assert!(normalize_decimal_input("1,000", None).is_err());
    assert_eq!(
        normalize_decimal_input("1,234.50", Some(NumericLocale::EnUs)).expect("en"),
        "1234.50"
    );
    assert_eq!(
        normalize_decimal_input("1.234,50", Some(NumericLocale::DeDe)).expect("de"),
        "1234.50"
    );
    assert!(normalize_grouped_decimal("12,34", ',', '.').is_err());
    assert!(normalize_grouped_decimal("1.2.3", ',', '.').is_err());
    assert!(normalize_grouped_decimal(".2", ',', '.').is_err());
    assert!(normalize_grouped_decimal("1,234.", ',', '.').is_err());
    assert!(normalize_grouped_decimal("1,,234", ',', '.').is_err());
    assert!(normalize_grouped_decimal("1234,567", ',', '.').is_err());
    assert!(normalize_grouped_decimal("1,2a4", ',', '.').is_err());
    assert!(normalize_grouped_decimal("1,234.5x", ',', '.').is_err());
    assert_eq!(
        normalize_grouped_decimal("123", ',', '.').expect("ungrouped decimal"),
        "123"
    );
    assert_eq!(
        normalize_grouped_decimal("1,234", ',', '.').expect("whole grouped decimal"),
        "1234"
    );
    assert_eq!(
        normalize_grouped_decimal("1,234.5", ',', '.').expect("decimal fraction"),
        "1234.5"
    );
    assert_eq!(
        parse_offset("-02:30").expect("offset").whole_seconds(),
        -9_000
    );
    assert!(parse_offset("+99:99").is_err());
    assert!(parse_offset("02:00").is_err());
    assert!(parse_offset("*02:00").is_err());
    assert!(parse_offset("+02-00").is_err());
    assert!(require_text("field", " \t").is_err());
    assert!(validate_max_bytes(1_024).is_ok());
    assert!(validate_max_bytes(1_023).is_err());
    assert_eq!(default_timeout_ms(), 15_000);
    assert_eq!(default_max_bytes(), 2_000_000);
    assert!(default_follow_redirects());
    assert!(is_sha256(&"a".repeat(64)));
    assert!(!is_sha256(&"A".repeat(64)));
    assert!(!is_sha256(&"!".repeat(64)));
    assert!(parse_canonical_value(DeclaredType::Integer, &TypeParams::default(), "x").is_err());
    assert!(parse_canonical_value(DeclaredType::Semver, &TypeParams::default(), "1.2").is_err());
    assert!(
        parse_canonical_value(
            DeclaredType::Datetime,
            &TypeParams {
                format: Some("rfc3339".to_owned()),
                ..TypeParams::default()
            },
            "not-a-date"
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Integer,
            &TypeParams {
                locale: Some(NumericLocale::Invariant),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Semver,
            &TypeParams {
                currency: Some("USD".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Decimal,
            &TypeParams {
                currency: Some("USD".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Decimal,
            &TypeParams {
                format: Some("ignored".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Decimal,
            &TypeParams {
                assumed_offset: Some("+02:00".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Money,
            &TypeParams {
                currency: Some("USD".to_owned()),
                format: Some("rfc3339".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Money,
            &TypeParams {
                currency: Some("usd".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Money,
            &TypeParams {
                currency: Some("USD".to_owned()),
                assumed_offset: Some("+02:00".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Datetime,
            &TypeParams {
                format: Some("rfc3339".to_owned()),
                locale: Some(NumericLocale::Invariant),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert!(
        validate_type_params(
            DeclaredType::Datetime,
            &TypeParams {
                format: Some("rfc3339".to_owned()),
                currency: Some("USD".to_owned()),
                ..TypeParams::default()
            }
        )
        .is_err()
    );
    assert_eq!(
            parse_datetime(
                "2026-07-14 10:00 +02:00",
                &TypeParams {
                    format: Some(
                        "[year]-[month]-[day] [hour]:[minute] [offset_hour sign:mandatory]:[offset_minute]"
                            .to_owned()
                    ),
                    ..TypeParams::default()
                }
            )
            .expect("offset format"),
            "2026-07-14T08:00:00Z"
        );
}

#[test]
fn state_reports_and_tokens_expose_one_complete_v2_document_surface() {
    let document = target("integer", "");
    let observation = document
        .parse_json_scalar_token("7".to_owned())
        .expect("observation");
    assert_eq!(observation.raw_selected(), "7");
    assert_eq!(observation.comparison_projection(), "7");
    assert_eq!(observation.canonical_value(), "7");
    let digest = document.contract_digest_sha256().expect("digest");
    let mut state = StateDocument::new(TargetId::new("demo").expect("target"), digest.clone());
    state
        .apply_valid_observation(&document, observation.clone(), &[], "2026-07-15T00:00:00Z")
        .expect("apply valid observation");
    state.validate().expect("state");
    assert_eq!(state.target_id(), "demo");
    assert_eq!(state.contract_digest_sha256(), digest);
    assert_eq!(state.accepted_observation(), Some(&observation));
    let mut invalid_state = mutate_state(&state, |wire| {
        wire["schema_name"] = serde_json::json!("other");
    });
    assert!(invalid_state.validate().is_err());
    invalid_state = mutate_state(&state, |wire| {
        wire["schema_version"] = serde_json::json!(8);
    });
    assert!(invalid_state.validate().is_err());
    invalid_state = mutate_state(&state, |wire| {
        wire["parser_id"] = serde_json::json!("other");
    });
    assert!(invalid_state.validate().is_err());
    invalid_state = mutate_state(&state, |wire| {
        wire["parser_grammar_version"] = serde_json::json!(0);
    });
    assert!(invalid_state.validate().is_err());
    invalid_state = mutate_state(&state, |wire| {
        wire["contract_digest_sha256"] = serde_json::json!("bad");
    });
    assert!(invalid_state.validate().is_err());

    let mut invalid_wire = serde_json::to_value(&state).expect("state JSON");
    invalid_wire["accepted_observation"]["canonical_value"] =
        serde_json::Value::String("not-an-integer".to_owned());
    let invalid_state: StateDocument =
        serde_json::from_value(invalid_wire).expect("state document");
    assert!(invalid_state.validate().is_err());
    let mut invalid_wire = serde_json::to_value(&state).expect("state JSON");
    invalid_wire["accepted_observation"]["parser_id"] =
        serde_json::Value::String("other-parser".to_owned());
    let invalid_state: StateDocument =
        serde_json::from_value(invalid_wire).expect("state document");
    assert!(invalid_state.validate().is_err());
    let mut invalid_wire = serde_json::to_value(&state).expect("state JSON");
    invalid_wire["accepted_observation"]["parser_grammar_version"] =
        serde_json::Value::Number(0.into());
    let invalid_state: StateDocument =
        serde_json::from_value(invalid_wire).expect("state document");
    assert!(invalid_state.validate().is_err());
    let invalid_state = mutate_state(&state, |wire| {
        wire["accepted_observation"]["comparison_projection"] = serde_json::json!("8");
    });
    assert!(invalid_state.validate().is_err());
    let invalid_state = mutate_state(&state, |wire| {
        wire["accepted_observation"]["raw_selected"] = serde_json::json!(" ");
        wire["accepted_observation"]["comparison_projection"] = serde_json::json!(" ");
    });
    assert!(invalid_state.validate().is_err());
    let invalid_state = mutate_state(&state, |wire| {
        wire["accepted_observation"]["raw_selected"] = serde_json::json!(r#""not-an-integer""#);
        wire["accepted_observation"]["comparison_projection"] =
            serde_json::json!(r#""not-an-integer""#);
    });
    assert!(invalid_state.validate().is_err());
    let invalid_state = mutate_state(&state, |wire| {
        wire["accepted_observation"]["parse_diagnostics"] =
            serde_json::json!(["invented diagnostic"]);
    });
    assert!(invalid_state.validate().is_err());

    let detail = ProcessErrorDetail::new(
        ProcessErrorKind::Contract,
        "detail",
        Some("path".to_owned()),
    );
    assert_eq!(detail.kind(), ProcessErrorKind::Contract);
    assert_eq!(detail.message(), "detail");
    let report = RunReport::new(RunReportParts {
        target_id: "demo".to_owned(),
        display_name: Some("Demo".to_owned()),
        run_mode: RunMode::Live,
        outcome: RunOutcome::Changed,
        started: "2026-01-01T00:00:00Z".to_owned(),
        finished: "2026-01-01T00:00:01Z".to_owned(),
        digest: Some(digest),
        observation: Some(observation),
        previous: Some("6".to_owned()),
        error: Some(detail),
        state_persisted: true,
        delivery_outcomes: Vec::new(),
        outbox_overflow: Vec::new(),
        outbox_error: None,
    });
    assert_eq!(report.target_id(), "demo");
    assert_eq!(report.display_name(), Some("Demo"));
    assert_eq!(report.run_mode(), RunMode::Live);
    assert_eq!(report.outcome(), RunOutcome::Changed);
    assert!(report.observation().is_some());
    assert!(report.error_detail().is_some());
    assert!(report.state_persisted());
    let batch = BatchRunReport::new(RunMode::Live, vec!["demo".to_owned()], vec![report]);
    assert_eq!(batch.reports().len(), 1);
    let status = StatusReport::new(
        "demo",
        StatusKind::Ready,
        Some("Demo".to_owned()),
        Some(true),
        Some("a".repeat(64)),
        state.accepted_observation().cloned(),
        None,
    );
    assert_eq!(status.kind(), StatusKind::Ready);
    assert!(status.accepted_observation().is_some());
    let reset = ResetReport::new("demo", true, Vec::new(), Vec::new(), None);
    assert!(reset.storage_cleared());
    assert_eq!(
        serde_json::to_value(reset).expect("reset JSON")["storage_cleared"],
        true
    );
    assert_eq!(RunMode::Live.as_str(), "live");
    assert_eq!(RunMode::DryRun.as_str(), "dry_run");
    let tokens = [
        RunOutcome::Initialized,
        RunOutcome::Changed,
        RunOutcome::Unchanged,
        RunOutcome::SkippedDisabled,
        RunOutcome::RefusedContractDigest,
        RunOutcome::AcquisitionFailed,
        RunOutcome::ValueUnparseable,
        RunOutcome::ConfigInvalid,
        RunOutcome::TargetUnavailable,
        RunOutcome::StateInvalid,
        RunOutcome::LockUnavailable,
        RunOutcome::FetchFailed,
        RunOutcome::PersistFailed,
    ]
    .map(RunOutcome::as_str);
    assert_eq!(tokens.len(), 13);
}

#[test]
fn state_episodes_preserve_the_closed_taxonomy_boundaries() {
    let document = target("integer", "");
    let observation = document
        .parse_json_scalar_token("7".to_owned())
        .expect("observation");
    let digest = document.contract_digest_sha256().expect("digest");
    let mut state = StateDocument::new(TargetId::new("demo").expect("target"), digest);

    assert!(
        state
            .apply_permanent_error(
                PermanentErrorCode::InvalidJsonPointer,
                "2026-07-15T00:00:00Z",
            )
            .expect("first permanent episode")
    );
    assert!(
        !state
            .apply_permanent_error(
                PermanentErrorCode::InvalidJsonPointer,
                "2026-07-15T00:00:01Z",
            )
            .expect("continued permanent episode")
    );
    let continued_permanent = serde_json::to_value(&state).expect("permanent state JSON");
    assert_eq!(
        continued_permanent["permanent_error_episode"]["error_code"],
        "invalid_json_pointer"
    );
    assert_eq!(
        continued_permanent["permanent_error_episode"]["first_seen_at"],
        "2026-07-15T00:00:00Z"
    );

    state
        .apply_valid_observation(&document, observation, &[], "2026-07-15T00:00:03Z")
        .expect("valid observation clears permanent episode");
    assert!(
        serde_json::to_value(&state).expect("recovered state JSON")["permanent_error_episode"]
            .is_null()
    );
    assert!(
        state
            .apply_permanent_error(
                PermanentErrorCode::InvalidJsonPointer,
                "2026-07-15T00:00:04Z",
            )
            .expect("recurrent permanent episode")
    );
    assert_eq!(
        serde_json::to_value(&state).expect("recurrent state JSON")["permanent_error_episode"]["first_seen_at"],
        "2026-07-15T00:00:04Z"
    );

    let detail = ProcessErrorDetail::new(ProcessErrorKind::Contract, "source detail", None);
    assert!(
        !state
            .apply_source_suspect(
                SourceSuspectReason::JsonMalformed,
                detail.clone(),
                "2026-07-15T00:00:05Z",
                3,
            )
            .expect("first source-suspect failure")
    );
    assert!(
        !state
            .apply_source_suspect(
                SourceSuspectReason::JsonMalformed,
                detail.clone(),
                "2026-07-15T00:00:06Z",
                3,
            )
            .expect("continued source-suspect failure")
    );
    assert!(
        !state
            .apply_source_suspect(
                SourceSuspectReason::ValueUnparseable,
                detail.clone(),
                "2026-07-15T00:00:07Z",
                3,
            )
            .expect("changed source-suspect classification")
    );
    let changed_source = serde_json::to_value(&state).expect("source state JSON");
    assert_eq!(changed_source["source_health"]["consecutive_unresolved"], 1);
    assert_eq!(
        changed_source["source_health"]["first_unresolved_at"],
        "2026-07-15T00:00:07Z"
    );
    assert!(
        !state
            .apply_source_suspect(
                SourceSuspectReason::ValueUnparseable,
                detail.clone(),
                "2026-07-15T00:00:08Z",
                3,
            )
            .expect("second source-suspect failure")
    );
    assert!(
        state
            .apply_source_suspect(
                SourceSuspectReason::ValueUnparseable,
                detail.clone(),
                "2026-07-15T00:00:09Z",
                3,
            )
            .expect("source escalation boundary")
    );
    assert!(
        !state
            .apply_source_suspect(
                SourceSuspectReason::ValueUnparseable,
                detail,
                "2026-07-15T00:00:10Z",
                3,
            )
            .expect("post-escalation source-suspect failure")
    );
}

#[test]
fn state_timestamps_must_be_canonical_utc_rfc3339() {
    let document = target("integer", "");
    let digest = document.contract_digest_sha256().expect("digest");
    let mut state = StateDocument::new(TargetId::new("demo").expect("target"), digest);
    let detail = ProcessErrorDetail::new(ProcessErrorKind::Contract, "source detail", None);

    assert!(
        state
            .apply_source_suspect(
                SourceSuspectReason::JsonMalformed,
                detail.clone(),
                "2026-07-15T00:00:00+00:00",
                3
            )
            .is_err()
    );
    assert!(
        state
            .apply_permanent_error(
                PermanentErrorCode::InvalidJsonPointer,
                "2026-07-15T02:00:00+02:00",
            )
            .is_err()
    );

    state
        .apply_source_suspect(
            SourceSuspectReason::JsonMalformed,
            detail,
            "2026-07-15T00:00:00Z",
            3,
        )
        .expect("canonical source-health timestamp");
    let invalid_state = mutate_state(&state, |wire| {
        wire["source_health"]["first_unresolved_at"] =
            serde_json::json!("2026-07-15T00:00:00+00:00");
    });
    assert!(invalid_state.validate().is_err());
}

#[test]
fn state_rejects_invented_failure_taxonomy_members() {
    let document = target("integer", "");
    let digest = document.contract_digest_sha256().expect("digest");
    let mut state = StateDocument::new(TargetId::new("demo").expect("target"), digest);
    state
        .apply_source_suspect(
            SourceSuspectReason::JsonMalformed,
            ProcessErrorDetail::new(ProcessErrorKind::Json, "detail", None),
            "2026-07-15T00:00:00Z",
            3,
        )
        .expect("source health");
    let mut wire = serde_json::to_value(&state).expect("state JSON");
    wire["source_health"]["reason_class"] = serde_json::json!("invented_reason");
    assert!(serde_json::from_value::<StateDocument>(wire).is_err());

    let mut permanent_state = StateDocument::new(
        TargetId::new("demo").expect("target"),
        document.contract_digest_sha256().expect("digest"),
    );
    permanent_state
        .apply_permanent_error(
            PermanentErrorCode::InvalidJsonPointer,
            "2026-07-15T00:00:00Z",
        )
        .expect("permanent error");
    let mut wire = serde_json::to_value(permanent_state).expect("state JSON");
    wire["permanent_error_episode"]["error_code"] = serde_json::json!("invented_permanent_error");
    assert!(serde_json::from_value::<StateDocument>(wire).is_err());
}

#[test]
fn html_observations_retain_complete_htmlcut_evidence_and_reject_incoherent_persisted_facts() {
    use htmlcut_core::interop::v1::{
        InteropDiagnostic, InteropDiagnosticCode, InteropDiagnosticLevel,
    };

    let diagnostics = [
        (InteropDiagnosticLevel::Error, "error"),
        (InteropDiagnosticLevel::Warning, "warning"),
        (InteropDiagnosticLevel::Info, "info"),
    ]
    .into_iter()
    .map(|(level, expected_level)| {
        let diagnostic = HtmlcutDiagnostic::from_interop(InteropDiagnostic {
            level,
            code: InteropDiagnosticCode::MultipleMatches,
            message: format!("{expected_level} diagnostic"),
            details: Some(serde_json::json!({"candidateCount": 2})),
        });
        assert_eq!(diagnostic.level(), expected_level);
        assert_eq!(diagnostic.code(), "MULTIPLE_MATCHES");
        assert_eq!(diagnostic.message(), format!("{expected_level} diagnostic"));
        assert_eq!(
            diagnostic.details(),
            Some(&serde_json::json!({"candidateCount": 2}))
        );
        diagnostic
    })
    .collect::<Vec<_>>();

    let html_target_document = html_target("html_text", "article", None, "decimal", "");
    let observation = html_target_document
        .parse_html_projection(HtmlObservationInput {
            raw_selected: " 1.00 ".to_owned(),
            comparison_projection: "1.00".to_owned(),
            acquisition_kind: AcquisitionKind::HtmlText,
            plan_digest_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
            candidate_count: 2,
            diagnostics: diagnostics.clone(),
        })
        .expect("HTML projection parses through the target type contract");
    assert_eq!(observation.raw_selected(), " 1.00 ");
    assert_eq!(observation.comparison_projection(), "1.00");
    assert_eq!(observation.acquisition_kind(), AcquisitionKind::HtmlText);
    assert_eq!(observation.canonical_value(), "1");
    assert_eq!(
        observation.htmlcut_semantics_version(),
        Some(htmlcut_core::interop::v1::HTMLCUT_EXTRACTION_SEMANTICS_VERSION)
    );
    assert_eq!(
        observation.plan_digest_sha256(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert_eq!(observation.htmlcut_candidate_count(), Some(2));
    assert_eq!(observation.htmlcut_diagnostics(), diagnostics.as_slice());
    observation.validate().expect("coherent HTML observation");

    assert_eq!(
        html_target_document
            .parse_html_projection(HtmlObservationInput {
                raw_selected: "not a decimal".to_owned(),
                comparison_projection: "not a decimal".to_owned(),
                acquisition_kind: AcquisitionKind::HtmlText,
                plan_digest_sha256:
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                candidate_count: 1,
                diagnostics: Vec::new(),
            })
            .expect_err("HTML type failures are value-unparseable")
            .kind(),
        ProcessErrorKind::ValueUnparseable
    );

    for mutate in [
        |wire: &mut serde_json::Value| {
            wire["acquisition_kind"] = serde_json::json!("json_pointer");
            wire["raw_selected"] = serde_json::json!("1.00");
        },
        |wire: &mut serde_json::Value| {
            wire["htmlcut_semantics_version"] = serde_json::json!(999);
        },
        |wire: &mut serde_json::Value| {
            wire["plan_digest_sha256"] = serde_json::json!("not-a-digest");
        },
        |wire: &mut serde_json::Value| {
            wire["htmlcut_candidate_count"] = serde_json::json!(0);
        },
    ] {
        let mut wire = serde_json::to_value(&observation).expect("observation JSON");
        mutate(&mut wire);
        let invalid: Observation = serde_json::from_value(wire).expect("structurally valid JSON");
        assert!(invalid.validate().is_err());
    }

    let json_observation = target("integer", "")
        .parse_json_scalar_token("1".to_owned())
        .expect("JSON observation");
    for (field, value) in [
        ("htmlcut_semantics_version", serde_json::json!(1)),
        (
            "plan_digest_sha256",
            serde_json::json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        ),
        ("htmlcut_candidate_count", serde_json::json!(1)),
        (
            "htmlcut_diagnostics",
            serde_json::json!([{
                "level": "info",
                "code": "MULTIPLE_MATCHES",
                "message": "unexpected JSON evidence"
            }]),
        ),
    ] {
        let mut wire = serde_json::to_value(&json_observation).expect("observation JSON");
        wire[field] = value;
        let invalid: Observation = serde_json::from_value(wire).expect("structurally valid JSON");
        assert!(invalid.validate().is_err(), "{field}");
    }
}

#[test]
fn html_selection_and_error_classification_cover_the_complete_public_contract() {
    use htmlcut_core::interop::v1::{ErrorCode, InteropError};

    let selection_target = html_target("html_text", "article", None, "integer", "");
    let Projection::HtmlText { selection } = selection_target.projection() else {
        panic!("fixture must produce an HTML text projection");
    };
    assert_eq!(
        selection.strategy().kind(),
        htmlcut_core::interop::v1::StrategyKind::CssSelector
    );
    assert!(matches!(
        selection.selection(),
        htmlcut_core::interop::v1::Selection::Single
    ));
    assert!(matches!(
        selection.rendering(),
        htmlcut_core::interop::v1::Rendering { .. }
    ));
    assert_eq!(
        selection.structured_plan().output.kind(),
        htmlcut_core::interop::v1::OutputKind::Structured
    );

    for (diagnostic_code, expected) in [
        (
            "INVALID_SELECTOR",
            PermanentErrorCode::HtmlcutInvalidSelector,
        ),
        (
            "INVALID_SLICE_PATTERN",
            PermanentErrorCode::HtmlcutInvalidSlicePattern,
        ),
        (
            "UNSUPPORTED_VALUE_TYPE",
            PermanentErrorCode::HtmlcutUnsupportedValueType,
        ),
    ] {
        let mut details = std::collections::BTreeMap::new();
        details.insert(
            "core_diagnostic_code".to_owned(),
            serde_json::json!(diagnostic_code),
        );
        let error = InteropError::new(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ErrorCode::PlanInvalid,
            "invalid plan",
            None,
            details,
            Vec::new(),
        );
        assert_eq!(permanent_code_for_htmlcut_error(&error), expected);
    }
    let fallback = InteropError::new(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ErrorCode::NoMatch,
        "unexpected permanent classifier fallback",
        None,
        std::collections::BTreeMap::new(),
        Vec::new(),
    );
    assert_eq!(
        permanent_code_for_htmlcut_error(&fallback),
        PermanentErrorCode::HtmlcutPlanInvalid
    );

    let invalid_plan = mutate_target(&selection_target, |wire| {
        wire["projection"]["selection"]["strategy"] = serde_json::json!({
            "kind": "delimiter_pair",
            "start": "<start>",
            "end": "<end>",
            "mode": "literal",
            "boundary_retention": "exclude_both",
            "flags": ["case_insensitive"]
        });
    });
    assert_eq!(
        invalid_plan.permanent_error().map(|error| error.code()),
        Some(PermanentErrorCode::HtmlcutPlanInvalid)
    );
}

#[test]
fn htmlcut_failure_detail_accessors_preserve_all_failure_evidence() {
    use htmlcut_core::interop::v1::{ErrorCode, InteropError};

    let failure = HtmlcutFailureDetails::new(
        "NO_MATCH".to_owned(),
        Some(2),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        Vec::new(),
    );
    assert_eq!(failure.reason(), "NO_MATCH");
    assert_eq!(failure.candidate_count(), Some(2));
    assert_eq!(
        failure.plan_digest_sha256(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert!(failure.diagnostics().is_empty());

    let detail = ProcessErrorDetail::new(ProcessErrorKind::Htmlcut, "no match", None)
        .with_htmlcut_failure(failure);
    assert_eq!(
        detail.htmlcut_failure().map(HtmlcutFailureDetails::reason),
        Some("NO_MATCH")
    );

    let nested_error = InteropError::new(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ErrorCode::NoMatch,
        "no match",
        None,
        std::collections::BTreeMap::from([(
            "nested".to_owned(),
            serde_json::json!({"outer": [{"candidate_count": 7}]}),
        )]),
        Vec::new(),
    );
    let nested_failure = HtmlcutFailureDetails::from_interop_error(&nested_error);
    assert_eq!(nested_failure.reason(), "no_match");
    assert_eq!(nested_failure.candidate_count(), Some(7));

    let no_count_error = InteropError::new(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ErrorCode::NoMatch,
        "no match",
        None,
        std::collections::BTreeMap::from([(
            "unrelated".to_owned(),
            serde_json::json!(["not a count"]),
        )]),
        Vec::new(),
    );
    assert_eq!(
        HtmlcutFailureDetails::from_interop_error(&no_count_error).candidate_count(),
        None
    );
}

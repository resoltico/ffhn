use super::*;

#[test]
fn agent_document_deserialization_enforces_its_current_closed_envelope() {
    let document: AgentDocument = serde_json::from_value(serde_json::json!({
        "schema_name": AGENT_SCHEMA_NAME,
        "schema_version": AGENT_SCHEMA_VERSION
    }))
    .expect("current agent document");
    assert_eq!(document, AgentDocument::new());
    for invalid in [
        serde_json::json!({"schema_name": "foreign.agent", "schema_version": 1}),
        serde_json::json!({"schema_name": AGENT_SCHEMA_NAME, "schema_version": 2}),
        serde_json::json!({"schema_name": AGENT_SCHEMA_NAME, "schema_version": 1, "extra": true}),
    ] {
        assert!(serde_json::from_value::<AgentDocument>(invalid).is_err());
    }
}

const HTTP_SOURCE: &str = r#"
schema_name = "ffhn.source"
schema_version = 1
source_id = "shop"
display_name = "Shop"
enabled = true
escalate_after = 2
[fetch]
engine = "http"
source_url = "https://example.test/product"
user_agent = "ffhn/11"
accept = "text/html"
max_bytes = 1024
follow_redirects = true
max_redirects = 5
[fetch.timeouts]
connect_ms = 1
read_idle_ms = 1
total_ms = 1
[fetch.headers]
Accept-Language = "en"
[fetch.header_secrets]
Authorization = { env = "TOKEN", format = "Bearer {value}", revision = 1 }
[conditional]
enabled = true
[schedule]
interval_ms = 100
min_interval_ms = 50
"#;

#[test]
fn source_document_rejects_header_boundary_and_schedule_violations() {
    let document: SourceDocument = toml::from_str(HTTP_SOURCE).expect("source document");
    assert_eq!(document.source_id().as_str(), "shop");
    assert!(document.fetch().is_http());

    for replacement in [
        "Host = \"example.test\"",
        "Authorization = \"Bearer plain\"",
        "User-Agent = \"other\"",
    ] {
        let invalid = HTTP_SOURCE.replace("Accept-Language = \"en\"", replacement);
        assert!(
            toml::from_str::<SourceDocument>(&invalid).is_err(),
            "{replacement}"
        );
    }
    let invalid_schedule = HTTP_SOURCE.replace("interval_ms = 100", "interval_ms = 49");
    assert!(toml::from_str::<SourceDocument>(&invalid_schedule).is_err());
    let invalid_secret = HTTP_SOURCE.replace("Bearer {value}", "Bearer value");
    assert!(toml::from_str::<SourceDocument>(&invalid_secret).is_err());
    let invalid_fixed = HTTP_SOURCE.replace("ffhn/11", "ffhn/11\\nInjected: value");
    assert!(toml::from_str::<SourceDocument>(&invalid_fixed).is_err());
    let invalid_literal = HTTP_SOURCE.replace(
        "Accept-Language = \"en\"",
        "X-Test = \"ok\\nInjected: value\"",
    );
    assert!(toml::from_str::<SourceDocument>(&invalid_literal).is_err());
}

#[test]
fn source_representation_digest_normalizes_header_names_and_excludes_operational_values() {
    let base: SourceDocument = toml::from_str(HTTP_SOURCE).expect("base source");
    let normalized: SourceDocument =
        toml::from_str(&HTTP_SOURCE.replace("Accept-Language", "accept-language"))
            .expect("normalized source");
    let operational: SourceDocument = toml::from_str(
        &HTTP_SOURCE
            .replace("max_bytes = 1024", "max_bytes = 2048")
            .replace("interval_ms = 100", "interval_ms = 200"),
    )
    .expect("operational source");
    assert_eq!(
        base.source_representation_digest().expect("base digest"),
        normalized
            .source_representation_digest()
            .expect("normalized digest")
    );
    assert_eq!(
        base.source_representation_digest().expect("base digest"),
        operational
            .source_representation_digest()
            .expect("operational digest")
    );
}

#[test]
fn source_and_agent_documents_cover_complete_public_contract_and_validation_families() {
    let agent = AgentDocument::new();
    agent.validate().expect("current agent");
    assert_eq!(AgentDocument::default(), agent);
    for (field, value) in [
        ("schema_name", serde_json::json!("foreign.agent")),
        ("schema_version", serde_json::json!(2)),
    ] {
        let mut wire = serde_json::to_value(&agent).expect("agent wire");
        wire[field] = value;
        assert!(serde_json::from_value::<AgentDocument>(wire).is_err());
    }

    let document: SourceDocument = toml::from_str(HTTP_SOURCE).expect("source");
    assert_eq!(document.display_name(), "Shop");
    assert!(document.enabled());
    assert_eq!(document.escalate_after(), 2);
    assert!(document.conditional_enabled());
    let unconditional: SourceDocument = toml::from_str(&HTTP_SOURCE.replace(
        "[conditional]\nenabled = true",
        "[conditional]\nenabled = false",
    ))
    .expect("unconditional source");
    assert!(!unconditional.conditional_enabled());
    toml::from_str::<SourceDocument>(
        &HTTP_SOURCE.replace("max_redirects = 5", "max_redirects = 20"),
    )
    .expect("maximum redirect count");
    assert_eq!(document.schedule().interval_ms(), 100);
    assert_eq!(document.schedule().min_interval_ms(), 50);
    assert!(document.outbox().is_none());
    assert!(document.routes().is_empty());

    let base = serde_json::to_value(&document).expect("source wire");
    for (pointer, value) in [
        ("/schema_name", serde_json::json!("foreign.source")),
        ("/schema_version", serde_json::json!(2)),
        ("/display_name", serde_json::json!(" ")),
        ("/escalate_after", serde_json::json!(0)),
        (
            "/fetch/source_url",
            serde_json::json!("ftp://example.test/a"),
        ),
        (
            "/fetch/source_url",
            serde_json::json!("https://user@example.test/a"),
        ),
        (
            "/fetch/source_url",
            serde_json::json!("https://:pass@example.test/a"),
        ),
        ("/fetch/user_agent", serde_json::json!(" ")),
        ("/fetch/accept", serde_json::json!(" ")),
        ("/fetch/max_bytes", serde_json::json!(1023)),
        ("/fetch/max_bytes", serde_json::json!(104_857_601usize)),
        ("/fetch/max_redirects", serde_json::json!(21)),
        ("/fetch/timeouts/connect_ms", serde_json::json!(0)),
        ("/fetch/timeouts/read_idle_ms", serde_json::json!(0)),
        ("/fetch/timeouts/total_ms", serde_json::json!(0)),
        ("/schedule/min_interval_ms", serde_json::json!(0)),
        ("/schedule/interval_ms", serde_json::json!(49)),
    ] {
        let mut wire = base.clone();
        *wire.pointer_mut(pointer).expect("pointer") = value;
        assert!(
            serde_json::from_value::<SourceDocument>(wire).is_err(),
            "{pointer}"
        );
    }

    let file_document: SourceDocument = toml::from_str(&format!(
        r#"
schema_name = "ffhn.source"
schema_version = 1
source_id = "file"
display_name = "File"
enabled = true
escalate_after = 1
[fetch]
engine = "file"
file_path = {:?}
max_bytes = 1024
[conditional]
enabled = false
[schedule]
interval_ms = 100
min_interval_ms = 100
"#,
        crate::graph::test_support::absolute_file_path("source.json")
    ))
    .expect("file source");
    assert!(!file_document.fetch().is_http());
    assert_ne!(
        file_document
            .source_representation_digest()
            .expect("file digest"),
        document
            .source_representation_digest()
            .expect("HTTP digest")
    );
    let file_base = serde_json::to_value(&file_document).expect("file wire");
    for (pointer, value) in [
        ("/fetch/file_path", serde_json::json!("relative.json")),
        ("/fetch/max_bytes", serde_json::json!(0)),
        ("/conditional/enabled", serde_json::json!(true)),
    ] {
        let mut wire = file_base.clone();
        *wire.pointer_mut(pointer).expect("pointer") = value;
        assert!(serde_json::from_value::<SourceDocument>(wire).is_err());
    }
}

#[test]
fn header_tables_reject_every_owned_secret_and_grammar_boundary() {
    let mut fixed = BTreeMap::new();
    let mut secrets = BTreeMap::new();
    fixed.insert("X-Test".to_owned(), "ok".to_owned());
    validate_header_tables(&fixed, &secrets).expect("valid fixed header");

    for name in [
        "bad header",
        "Host",
        "Content-Length",
        "Range",
        "If-None-Match",
        "If-Modified-Since",
        "If-Match",
        "If-Unmodified-Since",
        "If-Range",
        "Connection",
        "Proxy-Connection",
        "Keep-Alive",
        "TE",
        "Trailer",
        "Transfer-Encoding",
        "Upgrade",
        "Accept",
        "User-Agent",
    ] {
        let mut candidate = BTreeMap::new();
        candidate.insert(name.to_owned(), "value".to_owned());
        assert!(
            validate_header_tables(&candidate, &BTreeMap::new()).is_err(),
            "{name}"
        );
    }
    for name in ["Authorization", "Proxy-Authorization", "Cookie"] {
        let mut candidate = BTreeMap::new();
        candidate.insert(name.to_owned(), "value".to_owned());
        assert!(
            validate_header_tables(&candidate, &BTreeMap::new()).is_err(),
            "{name}"
        );
    }
    for value in [" ", "line\rbreak"] {
        let mut candidate = BTreeMap::new();
        candidate.insert("X-Test".to_owned(), value.to_owned());
        assert!(validate_header_tables(&candidate, &BTreeMap::new()).is_err());
    }
    fixed.insert("x-test".to_owned(), "other".to_owned());
    assert!(validate_header_tables(&fixed, &BTreeMap::new()).is_err());

    let valid_secret = FetchHeaderSecret {
        env: "TOKEN".to_owned(),
        format: "Bearer {value}".to_owned(),
        revision: 1,
    };
    secrets.insert("Authorization".to_owned(), valid_secret.clone());
    validate_header_tables(&BTreeMap::new(), &secrets).expect("valid secret");
    secrets.insert("authorization".to_owned(), valid_secret.clone());
    assert!(validate_header_tables(&BTreeMap::new(), &secrets).is_err());
    secrets.remove("authorization");
    let mut collision = BTreeMap::new();
    collision.insert("authorization".to_owned(), "fixed".to_owned());
    assert!(validate_header_tables(&collision, &secrets).is_err());
    for secret in [
        FetchHeaderSecret {
            env: " ".to_owned(),
            ..valid_secret.clone()
        },
        FetchHeaderSecret {
            format: "no slot".to_owned(),
            ..valid_secret.clone()
        },
        FetchHeaderSecret {
            format: "{value}{value}".to_owned(),
            ..valid_secret.clone()
        },
        FetchHeaderSecret {
            revision: 0,
            ..valid_secret.clone()
        },
        FetchHeaderSecret {
            format: "{value}\nInjected: yes".to_owned(),
            ..valid_secret
        },
    ] {
        let mut candidate = BTreeMap::new();
        candidate.insert("Authorization".to_owned(), secret);
        assert!(validate_header_tables(&BTreeMap::new(), &candidate).is_err());
    }
}

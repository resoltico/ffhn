use super::*;
use crate::graph::SourceDocumentBytes;

#[test]
fn unchanged_file_skips_projection_unless_document_required() {
    let file_path = crate::graph::test_support::absolute_file_path("source.json");
    let source: SourceDocument = toml::from_str(&format!(
        r#"
schema_name = "ffhn.source"
schema_version = 1
source_id = "file"
display_name = "File"
enabled = true
escalate_after = 2
[fetch]
engine = "file"
file_path = {file_path:?}
max_bytes = 1024
[conditional]
enabled = false
[schedule]
interval_ms = 100
min_interval_ms = 100
"#,
    ))
    .expect("source");
    let digest = source.source_representation_digest().expect("SRD");
    let content = "a".repeat(64);
    let state = SourceState::fresh(super::super::SourceInstanceId::mint())
        .with_representation_facts(None, None, None, Some(content.clone()), digest)
        .expect("state facts");
    let document = SourceDocumentBytes {
        body: "{}".to_owned(),
        effective_http_url: None,
        file_content_sha256: Some(content),
        validators: None,
    };
    assert_eq!(
        decide_source_cycle(
            &source,
            Some(&state),
            SourceAcquisition::Document(document.clone()),
            false,
        )
        .expect("unchanged file"),
        SourceCycleDecision::NotModified { validators: None }
    );
    assert_eq!(
        decide_source_cycle(
            &source,
            Some(&state),
            SourceAcquisition::Document(document),
            true,
        )
        .expect("required document"),
        SourceCycleDecision::Document(Box::new(SourceDocumentBytes {
            body: "{}".to_owned(),
            effective_http_url: None,
            file_content_sha256: Some("a".repeat(64)),
            validators: None,
        }))
    );

    let validators = super::super::HttpValidators {
        issued_url: url::Url::parse("https://example.test/value").expect("URL"),
        etag: Some("\"v1\"".to_owned()),
        last_modified: None,
    };
    assert_eq!(
        decide_source_cycle(
            &source,
            None,
            SourceAcquisition::NotModified(validators.clone()),
            false,
        )
        .expect("304"),
        SourceCycleDecision::NotModified {
            validators: Some(validators)
        }
    );
    let changed = SourceDocumentBytes {
        body: "changed".to_owned(),
        effective_http_url: None,
        file_content_sha256: Some("b".repeat(64)),
        validators: None,
    };
    assert!(matches!(
        decide_source_cycle(
            &source,
            Some(&state),
            SourceAcquisition::Document(changed),
            false,
        )
        .expect("changed"),
        SourceCycleDecision::Document(_)
    ));
}

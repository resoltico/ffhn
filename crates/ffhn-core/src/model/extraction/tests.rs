use super::*;
use crate::{
    EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION, HTMLCUT_INTEROP_PROFILE,
};
use serde_json::json;
use std::collections::BTreeMap;

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn valid_record() -> ExtractionRecord {
    ExtractionRecord {
        schema_name: EXTRACTION_RECORD_SCHEMA_NAME.to_owned(),
        schema_version: EXTRACTION_RECORD_SCHEMA_VERSION,
        interop_profile: HTMLCUT_INTEROP_PROFILE.to_owned(),
        htmlcut_plan_digest_sha256: DIGEST.to_owned(),
        htmlcut_result_digest_sha256: DIGEST.to_owned(),
        comparison_input_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        strategy_kind: SelectionKind::CssSelector,
        selection_mode: SelectionMatch::Single,
        output_kind: OutputKind::OuterHtml,
        candidate_count: 1,
        selected_candidate_index: 1,
        match_metadata: json!({"selector": "main"}),
        warning_codes: vec!["warn".to_owned()],
        created_at: "2026-04-05T10:15:30Z".to_owned(),
        extensions: None,
    }
}

#[test]
fn extraction_record_validation_accepts_the_canonical_shape() {
    valid_record().validate().expect("record");
}

#[test]
fn extraction_and_snapshot_accessors_expose_the_public_contract() {
    let mut record = valid_record();
    record.extensions = Some(BTreeMap::from([(
        "demo".to_owned(),
        json!({"kind": "ext"}),
    )]));

    assert_eq!(record.schema_name(), EXTRACTION_RECORD_SCHEMA_NAME);
    assert_eq!(record.schema_version(), EXTRACTION_RECORD_SCHEMA_VERSION);
    assert_eq!(record.interop_profile(), HTMLCUT_INTEROP_PROFILE);
    assert_eq!(record.htmlcut_plan_digest_sha256(), DIGEST);
    assert_eq!(record.htmlcut_result_digest_sha256(), DIGEST);
    assert_eq!(record.comparison_input_sha256(), DIGEST);
    assert_eq!(record.outer_html_sha256(), DIGEST);
    assert_eq!(record.strategy_kind(), SelectionKind::CssSelector);
    assert_eq!(record.selection_mode(), SelectionMatch::Single);
    assert_eq!(record.output_kind(), OutputKind::OuterHtml);
    assert_eq!(record.candidate_count(), 1);
    assert_eq!(record.selected_candidate_index(), 1);
    assert_eq!(record.match_metadata(), &json!({"selector": "main"}));
    assert_eq!(record.warning_codes(), ["warn"]);
    assert_eq!(record.created_at(), "2026-04-05T10:15:30Z");
    assert_eq!(
        record.extensions().expect("extensions").get("demo"),
        Some(&json!({"kind": "ext"}))
    );

    let snapshot = SnapshotReference {
        slot: SnapshotSlot::Current,
        canonical_text_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        extraction_record_path: RelativeArtifactPath::new("snapshots/current/extraction.json")
            .expect("relative path"),
        canonical_text_path: RelativeArtifactPath::new("snapshots/current/canonical.txt")
            .expect("relative path"),
        outer_html_path: RelativeArtifactPath::new("snapshots/current/outer.html")
            .expect("relative path"),
        captured_at: "2026-04-05T10:15:30Z".to_owned(),
    };
    assert_eq!(snapshot.slot(), SnapshotSlot::Current);
    assert_eq!(snapshot.canonical_text_sha256(), DIGEST);
    assert_eq!(snapshot.outer_html_sha256(), DIGEST);
    assert_eq!(
        snapshot.extraction_record_path().as_str(),
        "snapshots/current/extraction.json"
    );
    assert_eq!(
        snapshot.canonical_text_path().as_str(),
        "snapshots/current/canonical.txt"
    );
    assert_eq!(
        snapshot.outer_html_path().as_str(),
        "snapshots/current/outer.html"
    );
    assert_eq!(snapshot.captured_at(), "2026-04-05T10:15:30Z");
}

#[test]
fn extraction_record_validation_rejects_invalid_contract_data() {
    let mut record = valid_record();
    record.interop_profile = "other".to_owned();
    assert!(record.validate().is_err());

    let mut record = valid_record();
    record.htmlcut_plan_digest_sha256 = "bad".to_owned();
    assert!(record.validate().is_err());

    let mut record = valid_record();
    record.htmlcut_result_digest_sha256 = "bad".to_owned();
    assert!(record.validate().is_err());

    let mut record = valid_record();
    record.candidate_count = 0;
    assert!(record.validate().is_err());

    let mut record = valid_record();
    record.selected_candidate_index = 0;
    assert!(record.validate().is_err());

    let mut record = valid_record();
    record.selected_candidate_index = 2;
    assert!(record.validate().is_err());

    let mut record = valid_record();
    record.match_metadata = json!(["not", "an", "object"]);
    assert!(record.validate().is_err());

    let mut record = valid_record();
    record.created_at = "bad".to_owned();
    assert!(record.validate().is_err());
}

#[test]
fn snapshot_reference_validation_checks_digests_paths_and_timestamp() {
    SnapshotReference {
        slot: SnapshotSlot::Current,
        canonical_text_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        extraction_record_path: RelativeArtifactPath::new("snapshots/current/extraction.json")
            .expect("relative path"),
        canonical_text_path: RelativeArtifactPath::new("snapshots/current/canonical.txt")
            .expect("relative path"),
        outer_html_path: RelativeArtifactPath::new("snapshots/current/outer.html")
            .expect("relative path"),
        captured_at: "2026-04-05T10:15:30Z".to_owned(),
    }
    .validate()
    .expect("snapshot reference");

    assert!(
        serde_json::from_str::<SnapshotReference>(
            r#"{
                "slot":"current",
                "canonical_text_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "outer_html_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "extraction_record_path":"../escape",
                "canonical_text_path":"snapshots/current/canonical.txt",
                "outer_html_path":"snapshots/current/outer.html",
                "captured_at":"2026-04-05T10:15:30Z"
            }"#
        )
        .is_err()
    );
}

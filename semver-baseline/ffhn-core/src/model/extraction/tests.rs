use super::*;
use crate::{EXTRACTION_RECORD_SCHEMA_NAME, EXTRACTION_RECORD_SCHEMA_VERSION};
use serde_json::json;
use std::collections::BTreeMap;

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn css_evidence() -> SelectionEvidence {
    SelectionEvidence::CssSelector {
        path: "html > body > main".to_owned(),
        tag_name: "main".to_owned(),
    }
}

fn delimiter_evidence() -> SelectionEvidence {
    SelectionEvidence::DelimiterPair {
        selected_range: SelectionRange {
            start_byte: 10,
            end_byte: 20,
        },
        inner_range: SelectionRange {
            start_byte: 11,
            end_byte: 19,
        },
        outer_range: SelectionRange {
            start_byte: 8,
            end_byte: 22,
        },
        include_start: true,
        include_end: true,
    }
}

fn valid_record() -> ExtractionRecord {
    ExtractionRecord {
        schema_name: EXTRACTION_RECORD_SCHEMA_NAME.to_owned(),
        schema_version: EXTRACTION_RECORD_SCHEMA_VERSION,
        compare_source_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        selection_kind: SelectionKind::CssSelector,
        selection_match: SelectionMatch::Single,
        compare_basis: CompareBasis::Text,
        candidate_count: 1,
        selected_candidate_index: 1,
        selection_evidence: css_evidence(),
        warning_codes: vec!["warn".to_owned()],
        created_at: "2026-04-05T10:15:30Z".to_owned(),
        monitoring_contract_digest_sha256: DIGEST.to_owned(),
        extensions: None,
    }
}

#[test]
fn extraction_record_validation_accepts_the_canonical_shape() {
    valid_record().validate().expect("record");

    ExtractionRecord {
        selection_kind: SelectionKind::DelimiterPair,
        selection_match: SelectionMatch::First,
        selection_evidence: delimiter_evidence(),
        ..valid_record()
    }
    .validate()
    .expect("delimiter record");
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
    assert_eq!(record.compare_source_sha256(), DIGEST);
    assert_eq!(record.outer_html_sha256(), DIGEST);
    assert_eq!(record.selection_kind(), SelectionKind::CssSelector);
    assert_eq!(record.selection_match(), SelectionMatch::Single);
    assert_eq!(record.compare_basis(), CompareBasis::Text);
    assert_eq!(record.candidate_count(), 1);
    assert_eq!(record.selected_candidate_index(), 1);
    assert_eq!(
        record.selection_evidence().kind(),
        SelectionKind::CssSelector
    );
    assert_eq!(record.warning_codes(), ["warn"]);
    assert_eq!(record.created_at(), "2026-04-05T10:15:30Z");
    assert_eq!(record.monitoring_contract_digest_sha256(), DIGEST);
    assert_eq!(
        record.extensions().expect("extensions").get("demo"),
        Some(&json!({"kind": "ext"}))
    );

    match record.selection_evidence() {
        SelectionEvidence::CssSelector { path, tag_name } => {
            assert_eq!(path, "html > body > main");
            assert_eq!(tag_name, "main");
        }
        other => panic!("expected css evidence, got {other:?}"),
    }

    let delimiter_record = ExtractionRecord {
        selection_kind: SelectionKind::DelimiterPair,
        selection_match: SelectionMatch::First,
        selection_evidence: delimiter_evidence(),
        ..valid_record()
    };
    assert_eq!(
        delimiter_record.selection_evidence().kind(),
        SelectionKind::DelimiterPair
    );
    match delimiter_record.selection_evidence() {
        SelectionEvidence::DelimiterPair {
            selected_range,
            inner_range,
            outer_range,
            include_start,
            include_end,
        } => {
            assert_eq!(selected_range.start_byte(), 10);
            assert_eq!(selected_range.end_byte(), 20);
            assert_eq!(inner_range.start_byte(), 11);
            assert_eq!(inner_range.end_byte(), 19);
            assert_eq!(outer_range.start_byte(), 8);
            assert_eq!(outer_range.end_byte(), 22);
            assert!(*include_start);
            assert!(*include_end);
        }
        other => panic!("expected delimiter evidence, got {other:?}"),
    }

    let snapshot = SnapshotReference {
        slot: SnapshotSlot::Current,
        compare_digest_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        extraction_record_path: RelativeArtifactPath::new("snapshots/current/extraction.json")
            .expect("relative path"),
        compare_path: RelativeArtifactPath::new("snapshots/current/compare.txt")
            .expect("relative path"),
        outer_html_path: RelativeArtifactPath::new("snapshots/current/outer.html")
            .expect("relative path"),
        captured_at: "2026-04-05T10:15:30Z".to_owned(),
    };
    assert_eq!(snapshot.slot(), SnapshotSlot::Current);
    assert_eq!(snapshot.compare_digest_sha256(), DIGEST);
    assert_eq!(snapshot.outer_html_sha256(), DIGEST);
    assert_eq!(
        snapshot.extraction_record_path().as_str(),
        "snapshots/current/extraction.json"
    );
    assert_eq!(
        snapshot.compare_path().as_str(),
        "snapshots/current/compare.txt"
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
    record.compare_source_sha256 = "bad".to_owned();
    assert!(record.validate().is_err());

    let mut record = valid_record();
    record.outer_html_sha256 = "bad".to_owned();
    assert!(record.validate().is_err());

    let mut record = valid_record();
    record.monitoring_contract_digest_sha256 = "bad".to_owned();
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
    record.selection_evidence = SelectionEvidence::DelimiterPair {
        selected_range: SelectionRange {
            start_byte: 0,
            end_byte: 1,
        },
        inner_range: SelectionRange {
            start_byte: 0,
            end_byte: 1,
        },
        outer_range: SelectionRange {
            start_byte: 0,
            end_byte: 1,
        },
        include_start: false,
        include_end: false,
    };
    assert!(record.validate().is_err());

    let mut record = valid_record();
    record.selection_kind = SelectionKind::DelimiterPair;
    assert!(record.validate().is_err());

    let mut record = ExtractionRecord {
        selection_kind: SelectionKind::DelimiterPair,
        selection_match: SelectionMatch::Nth,
        selection_evidence: SelectionEvidence::DelimiterPair {
            selected_range: SelectionRange {
                start_byte: 10,
                end_byte: 8,
            },
            inner_range: SelectionRange {
                start_byte: 11,
                end_byte: 19,
            },
            outer_range: SelectionRange {
                start_byte: 8,
                end_byte: 22,
            },
            include_start: true,
            include_end: true,
        },
        ..valid_record()
    };
    assert!(record.validate().is_err());

    record = ExtractionRecord {
        selection_kind: SelectionKind::CssSelector,
        selection_evidence: SelectionEvidence::CssSelector {
            path: String::new(),
            tag_name: "main".to_owned(),
        },
        ..valid_record()
    };
    assert!(record.validate().is_err());

    let mut record = valid_record();
    record.created_at = "bad".to_owned();
    assert!(record.validate().is_err());
}

#[test]
fn snapshot_reference_validation_checks_digests_paths_and_timestamp() {
    SnapshotReference {
        slot: SnapshotSlot::Current,
        compare_digest_sha256: DIGEST.to_owned(),
        outer_html_sha256: DIGEST.to_owned(),
        extraction_record_path: RelativeArtifactPath::new("snapshots/current/extraction.json")
            .expect("relative path"),
        compare_path: RelativeArtifactPath::new("snapshots/current/compare.txt")
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
                "compare_digest_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "outer_html_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "extraction_record_path":"../escape",
                "compare_path":"snapshots/current/compare.txt",
                "outer_html_path":"snapshots/current/outer.html",
                "captured_at":"2026-04-05T10:15:30Z"
            }"#
        )
        .is_err()
    );
}

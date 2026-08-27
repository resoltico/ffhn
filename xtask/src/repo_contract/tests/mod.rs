use std::fs;
use std::path::Path;

use super::*;

#[test]
fn public_docs_describe_only_current_observation_graph_contracts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let targets = fs::read_to_string(root.join("docs/targets.md")).expect("targets documentation");
    assert!(targets.contains("JSON Pointer"));
    assert!(targets.contains("ffhn.source"));
    assert!(targets.contains("ffhn.measurement"));
    assert!(targets.contains("source_id"));
    assert!(targets.contains("measurement_id"));
    assert!(targets.contains("HTMLCut"));
    assert!(targets.contains("html_text"));
    assert!(targets.contains("html_rendered_text"));
    assert!(targets.contains("html_attribute"));
    assert!(targets.contains("plain DOM descendant text"));
    assert!(targets.contains("dom_canonicalization"));
    assert!(targets.contains("require a CSS selector"));
    assert!(targets.contains("rejects it as inert configuration"));
    assert!(targets.contains("exact"));
    assert!(targets.contains("reset --source"));
    assert!(!targets.contains("notification_endpoints"));

    let contracts =
        fs::read_to_string(root.join("docs/contracts.md")).expect("contracts documentation");
    let contracts = normalize_whitespace(&contracts);
    for schema in [
        "ffhn.agent",
        "ffhn.graph_identity",
        "ffhn.source",
        "ffhn.source_identity",
        "ffhn.measurement",
        "ffhn.source_state",
        "ffhn.measurement_state",
        "ffhn.commit_manifest",
        "ffhn.lineage_manifest",
        "ffhn.delivery_record",
        "ffhn.dead_letter",
        "ffhn.event_envelope",
        "ffhn.measure_report",
        "ffhn.agent_tick_report",
        "ffhn.agent_status_report",
        "ffhn.source_status_report",
        "ffhn.measurement_status_report",
        "ffhn.reset_report",
        "ffhn.validate_report",
        "ffhn.list_report",
        "ffhn.new_report",
    ] {
        assert!(contracts.contains(schema), "missing {schema}");
    }
    for retired in [
        "ffhn.target",
        "ffhn.state`",
        "ffhn.run_report",
        "ffhn.batch_run_report",
        "ffhn.process_stdin",
    ] {
        assert!(!contracts.contains(retired), "retired contract {retired}");
    }

    let reports = fs::read_to_string(root.join("docs/reports.md")).expect("reports documentation");
    let reports = normalize_whitespace(&reports);
    assert!(reports.contains("ffhn.measure_report"));
    assert!(reports.contains("ffhn.source_status_report"));
    assert!(reports.contains("ffhn.measurement_status_report"));
    assert!(reports.contains("route-independent"));

    for path in [
        "docs/README.md",
        "docs/architecture.md",
        "docs/cli.md",
        "docs/contracts.md",
        "docs/core.md",
        "docs/getting-started.md",
        "docs/reports.md",
        "docs/targets.md",
        "docs/versioning-policy.md",
    ] {
        let current = fs::read_to_string(root.join(path)).expect("current public documentation");
        for retired in [
            "ffhn.target",
            "ffhn.run_report",
            "ffhn.batch_run_report",
            "target.toml",
            "--watch-root",
        ] {
            assert!(!current.contains(retired), "{path} retains {retired}");
        }
    }
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn maintained_document_and_source_inventory_remains_readable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let public_markdown = public_markdown_paths(root).expect("public markdown inventory");
    assert!(
        public_markdown
            .iter()
            .any(|path| path.ends_with("docs/targets.md"))
    );
    let afad_paths = afad_managed_markdown_paths(root).expect("AFAD inventory");
    assert!(afad_paths.iter().any(|path| path.ends_with("docs/cli.md")));
    let frontmatter = afad_frontmatter(&root.join("docs/targets.md"))
        .expect("frontmatter")
        .expect("AFAD frontmatter");
    assert_eq!(frontmatter.afad, "4.0");
    assert_eq!(
        protocol_afad_version(root).expect("AFAD protocol version"),
        "4.0"
    );
    assert!(
        !user_facing_source_paths(root)
            .expect("source inventory")
            .is_empty()
    );
}

#[test]
fn afad_metadata_parsers_reject_incomplete_or_legacy_frontmatter_without_guessing() {
    assert_eq!(
        parse_afad_frontmatter("# reader-first\n").expect("no frontmatter"),
        None
    );
    assert_eq!(
        parse_afad_frontmatter("---\nafad: \"4.0\"\n---\n# document\n")
            .expect("current frontmatter"),
        Some(AfadFrontmatter {
            afad: "4.0".to_owned()
        })
    );
    assert!(parse_afad_frontmatter("---\nafad: \"4.0\"\n").is_err());
    assert!(parse_afad_frontmatter("---\nversion: \"8.1.0\"\n---\n").is_err());
    assert_eq!(
        parse_afad_frontmatter("---\nother: value\n---\n").expect("unmanaged frontmatter"),
        None
    );
    assert_eq!(
        frontmatter_value(" afad: \"4.0\" ", "afad"),
        Some("4.0".to_owned())
    );
    assert_eq!(frontmatter_value("kind: guide", "afad"), None);

    assert_eq!(
        parse_protocol_afad_version("# Protocol\n**Version:** `4.0`\n").expect("version"),
        "4.0"
    );
    assert!(parse_protocol_afad_version("# Protocol\n**Version:** \n").is_err());
    assert!(parse_protocol_afad_version("# Protocol\n").is_err());
}

#[test]
fn repo_contract_inventory_helpers_keep_errors_visible() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let root = temporary.path();
    let invalid = root.join("invalid.md");
    fs::write(&invalid, "---\nafad: \"4.0\"\n").expect("invalid document");
    assert!(afad_frontmatter(&invalid).is_err());
    assert!(afad_frontmatter(&root.join("missing.md")).is_err());

    fs::create_dir_all(root.join(".codex")).expect("protocol directory");
    fs::write(root.join(".codex/PROTOCOL_AFAD.md"), "# Protocol\n").expect("invalid protocol");
    assert!(protocol_afad_version(root).is_err());
}

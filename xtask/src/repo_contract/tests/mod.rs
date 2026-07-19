use std::fs;
use std::path::Path;

use super::*;

#[test]
fn public_docs_describe_current_v2_json_and_html_measurement_contracts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let targets = fs::read_to_string(root.join("docs/targets.md")).expect("targets documentation");
    assert!(targets.contains("JSON Pointer"));
    assert!(targets.contains("HTMLCut"));
    assert!(targets.contains("html_text"));
    assert!(targets.contains("html_rendered_text"));
    assert!(targets.contains("html_attribute"));
    assert!(targets.contains("plain DOM descendant text"));
    assert!(targets.contains("dom_canonicalization"));
    assert!(targets.contains("htmlcut_plan_invalid"));
    assert!(targets.contains("html_text_requires_css_selector"));
    assert!(targets.contains("rust_decimal"));
    assert!(targets.contains("ffhn reset"));
    assert!(!targets.contains("notification_endpoints"));

    let contracts =
        fs::read_to_string(root.join("docs/contracts.md")).expect("contracts documentation");
    assert!(contracts.contains("html_plain_text"));
    assert!(contracts.contains("html_rendered_text"));
    assert!(contracts.contains("detached canonical"));
    let reports = fs::read_to_string(root.join("docs/reports.md")).expect("reports documentation");
    assert!(reports.contains("detached selected-subtree clone"));
}

#[test]
fn public_target_examples_decode_as_current_v2_contracts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    for path in public_target_example_paths(root).expect("public target paths") {
        let text = fs::read_to_string(&path).expect("target text");
        let target: ffhn_core::TargetDocument = toml::from_str(&text).expect("current target TOML");
        target.validate().expect("current target contract");
    }
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
fn repo_contract_inventory_helpers_keep_errors_and_current_target_paths_visible() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let root = temporary.path();
    fs::create_dir_all(root.join("examples")).expect("examples");
    fs::write(root.join("examples/standalone.toml"), "target = true\n").expect("example");
    fs::write(root.join("examples/ignore.txt"), "ignored\n").expect("text example");
    fs::create_dir_all(root.join("watchlist/demo")).expect("watchlist");
    fs::write(root.join("watchlist/demo/target.toml"), "target = true\n").expect("watch target");
    let paths = public_target_example_paths(root).expect("target inventory");
    assert!(paths.contains(&root.join("examples/standalone.toml")));
    assert!(paths.contains(&root.join("watchlist/demo/target.toml")));

    let no_examples = tempfile::tempdir().expect("empty temporary directory");
    assert!(
        public_target_example_paths(no_examples.path())
            .expect("empty target inventory")
            .is_empty()
    );

    let invalid = root.join("invalid.md");
    fs::write(&invalid, "---\nafad: \"4.0\"\n").expect("invalid document");
    assert!(afad_frontmatter(&invalid).is_err());
    assert!(afad_frontmatter(&root.join("missing.md")).is_err());

    fs::create_dir_all(root.join(".codex")).expect("protocol directory");
    fs::write(root.join(".codex/PROTOCOL_AFAD.md"), "# Protocol\n").expect("invalid protocol");
    assert!(protocol_afad_version(root).is_err());
}
